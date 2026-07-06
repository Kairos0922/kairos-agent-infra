# Loop Engine 设计

Loop Engine 是 harness 层核心:以显式状态机驱动 agent 主循环。
本层只依赖各模块 contracts(ChatModel/ToolRegistry/StepSink 等),
实现经 factory 注入;不 import 任何 providers。

## 1. 状态机

┌──────────────────────────────────────────────────────┐
│ ASSEMBLE ──→ MODEL_CALL ──→ ROUTE                     │
│    ↑                          │                       │
│    │            ┌─ tool_calls → EXECUTE → OBSERVE ─┐  │
│    │            │      │(需审批)                    │  │
│    │            │      └→ SUSPENDED ──(回执/超时)──→│  │
│    └────────────│──────────────────────────────────┘  │
│                 ├─ final_answer ──→ FINISHED           │
│                 └─ (预算触发/取消) → WRAP_UP → FINISHED │
└──────────────────────────────────────────────────────┘

| 状态 | 职责 | 发出事件(见 protocol/agent-events.md) |
|---|---|---|
| ASSEMBLE | 调 Context Engine 组装分区 prompt(详设见 context.md) | step_started |
| MODEL_CALL | 经 model_gateway 调模型(tier 由 loop policy 定),流式转发 | text_delta |
| ROUTE | 解析输出:工具调用/最终回答/异常 | — |
| EXECUTE | 经 orchestration 执行工具(并发/超时/权限详设见该篇) | tool_call_* |
| SUSPENDED | 等待审批回执;run 状态持久化,进程可释放 | approval_* |
| OBSERVE | 工具结果规整为下一轮观察;写 Step | step_completed |
| WRAP_UP | 优雅收尾:注入收尾指令,让模型基于已有信息给结论 | — |
| FINISHED | 终态:completed│budget_exhausted│cancelled│failed | run_finished / run_error |

规则:
- 状态转移是唯一控制流,不允许状态处理函数内部隐式跳转。
- sub-agent = EXECUTE 中的一种工具调用,内部是递归的完整状态机
  (详设见 subagent.md);父 Loop 视其为一次(可能很长的)工具执行。

> 澄清(S16 演练增补):模型输出文本(无论是最终答案还是需要
> 用户回应的追问)均视为本轮任务完成,进入 FINISHED(completed)。
> 多轮澄清式对话通过同一 session 内的多个 run 实现
> (见 session-hitl.md §1),而非在单个 run 内等待用户输入。

## 2. Step 记录(不可变,一轮一条)

Step 是三位一体:**trace 单元、checkpoint 单元、事件重建源**。

```rust
// 不可变:构造后字段只读(无 pub setter),一轮一条
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub run_id: String,
    pub agent_path: String,               // "root" 或 "root/0/1"
    pub turn: u32,                        // 从 1 起
    pub context_digest: ContextDigest,    // 分区组装的摘要与哈希,非全文
    pub model_call: ModelCallRecord,      // tier、实际模型、请求哈希、输出、usage
    pub tool_calls: Vec<ToolCallRecord>,  // call_id/名称/参数(服务端全文)/结果/状态/耗时
    pub stop_reason: StopReason,
    pub budget_snapshot: BudgetSnapshot,  // 本轮结束时预算余量
    pub started_at: DateTime<Utc>,        // 时间类型用 chrono::DateTime<Utc>
    pub ended_at: DateTime<Utc>,
}
```

- 写入:OBSERVE 末尾(或 ROUTE 直达 FINISHED 前)经 StepSink 落盘,
  **写入成功才进入下一轮**(checkpoint 语义优先于吞吐)。
- 明文边界:Step 含工具完整参数与模型输出(服务端侧),
  但 contextDigest 不含记忆/知识明文,只含各分区的 id 列表与哈希。
  脱敏后的 summary 才进入对客户端的事件。

## 3. 预算树(Budget)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    pub max_turns: u32,               // 默认 20
    pub max_tokens: u64,              // in+out 合计
    pub max_cost: f64,                // 按 model_gateway 记账口径
    pub deadline: Option<DateTime<Utc>>,
    pub wrap_up_reserve: f64,         // 默认 0.1,预留 10% token 用于 WRAP_UP
}
```

- 检查点:每次进入 ASSEMBLE 前检查;任一维度剩余 ≤ reserve
  即转 WRAP_UP(而非硬中断)。WRAP_UP 本身限一轮、可动用 reserve。
- 预算树:spawn sub-agent 时父预算显式划拨一个子 Budget
  (划拨即从父扣减,子超支不回溯父)。SUSPENDED 等待时长不计入
  deadline 之外的维度,但 deadline 是墙上时钟,照常流逝。

## 4. Loop Policy(来自 Profile,run 级只读)

```yaml
loopPolicy:
  modelTier: strong          # strong│fast│cheap → 路由表映射
  budget: {maxTurns: 20, maxTokens: 200000, maxCost: "1.00"}
  reflection: off             # v1 保留字段,见 §7 暂缓
  approvalTimeout: 10m        # 超时按 denied 处理(S3 已定)
  verboseSubagentEvents: false
```

## 5. 错误语义(分三类,不混淆)

| 类别 | 例 | 处理 |
|---|---|---|
| 工具级错误 | 工具超时/执行失败/参数校验失败 | **不是异常**:作为 status=error 的观察结果回给模型,由模型决定重试或绕行;同一工具连续失败 3 次后在观察中注入"停止重试该工具"指令 |
| 模型调用错误 | 限流/网络/provider 故障 | 重试与降级链归 model_gateway(本层不重试);gateway 最终失败则 run 转 FINISHED(failed),发 run_error(retryable=true) |
| 引擎级错误 | Step 写入失败/状态机不变量被破坏 | 立即 FINISHED(failed),retryable=false,完整现场留 observability |

## 6. 恢复与取消

- 恢复:run 的全部状态 = Profile + Budget 余量 + Step 序列
  + 待决审批。SUSPENDED 或进程重启后,从最后一条 Step 重建上下文
  继续(ASSEMBLE 是纯函数式重组,天然可重入)。
- 取消:客户端 POST /v1/runs/{id}/cancel。语义为
  **当前状态处理完成后**转 WRAP_UP(cancelled 不做收尾、直接
  FINISHED 的"硬取消"作为 force=true 选项)。执行中的工具收到
  取消信号(工具契约含 cancellation token,详设见 tools 篇)。

## 7. 暂缓项(YAGNI,引入须 ADR)

- reflection(自检轮):字段已留,v1 不实现——教师场景先靠
  HITL 审批兜底,eval 数据表明有需要时再上。
- 并行多分支推理(tree/beam):不做。
- Step 压缩存储/冷热分层:量到了再说。

## 8. 契约依赖清单(本层消费的抽象)

ChatModel(model_gateway) │ ToolRegistry/ToolExecutor(tools)
│ StepSink(observability) │ SessionStore(harness/session)
│ ContextEngine(harness/context,层内协作)
