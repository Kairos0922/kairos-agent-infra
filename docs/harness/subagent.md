# Sub-Agent 设计

统一模型(已定于 loop.md):**sub-agent = 一种特殊的工具调用,
内部是递归的完整 Loop 状态机**。父 Loop 视其为一次工具执行,
Tool Orchestrator 无需为多 agent 开任何特殊通道。

## 1. 声明(Profile 中,静态)

```yaml
subagents:
  - name: research_assistant        # 成为父 agent 可见的工具名
    profile_ref: education/research@1   # 引用另一个 Profile(带版本)
    description: "文献与案例深度调研,输入研究问题,产出结构化报告"
    budget_ratio: 0.3               # 单次派生最多占父剩余预算比例
```

- 派生目标必须是已注册的 Profile——**没有匿名/动态构造的
  sub-agent**(可审计、可测试、可复用;动态构造列入暂缓)。
- 渲染进 P2 工具区,对模型而言就是一个工具:
  spawn 参数 = task(任务简报) + 可选 context_brief。

## 2. 隔离与共享(逐项明确)

| 资源 | 策略 |
|---|---|
| 上下文 | **全新**:子 Loop 只拿到 task + context_brief,不继承父 history(需要的信息由父模型显式写入简报——这是刻意设计:强迫任务边界清晰) |
| 工具 | 子 Profile 自己的白名单,与父无关;审批规则同样生效,approval_required 事件带 agent_path 上浮到同一客户端 |
| 预算 | 从父剩余预算显式划拨(≤ budget_ratio),划拨即扣减 |
| memory | **读:允许**(同 TenantContext,子 Profile 自己的 namespace 视图);**写回:禁用**——记忆写回只发生在 root run 结束的统一抽取,子 agent 的产出经父采纳后自然进入 |
| session | 子 Loop 无独立 session,挂在父 run 之下(agent_path 定位) |

## 3. 拓扑约束

- 树形,父子单向:兄弟间不通信,一切经父中转(自由拓扑在
  overview 已列 Non-goal)。
- 深度上限默认 2(root→子→孙),Profile 可配但全局硬上限 3。
- 循环引用在 Profile 注册期检测(A 引 B 引 A → 注册失败)。
- 并发派生:允许(父的一轮里多个 spawn 并发执行,归
  orchestration 的并发语义管)。

## 4. 结果与失败语义

- 结果 = 子 run 的 final_text + 结构化 usage,作为工具结果
  回父 Loop 的 OBSERVE。
- 子 run 失败/预算耗尽 = **工具级错误**(status=error + 已产出
  的部分结果),父模型决定重试/绕行/自己做——不上升为父 run 失败。
- 取消向下传播:父被取消,所有活跃子 Loop 收到取消信号,
  各自走 WRAP_UP。

## 5. Step 与事件

- 子 Loop 的 Step 正常写 observability(agent_path 区分),
  回放/eval 可见全貌。
- 客户端默认只见 subagent_spawned/finished(S3 已定)。