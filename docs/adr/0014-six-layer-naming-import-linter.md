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

## 影响

- 全仓"三层架构/适配层/上层应用层"旧表述替换为六层;删除 `src/kairos/adapter/`。
- `src/kairos/` 建齐六个顶层包;pyproject.toml 落三契约。
- AGENTS.md 增"命名硬规则"节;架构文档以六层表述为准。
