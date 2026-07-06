# ADR 0014:六层架构分层命名与 import-linter 契约冻结

- **状态**:已接受
- **日期**:2026-07-05
- **相关文档**:[project/architecture.md](../project/architecture.md)、[AGENTS.md](../../AGENTS.md)、[foundation/foundation.md](../foundation/foundation.md)
- **上位关系**:是全项目分层的宪法级约定;取代早期"三层架构(上层应用 → 适配层 → Agent Infra)"表述;为所有模块/harness/assembly/server 的依赖方向立法,由 import-linter 在 CI 强制。

## 背景

早期设计用"三层架构(上层应用 → 适配层 → Agent Infra 层)"。随着 harness(运行时骨架)、assembly(声明式装配)、server(对外服务面)等职责浮现,三层不足以表达真实分层,且"适配层"这一名字既承接不清又与新职责重叠。需要一次性冻结分层命名与依赖契约,避免命名分裂与依赖腐化。

## 候选方案

1. **沿用三层 + 在层内塞新职责**:被否——"适配层"要同时装 harness 编排 + server API + assembly 装配,内聚性崩塌。
2. **六层严格分层,命名冻结,import-linter 三契约强制(选定)**。

## 结论

### 六个顶层包(唯一事实源)

| 包名 | 层级 | 一句话职责 | 允许依赖 |
|---|---|---|---|
| `kairos.foundation` | L0 | 零业务语义底座:配置、错误、日志、租户上下文、类型、装配 | (无) |
| `kairos.modules` | L1 | 可插拔基础设施能力模块,模块间互不感知 | `foundation` |
| `kairos.harness` | L2 | Agent 运行时骨架:循环、上下文、编排、会话;**唯一允许编排多个 L1 模块的层** | `modules`(仅 contracts)、`foundation` |
| `kairos.assembly` | L3 | 声明式装配:Profile、Skill、注册表;**不含运行时逻辑** | `harness`、`foundation` |
| `kairos.server` | L4 | 对外服务面:认证、REST/SSE、配额;`TenantContext` 唯一构造点 | `assembly`、`harness`、`foundation` |
| `kairos.cli` | L5 | 参考客户端;**只消费 server API** | (HTTP 边界) |

### 命名硬规则

1. 包名/模块名:小写 snake_case,单数(`contracts/` 目录例外,与既有 memory 模块一致)。
2. 每个 L1 模块固定骨架:`contracts/`、`providers/`、`factory.py` 三件强制命名,不得改叫 `interfaces/`、`impl/`。
3. 契约类用能力名词(`ChatModel`、`ToolRegistry`、`SessionStore`),**不加 `Abstract`/`Base`/`I` 前缀**;实现类 = `<技术名><契约名>`(`LanceDbVectorStore`)。
4. DTO 一律 Pydantic v2,**禁裸 dict 跨层传递**。
5. 事件 type 用 snake_case(`step_started`、`run_finished`),在 [protocol/agent-events.md](../protocol/agent-events.md) 冻结枚举。
6. **禁用词表**:`util`/`utils`/`common`/`helper` 作包名/模块名全仓禁用(低内聚温床)。
7. 改名即全仓:走文档传播清单,`grep` 零残留。

### import-linter 三契约(落 pyproject.toml)

- **契约一(layers)**:六层严格单向,下层不知上层。
- **契约二(independence)**:L1 模块间零依赖。
- **契约三(forbidden)**:`kairos.harness` 禁止 import 任何模块的 `providers`(只依赖 contracts)。

三契约按 grow-by-task 激活:包存在即激活对应契约,新模块创建时加入 independence 列表。

## 理由

- 六层让每层职责单一、依赖方向清晰;"唯一跨模块编排层 = harness"把耦合收敛到一处。
- 命名冻结 + 禁用词表从源头防命名分裂与低内聚。
- import-linter 把架构纪律变成 CI 门禁,违反即红,不靠人肉 review。

## 追记(2026-07-06,ADR 0019/0021 语言与架构迁移)

分层与依赖方向的**结论不变**(六层单向、L1 独立、harness 禁触 providers 仍是宪法级约定);命名与强制手段随 Rust Runtime 落地更新。

> 说明:本追记先经历一版"切纯 TS(kebab-case + camelCase + dependency-cruiser)"的中间修订,同日又随 ADR 0019 修订为 **Rust Runtime + TS UI**。以下为**最终**(Rust)结论,取代中间的 TS 版本。

- **顶层结构**:`kairos.foundation` 等 Python 包 → Cargo workspace 的六层 **crate**:`crates/foundation`、`crates/{memory,model_gateway,...}`(每个 L1 模块一个 crate)、`crates/harness`、`crates/assembly`、`crates/server`,另加 `crates/protocol`;客户端在 `apps/`(见 ADR 0021)。
- **命名硬规则 1(命名风格)**:回归 **snake_case**(Rust 官方 RFC 430:crate/包名/module/目录/文件/函数/变量皆 snake_case),与最初 Python 一致;类型/trait/枚举用 PascalCase。**全仓不用 kebab-case**——包名亦 snake(`kairos_model_gateway`),同一概念一个拼法(参照 rustc `rustc_middle` 风格)。TS 侧(L5 客户端)另按 TS/npm 官方(kebab 包名 + camelCase 标识符)。
- **命名硬规则 2(模块骨架)**:每个 L1 模块 crate 内 `contracts`(trait 定义)、`providers`(实现)两个 mod 强制命名;工厂为 `factory.rs`/`factory` mod。
- **命名硬规则 3(契约)**:契约用能力名词的 **trait**(`ChatModel`、`ToolRegistry`、`SessionStore`),不加 `Abstract`/`Base`/`I` 前缀;实现类型 = `<技术名><契约名>`(`LanceDbVectorStore`)。
- **命名硬规则 4(DTO)**:Pydantic → **serde 结构体**(`#[derive(Serialize/Deserialize)]`);禁裸 map 跨层传递不变。跨进程 DTO(agent-events / 控制 API)字段命名以协议 wire 格式为准(见 protocol/agent-events),Rust 侧用 `#[serde(rename_all)]` 映射。
- **强制工具**:import-linter → **Cargo crate 依赖边界**:下层 crate 不声明上层为依赖 → 上层符号物理不可见,违反即**编译失败**(比任何 linter 更硬)。契约三(harness 禁触 providers)用 crate 内 `pub` 可见性 + 架构测试兜底(见 ADR 0021)。禁用词表(`util`/`utils`/`common`/`helper`)、改名即全仓,均不变。

## 影响(原始,Python 阶段)

- 全仓"三层架构/适配层/上层应用层"旧表述替换为六层;删除 `src/kairos/adapter/`。
- `src/kairos/` 建齐六个顶层包;pyproject.toml 落三契约。
- AGENTS.md 增"命名硬规则"节;架构文档以六层表述为准。
