"""harness(L2):Agent 运行时骨架,唯一允许编排多个 L1 模块的层。

包含(随 Phase 2+ 落地):
- loop          显式状态机主循环(ASSEMBLE→MODEL_CALL→ROUTE→EXECUTE→OBSERVE)
- context       Context Engine:分区组装、history 压缩、记忆写回、scope 推断
- orchestration 工具调度/超时/重试/权限判定与审批路由
- subagent      sub-agent = 递归 Loop 的工具调用(ADR 0016)
- session       SessionStore 契约 + 中断续跑
- hitl          审批点管理 + AgentEvent 生成
- distill       procedural 经验的离线提炼管线(ADR 0008)

边界约束(import-linter 契约三强制):harness 只 import 各 L1 模块的
contracts/,**禁止 import 任何模块的 providers/**;实现由组装根注入。

详见 docs/harness/。
"""
