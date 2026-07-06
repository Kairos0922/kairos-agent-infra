# ADR 0021:Rust Runtime + TypeScript UI 架构(Monorepo workspace + Cargo crate 边界)

- **状态**:已接受
- **日期**:2026-07-06
- **相关文档**:[ADR 0019](./0019-language-migration-python-to-rust-ts.md)、[ADR 0014](./0014-six-layer-naming-import-linter.md)、[project/architecture.md](../project/architecture.md)、[foundation/foundation.md](../foundation/foundation.md)
- **上位关系**:ADR 0019(语言分工)的架构落地——定义进程边界、仓库结构、依赖强制方式、Adapter 位置。取代 ADR 0014 中 import-linter 的强制手段(命名与分层结论仍沿用,见 0014 追记)。

## 背景

ADR 0019 定下 Rust 写 Runtime、TS 写 UI。落地前需回答四个结构性问题,否则目录树、依赖强制、进程模型无法确定:

1. **Runtime 是不是一份、谁连它?**
2. **Adapter(LLM/MCP)放 Rust 还是 TS?这决定跨语言边界与进程数。**
3. **Rust 后端 + TS 前端如何组织仓库?**
4. **六层单向依赖用什么强制(原 import-linter 是 Python 工具)?**

## 结论

### 1. Runtime 即服务,永远一份

Runtime 是**独立的长运行进程**,承载 L0–L4 全部职责。所有入口都是它的客户端,经**同一套稳定边界**接入:

```
CLI  ─┐
Desktop(Tauri/Electron,壳选型暂缓)─┤
Cloud ─┼─→  agent-events 协议 + 控制 API  ─→  Rust Agent Runtime(唯一一份)
API  ─┘
```

这是既有 L4 server(TenantContext 唯一构造点)+ L5 客户端(只消费 API)+ agent-events 协议的显式化,非新增设计。UI(React/TS)是又一个 L5 客户端。

### 2. Adapter 在 Rust,MCP 走子进程

- **LLM Adapter(OpenAI/Anthropic/Gemini/本地)在 Rust**:用 Rust HTTP 客户端直连,Runtime 自包含、无跨语言热路径。
- **MCP 走子进程**:MCP 本就是 stdio/子进程协议,天然跨进程,不引入额外**语言**边界。Runtime 以子进程方式拉起 MCP server,标准协议通信。
- **结论:Runtime 是单一 Rust 进程**(+ 按需拉起的 MCP 子进程 / sandbox 子进程)。跨语言边界只有一处——Runtime 与 UI 之间的 agent-events + 控制 API,且是网络/IPC 协议边界,非函数调用边界。

### 3. Monorepo:Rust workspace + TS apps

单仓,一次提交覆盖协议两侧:

```
kairos-agent-infra/
├── Cargo.toml                 # workspace 根:声明成员 crates
├── crates/
│   ├── foundation/            # L0:config/errors/tenancy/logging/factory
│   ├── memory/                # L1 模块(每个 L1 模块一个 crate)
│   ├── model_gateway/         # L1
│   ├── tools/                 # L1
│   ├── knowledge/             # L1
│   ├── observability/         # L1
│   ├── eval/                  # L1
│   ├── harness/               # L2:loop/scheduler/session/permission/event-bus
│   ├── assembly/              # L3:profile/skill
│   ├── server/                # L4:控制 API + agent-events + 认证
│   └── protocol/              # 协议类型(agent-events + 控制 API 的 Rust 侧定义)
├── apps/
│   ├── cli/                   # L5:Rust CLI 客户端(或 TS,视情况)
│   └── ui/                    # L5:React/TS UI(+ 桌面壳,壳选型暂缓)
├── packages/
│   └── protocol-ts/           # 协议类型的 TS 侧定义(与 crates/protocol 对齐)
└── docs/
```

### 4. 六层依赖靠 Cargo crate 边界物理强制

每层是独立 crate,依赖方向写死在各 crate 的 `Cargo.toml`:

- 下层 crate **不声明**上层为依赖 → 上层符号在下层**物理不可见**(编译期硬失败,比 import-linter/dependency-cruiser 的规则检查更硬)。
- 契约一(六层单向):`foundation` 无内部依赖;`memory` 等 L1 只依赖 `foundation`;`harness` 依赖各 L1 的 crate(但只用其 contracts 模块)+ `foundation`;依此类推。
- 契约二(L1 独立):L1 各 crate 互不声明对方为依赖。
- 契约三(harness 禁触 providers):在模块 crate 内,providers 作为私有 mod 不 `pub`,或拆为 feature/子 crate;harness 只依赖公开的 contracts。辅以 `cargo test` 架构断言兜底。

## 理由

- **Runtime 一份 + 稳定边界**:契合"长运行、多入口、双端",且是既有设计的显式化。
- **Adapter 在 Rust**:消除 Runtime 热路径的跨语言依赖;MCP 子进程是协议本性,非妥协。
- **Monorepo**:协议两侧原子提交、类型对齐容易、单一 CI。
- **Cargo crate 边界**:把分层纪律变成编译期物理约束,是 Rust 独有的、比任何 linter 都强的强制手段——ADR 0014 想要的"架构纪律即门禁",在 Rust 下升级为"架构纪律即编译错误"。

## 影响

- 目录结构由 `src/kairos/`(Python)/ `src/`(TS)改为 `crates/` + `apps/` + `packages/`(见上)。
- import-linter / dependency-cruiser 移除;依赖方向由 Cargo 依赖图强制 + 架构测试兜底。
- `protocol` crate 与 `protocol-ts` package 是协议的两侧事实源,需保持对齐(CI 校验或代码生成,后续任务定)。
- 桌面壳(Tauri vs Electron)选型**暂缓**,不阻塞 Runtime 与协议落地;UI 先以能连 Runtime 的最小客户端验证。
- foundation.md、architecture.md、AGENTS.md 的目录树/命令/命名随之更新。
