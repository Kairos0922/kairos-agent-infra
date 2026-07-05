"""Kairos Agent Infra:解耦的 Agent 基础设施(headless Agent Runtime 服务)。

六层架构,依赖严格单向向下:
foundation(L0)→ modules(L1)→ harness(L2)→ assembly(L3)→ server(L4)→ cli(L5)。
横切关注点收敛在 foundation;唯一跨模块编排层是 harness。详见 docs/project/architecture.md。
"""
