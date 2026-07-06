# Kairos Agent Infra

一套**解耦的 Agent 基础设施**(headless Agent Runtime 服务)。目标路径:通用个人助手(验证底座)→ 垂直行业助手(商业落地);首个行业为教育。

- **六层架构**:`foundation`(L0)→ `modules`(L1)→ `harness`(L2)→ `assembly`(L3)→ `server`(L4)→ `cli`(L5),依赖严格单向向下。
- **核心命题**:新建行业助手不改底座一行代码,只新增 Profile + Skill 包 + 知识包。此命题是全项目持续验收标准。
- **核心原则**:高内聚低耦合、模块可插拔、契约驱动、避免过度设计(YAGNI)、租户隔离是不变量。
- **技术栈**:Rust(Agent Runtime,L0–L4 + Adapter,tokio)+ TypeScript(UI/客户端,L5);向量库 LanceDB(`lancedb` crate)。架构见 ADR 0019/0021。

## 当前状态

**Phase 1(系统设计)已完成** — 六层架构、事件协议、harness 五篇、模块设计(memory 四件套定稿)、assembly 两篇、教育垂直、纸上演练通过、ADR 0001–0017 建档。实时进度见 [PROGRESS.md](./PROGRESS.md),演进路线见 [roadmap](./docs/project/roadmap.md)。

## 协作方式

本项目由 Code Agent 100% 执行、用户 100% 决策。所有 Agent 协作规范见 [AGENTS.md](./AGENTS.md),核心约定:

- **改动前先报方案**:任何代码/文档/配置修改,动手前先把方案交用户判断。
- **任务收尾三同步**:每个任务结束同步更新单元测试、注释、文档,保证三者与实现一致。
- **进度实时跟进**:[PROGRESS.md](./PROGRESS.md) 是唯一进度事实源。
- **Definition of Done**:实现 + 测试通过 + 覆盖率达标 + cargo fmt/clippy/crate 边界检查 + 三同步 + 进度更新 + 规范提交。

## 项目结构

```
kairos-agent-infra/
├── AGENTS.md          # Agent 协作规范(唯一事实源,跨工具通用)
├── CLAUDE.md          # 薄引用,导入 AGENTS.md
├── PROGRESS.md        # 进度事实源
├── Cargo.toml         # Rust workspace 根:声明六层 crate 成员
├── crates/            # Rust Runtime(L0–L4):
│   ├── foundation/    #   L0 底座
│   ├── memory/ … model_gateway/ tools/ knowledge/ observability/ eval/  # L1 各模块一个 crate
│   ├── harness/       #   L2 运行时骨架
│   ├── assembly/      #   L3 声明式装配
│   ├── server/        #   L4 控制 API + agent-events + 认证
│   └── protocol/      #   agent-events + 控制 API 的 Rust 侧类型
├── apps/              # L5 客户端:
│   ├── cli/           #   CLI
│   └── ui/            #   React/TS UI(+ 桌面壳,壳选型暂缓)
├── packages/
│   └── protocol-ts/   # 协议类型的 TS 侧定义(与 crates/protocol 对齐)
└── docs/              # 设计文档与决策记录
    ├── project/       #   整体项目:概述、六层架构、路线
    ├── protocol/      #   对外事件协议
    ├── foundation/    #   L0 底座
    ├── harness/       #   L2 运行时骨架
    ├── modules/       #   L1 infra 模块(memory / model_gateway / tools / knowledge / observability / eval / benchmark)
    ├── assembly/      #   L3 声明式装配(profile / skills)
    ├── verticals/     #   垂直样例(education)
    └── adr/           #   架构决策记录
```

## 文档入口

文档结构映射系统结构。从 [文档导航](./docs/README.md) 或 [项目概述](./docs/project/overview.md) 开始。重大技术决策见 [ADR](./docs/adr/README.md)。

