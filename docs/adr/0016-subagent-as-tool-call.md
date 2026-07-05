# ADR 0016:Sub-agent 统一建模为工具调用(父子式,禁自由拓扑)

- **状态**:已接受
- **日期**:2026-07-05
- **相关文档**:[harness/subagent.md](../harness/subagent.md)、[harness/loop.md](../harness/loop.md)
- **上位关系**:是 harness/loop 状态机的组成部分;与 [overview](../project/overview.md) Non-goal"多 agent 自由协作拓扑"呼应。

## 背景

Agent 常需要把子任务委派给一个受限的子 agent(独立上下文、受限工具集、独立预算)。设计问题:**sub-agent 是一等的并列 agent(自由协作拓扑),还是父 agent 的一次受控调用?**

## 候选方案

1. **自由多 agent 拓扑**(任意 agent 互相通信/协商):被否——控制流复杂、预算与可观测性难界定、调试地狱,远超 MVP 需要(overview 明确列为 Non-goal)。
2. **sub-agent = 一种特殊工具调用(选定)**:父 Loop 在 EXECUTE 阶段 spawn 一个子 Loop,子 Loop 是递归的完整状态机,其最终结果作为工具结果回传父 Loop。

## 结论

- **sub-agent 是 EXECUTE 中的一种工具调用**:`spawn(profile_ref, task, budget)`。
- **子 = 递归的完整 Loop**:独立上下文、受限工具集(父的子集)、预算从父显式划拨(划拨即从父扣减,子超支不回溯父)。
- **父视其为一次(可能很长的)工具执行**:子的最终输出作为 `ToolResult` 回到父的 OBSERVE。
- **只有父子式树形拓扑**,无兄弟间直接通信、无环、无自由协商。

## 理由

- **控制流单一**:整个多 agent 系统仍是一棵 Loop 树,每个节点是同一个状态机,复用 loop/budget/observability 的全部机制,零新概念。
- **预算天然可控**:预算树(父划拨子)使 max_tokens/max_cost/deadline 在树上可加、可界定,不会失控。
- **可观测性一致**:子的 Step 挂在 `agent_path`(如 `root/0/1`)下,回放/eval 归因照常。
- **YAGNI**:自由拓扑的协商/共识机制没有 MVP 需求,过早引入是纯负债。

## 影响

- harness/subagent.md:`spawn` 契约 + 预算划拨 + agent_path 命名。
- harness/loop.md:EXECUTE 分支包含 spawn;子 Loop 是递归实例。
- Non-goal 保持:多 agent 自由协作拓扑不做。
