# AGENTS.md — 文档维护规范(docs/)

本文件是根 [`../AGENTS.md`](../AGENTS.md) 的**分场景细则**,作用域为 **`docs/` 下的项目文档**。按需载入:只在改动设计/文档时才需遵循这些细则。

## 文档命名

- 文档 Markdown 文件名 **kebab-case**(如 `memory-types.md`、`agent-events.md`、`model-gateway.md`;ADR 为 `NNNN-kebab.md`)。
- 文档是主题 slug,遵循 Markdown/仓库既有约定,**不随其描述的 Rust 模块改用 snake_case**;正文引用代码标识符时各按其语言规则(如正文写 `model_gateway` 模块、`LanceDbVectorStore` 类型)。

## 文档传播清单(改了设计就逐项核对)

改了设计、接口或决策后,按以下清单逐项核对——这些位置最容易漏改,漏改即造成文档不一致。

- **ADR**:重大决策新增 `adr/NNNN-*.md`,并同步更新 `adr/README.md` 索引表;历史 ADR 若被更新则加追记(不改写结论)。
- **memory 模块四件套**:改其一常须连带改其余——`modules/memory/memory-types.md`(数据模型/写入/检索/淘汰)、`retrieval.md`(检索/召回/隔离)、`api.md`(DTO/契约)、`tradeoffs.md`(取舍 + 来源)。
- **harness 五篇联动**:`harness/context.md`(注入/写回/scope)、`session-hitl.md`(Session/审批)、`loop.md`(状态机/Step)、`subagent.md`、`distill.md`——改 scope/记忆时机/procedural 生产常牵动 context ↔ memory-types、distill ↔ memory-types。
- **README 三处**:`modules/memory/README.md`(结构/关键决策)、`README.md`(导航/术语表/ADR 索引摘要)、根 `../README.md`(结构/状态)。
- **项目层三处**:`project/overview.md`(Non-goal)、`project/roadmap.md`(交付清单/路线)、`foundation/foundation.md`(目录/配置/契约测试)。
- **跨层交叉引用**:改契约/DTO 时核对 harness 消费方(context/tools/distill)与 memory 契约是否对齐;改分层命名时核对命名硬规则([`../crates/AGENTS.md`](../crates/AGENTS.md))、workspace `Cargo.toml` 的 crate 依赖边界、`project/architecture.md`。
- **进度**:根 `../PROGRESS.md`(任务项 + 变更记录)。

收尾两步,缺一不可:

1. 运行 `cargo xtask check-docs`,确认内部链接与锚点全绿。
2. `grep` 被替换的旧术语/旧字段名,确认全仓无残留。

## 文档洁癖

- **三同步不留死角**:代码注释(doc comment)、文档、实现任一处改了,另两处同步,不留过时描述(过时的 `MemoryConfig` 字段即前车之鉴)。
- **改名/改决策全仓追到底**:术语、引用、索引、链接一处不漏,按上方「文档传播清单」逐项核对。
- **断链零容忍**:收尾跑 `cargo xtask check-docs`,不留失效链接或锚点。
- **来源诚实标注**:外部事实区分 `【已验证】`/`【待验证】`;近期 preprint 标注"权威度待检验",不伪装成定论。
- **不堆冗余**:文档讲清"为什么"而非复述代码;同一事实只在一处定义、其余引用,不在多处重复维护。

## ADR(架构决策记录)

重大技术决策写入 `adr/NNNN-标题.md`,记录:背景、候选方案、结论、理由、影响。

- 触发场景:选型(向量库、模型)、关键算法策略(融合方式)、架构边界(抽象归属、分层命名)、安全/租户决策、其他难以逆转或影响全局的决策。
- 目的:决策可追溯,避免反复推翻已定结论。历史 ADR 不改写结论,后续决策可对其加"术语更新/实现更新"追记。
- **外部事实必须查证**:ADR 涉及他家工具/外部规范怎么做时,以官方文档/源码核实并**标注来源**;禁用"业界通用""与业界看齐"等模糊说辞掩盖未查证的臆断或实际分歧(参见 ADR 0018 的诚实记录)。
- 已落地决策见 [`adr/`](./adr/)(0001–0024),索引见 [ADR README](./adr/README.md)。
