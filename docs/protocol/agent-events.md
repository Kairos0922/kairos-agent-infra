# Agent 事件协议(agent-events)

本协议是 L4(server)与 L5(客户端:CLI/行业APP)之间的唯一耦合面。
版本化管理,任何变更走 ADR;客户端团队只依赖本文档与 JSON Schema,
不依赖 Kairos 内部实现。

## 1. 传输与方向

- 服务端 → 客户端:SSE(text/event-stream),一个 run 一条流,
  端点 GET /v1/runs/{run_id}/events。
- 客户端 → 服务端:普通 REST(触发 run、审批回执、取消),不走流。
- 断线重连:事件带全局单调递增 seq;SSE 标准 Last-Event-ID 携带
  最后收到的 seq,服务端从 seq+1 补发。事件在 run 结束后保留
  可回放(存储于 observability,保留期见 §6)。
- 心跳:空闲 15s 发 heartbeat 事件,客户端以 45s 无消息判定断线。

## 2. 事件信封(Envelope,所有事件统一)

```json
{
  "protocol": "kairos-events/1.0",
  "seq": 42,                     // run 内单调递增,从 1 开始
  "run_id": "run_...",
  "session_id": "ses_...",
  "agent_path": "root",          // sub-agent 用 "root/0/1" 表示派生树位置
  "type": "text_delta",
  "ts": "2026-07-05T12:00:00Z",
  "payload": { ... }             // 按 type 定义,见 §3
}
```

约束:客户端遇到未知 type **必须忽略而非报错**(向前兼容基础);
payload 未知字段同样忽略。

## 3. 事件全集(v1.0 冻结,新增须走 ADR)

### 生命周期
| type | payload 要点 |
|---|---|
| run_started | profile_ref, budget(max_turns/max_tokens/deadline) |
| run_finished | status: completed│budget_exhausted│cancelled, usage(tokens/cost/turns), final_text? |
| run_error | code, message(用户可读,不含内部堆栈), retryable: boolean |
| heartbeat | (空) |

### 循环与文本
| type | payload 要点 |
|---|---|
| step_started | turn: number |
| text_delta | delta: string(助手回复增量) |
| step_completed | turn, usage(本轮 tokens), stop_reason |

### 工具
| type | payload 要点 |
|---|---|
| tool_call_started | call_id, tool_name, args_summary(脱敏摘要,非完整参数) |
| tool_call_result | call_id, status: ok│error│timeout│denied, result_summary |

### 审批(HITL)
| type | payload 要点 |
|---|---|
| approval_required | approval_id, tool_name, reason, args_summary, expires_at |
| approval_resolved | approval_id, decision: approved│denied│expired, by: user│timeout |

回执端点:POST /v1/approvals/{approval_id} {decision}。
等待审批期间 run 挂起,超时(默认 10min,Profile 可配)按 denied 处理并继续
"优雅收尾"分支。

### Sub-agent
| type | payload 要点 |
|---|---|
| subagent_spawned | child_path, profile_ref, task_summary, budget |
| subagent_finished | child_path, status, usage |

子 agent 的内部事件(text_delta 等)默认**不**转发给客户端,仅发
spawned/finished 两端点;Profile 可开 verbose 模式全量转发
(事件以 agent_path 区分归属)。

### 保留位(v1.0 定义但默认不发,开关在 Profile)
| type | 说明 |
|---|---|
| thinking_delta | 模型推理过程增量;教育教师场景默认关闭 |
| memory_written | 记忆写入通知(kind + id,无明文);默认关闭 |

## 4. 版本策略

- 协议号 kairos-events/MAJOR.MINOR,置于信封 protocol 字段。
- MINOR(向后兼容):新增事件 type、payload 新增可选字段。
  客户端按 §2 忽略规则自然兼容。
- MAJOR(破坏性):字段改名/删除/语义变更。服务端支持相邻两个
  MAJOR 并行(客户端经 Accept 头协商),旧版本弃用期 ≥ 6 个月。
- Schema 事实源:所有事件为 serde 结构体 + `#[serde(tag = "type")]`
  区分变体(crates/harness/src/hitl/events.rs),JSON Schema 由 CI
  经 schemars 自动导出到 schemas/agent-events/,与文档不一致视为 CI 失败。

## 5. 脱敏不变量(与合规策略联动)

- 事件中一切 *_summary 字段经统一脱敏器生成:不含学生 PII、
  不含记忆/知识明文、完整工具参数仅存 observability(服务端)。
- run_error.message 不含堆栈与内部路径。

## 6. 存储与回放

事件持久化归 observability 模块(事件可由 Step 记录重建,
不双写明细)。在线保留期默认 30 天(租户可配),超期仅留 run 级
汇总。回放端点与查询归 S9(observability 设计)。