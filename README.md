# Kairos Agent Infra

一套**解耦的 Agent 基础设施**。各基础设施模块(记忆、上下文等)彼此独立,可单独演进、单独替换。

- **三层架构**:上层应用层 → 适配层 → Agent Infra 层。
- **核心原则**:高内聚低耦合、模块可插拔、避免过度设计。
- **技术栈**:Python;向量库 LanceDB。

## 当前状态

**第一阶段(MVP)设计阶段** — 范围:项目底座 + 记忆模块。其余模块只留接入约定。实时进度见 [PROGRESS.md](./PROGRESS.md)。

## 协作方式

本项目由 Code Agent 100% 执行、用户 100% 决策。所有 Agent 协作规范见 [CLAUDE.md](./CLAUDE.md),核心约定:

- **改动前先报方案**:任何代码/文档/配置修改,动手前先把方案交用户判断。
- **任务收尾三同步**:每个任务结束同步更新单元测试、注释、文档,保证三者与实现一致。
- **进度实时跟进**:[PROGRESS.md](./PROGRESS.md) 是唯一进度事实源。
- **Definition of Done**:实现 + 测试通过 + 覆盖率达标 + lint/类型/依赖检查 + 三同步 + 进度更新 + 规范提交。

## 项目结构

```
kairos-agent-infra/
├── CLAUDE.md          # Agent 协作规范(AGENTS.md 导入此文件)
├── AGENTS.md          # 空壳,导入 CLAUDE.md
├── PROGRESS.md        # 进度事实源
├── pyproject.toml     # 依赖与工具配置(待创建)
├── src/kairos/        # 源码(待创建):foundation / modules/memory / adapter
├── tests/             # 测试(待创建):unit / contracts / integration
└── docs/              # 设计文档与决策记录
    ├── project/       #   整体项目:概述、架构、路线
    ├── foundation/    #   底座
    ├── modules/memory/#   记忆模块(自包含)
    └── adr/           #   架构决策记录
```

## 文档入口

文档结构映射系统结构。从 [文档导航](./docs/README.md) 或 [项目概述](./docs/project/overview.md) 开始。重大技术决策见 [ADR](./docs/adr/README.md)。
