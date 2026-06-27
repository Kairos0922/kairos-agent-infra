"""Kairos Agent Infra:解耦的 Agent 基础设施。

三层架构:上层应用 → 适配层(adapter) → Agent Infra 层(modules)。
横切关注点收敛在 foundation。当前阶段实现底座 + 记忆模块。
"""
