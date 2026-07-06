# ADR 0019:实现语言由 Python 切换为 Rust(Runtime)+ TypeScript(UI)

- **状态**:已接受
- **日期**:2026-07-06
- **相关文档**:[project/architecture.md](../project/architecture.md)、[foundation/foundation.md](../foundation/foundation.md)、[ADR 0014](./0014-six-layer-naming-import-linter.md)、[ADR 0018](./0018-config-file-format-toml.md)、[ADR 0020](./0020-cpu-offload-strategy.md)、[ADR 0021](./0021-rust-runtime-ts-ui-architecture.md)
- **上位关系**:全局性决策,影响所有层的实现语言与工程化基线;不改变六层架构、依赖方向、租户隔离等设计层不变量。语言与进程边界的架构细节见 ADR 0021。

> **演进说明(2026-07-06)**:本 ADR 初版曾结论为"切换为纯 TypeScript"。同日经进一步评估双端部署与长运行 Runtime 的稳定性/并发/系统能力诉求后,修订为**Rust 写 Runtime(L0–L4)+ TypeScript 写 UI(L5)与 Adapter 编排边界之上的客户端**的双语言分层。本文记录最终结论;纯 TS 方案作为候选保留在下方。

## 背景

Kairos 原以 Python 实现(Phase 1 设计完成,Phase 2 起步:foundation 全套 + memory contracts 已落地,约 1000 行源码)。随部署形态与产品形态明确,出现了 Python 难以满足的硬约束:

- **长运行 Agent Runtime**:Runtime 是常驻、长生命周期进程,承载 harness/loop/scheduler/session/permission/event-bus。要求**无 GC 停顿、内存可控、并发调度稳、生命周期长的逻辑健壮**。
- **双端 + 统一 Runtime**:同一个 Runtime 同时支撑本地(C 端个人用户,含教育行业老旧低配机器)与服务器;所有入口(CLI / Desktop / Cloud / API)都是它的客户端。**Runtime 永远只有一份**。
- **本地端约束**:老旧机器要求快启动、低内存占用、单文件/小体积分发。
- **系统级能力**:sandbox、子进程、FS、权限隔离等是 Runtime 一等职责,需要贴近系统、强隔离。

问题:**继续用 Python,还是换语言?Runtime 与 UI 是否分语言?**

## 候选方案

1. **保持 Python(现状)**:被否——长运行内存/并发、系统级隔离、本地分发均是弱项。
2. **纯 TypeScript(本 ADR 初版结论)**:Node/Bun 一套语言写全栈,生态最强(MCP/模型 SDK 原生)、开发快、启动/占用"足够好"。被否/降级——对**长运行 Runtime 的稳定性、并发调度、内存可控性、sandbox 隔离强度**,TS/Node 仍弱于 Rust;而这些正是常驻 Runtime 的核心诉求。保留其优点用于 **UI 层**。
3. **纯 Rust**:被否——UI/插件生态、产品迭代速度是 Rust 弱项;前端用 Rust 得不偿失。
4. **Rust Runtime + TypeScript UI(选定)**:各取所长。Runtime 核心(L0–L4)+ LLM/工具 Adapter 用 **Rust**(稳定、并发、系统能力、单二进制);UI/CLI 等入口客户端(L5)用 **TypeScript**(React + WebView 壳);两者以稳定协议(agent-events + 控制 API)解耦。

## 结论

**语言按层分工:**

| 层 | 职责 | 语言 |
|---|---|---|
| L0 foundation | config / errors / tenancy / logging / factory | **Rust** |
| L1 modules | memory / context / tools / model_gateway … 领域逻辑 | **Rust** |
| L2 harness | loop / scheduler / session / permission / event-bus | **Rust**(Runtime 核心) |
| L3 assembly | Profile / Skill 加载校验 | **Rust** |
| L4 server | 控制 API + agent-events(SSE)+ 认证 | **Rust** |
| Adapter | LLM(OpenAI/Anthropic/Gemini/本地);MCP 走子进程 | **Rust** |
| — 稳定边界 — | agent-events 协议 + 控制 API | 语言无关 |
| L5 客户端 | React UI / 桌面壳 / CLI | **TypeScript** |

工具链契约重映射(设计不变):

| 关注点 | Python(原) | 新 |
|---|---|---|
| Runtime 语言 | Python | **Rust** |
| 构建 / 包管理 | uv | **Cargo**(Rust workspace);UI 侧 pnpm |
| DTO / 校验 | pydantic v2 | **serde**(+ 校验);跨边界 DTO 由协议 schema 定义 |
| 类型检查 | mypy strict | **rustc**(编译期);UI 侧 tsc strict |
| 格式化 + lint | ruff | **rustfmt + clippy**;UI 侧 Biome |
| 依赖方向 | import-linter | **Cargo crate 边界**(物理强制,见 ADR 0021) |
| 测试 + 覆盖率 | pytest + pytest-cov | **cargo test**(+ 覆盖率工具);UI 侧 Vitest |
| 配置格式 | TOML(tomllib) | TOML(serde + toml)——**格式不变**,Rust 一等公民,见 ADR 0018 追记 |
| 异步 | asyncio | **tokio** |
| 本地打包 | (PyInstaller,劣) | Cargo 单二进制(Runtime);UI 壳 Tauri/Electron(暂缓选型) |

**迁移窗口**:此时源码仅约 1000 行,一次性整体切换成本最低。设计资产(47 篇文档、19 篇 ADR、六层架构、事件协议、多租户不变量、memory 四件套)语言无关,完整保留。

## 理由

- **Runtime 诉求匹配 Rust**:长运行、无 GC、并发调度、内存可控、系统级隔离——常驻 Agent Runtime 的核心诉求,Rust 是最优解。
- **UI 诉求匹配 TS**:UI/插件/产品迭代/Web 能力是 TS 主场;React + WebView 是本地 Agent 产品的主流形态,契合本地端快启动/低占用。
- **稳定协议边界解耦**:Runtime 与 UI 以 agent-events + 控制 API 交互,双方可独立演进;这本就是既有 L4/L5 + 协议设计,只是显式化并指定语言。
- **Adapter 在 Rust + MCP 子进程**:Runtime 自包含、无跨语言热路径;MCP 本是 stdio/子进程协议,天然跨进程,不引入额外语言边界(细节见 ADR 0021)。
- **诚实记录**:纯 TS 更快、生态更省心,但对长运行 Runtime 的稳定性/并发/隔离不如 Rust;取"Runtime 稳定性" > "全栈单语言便利",接受双语言与协议边界的复杂度。

## 影响

- **代码**:Python/TS 后端产物作废,Runtime 以 Rust workspace 重建(见 ADR 0021 目录结构);TS 保留 UI/CLI 客户端。
- **命名**:回归 Rust 惯例 snake_case(crate/module/文件/标识符),与最初 Python 一致(ADR 0014 追记)。
- **契约强制**:import-linter / dependency-cruiser → **Cargo crate 依赖边界**(下层 crate 不声明上层依赖即物理不可见,比 linter 更硬)。
- **CI**:验证链改为 `cargo fmt --check → cargo clippy → cargo test`(+ crate 边界)与 UI 侧 `biome → tsc → vitest`。
- **文档**:AGENTS.md、architecture.md、foundation.md、README、各模块/harness/assembly 文档按传播清单同步为 Rust;ADR 0014/0018 加追记,0001/0013 说明 LanceDB 用 Rust 原生绑定(`lancedb` crate 内核即 Rust)。
- **CPU 密集计算**:Runtime 既是 Rust,ADR 0020 的"下沉到 Rust/WASM"策略在 Runtime 内天然满足,不再需要跨语言下沉(见 ADR 0020 追记)。
- **工具脚本**:原 `tools/check_doc_links.py` 重写为 Rust `xtask`(`cargo xtask check-docs`),消除 Python 运行时依赖,并扩展覆盖根级 Markdown。至此全仓无 Python。
