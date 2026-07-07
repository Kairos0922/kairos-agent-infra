# 架构决策记录 (ADR)

本目录记录 Kairos 的重大技术决策。每条 ADR 记录:背景、候选方案、结论、理由、影响。

决策可追溯,避免反复推翻已定结论。规范见 [CLAUDE.md](../../CLAUDE.md) 的 ADR 一节。

## 索引

| 编号 | 标题 | 状态 |
|------|------|------|
| [0001](./0001-vector-store-lancedb.md) | 向量库选用 LanceDB | 已接受(0019 追记:用 `lancedb` crate,内核即 Rust;0022 追记:单机前提由 cell-per-tenant 承载) |
| [0002](./0002-hybrid-fusion-rrf.md) | 混合检索融合策略选用 RRF | 已接受 |
| [0003](./0003-abstractions-in-module.md) | 抽象接口归属记忆模块,不预先上提到底座 | 已接受 |
| [0004](./0004-no-knowledge-graph-mvp.md) | MVP 不做知识图谱,先做原子事实 | 已接受 |
| [0005](./0005-decay-ranking-conflict-deletion.md) | 衰减管排序,冲突管删除(两套机制分开) | 已接受 |
| [0006](./0006-memory-classification-by-cognitive-function.md) | 记忆按认知功能分类(工作记忆归 harness 层,长期记忆分情景/语义/程序) | 已接受(0014 术语更新) |
| [0007](./0007-memory-mechanism-vs-policy-timing.md) | 记忆模块是机制,时机与质量评估是策略(写入/召回时机 + 选择性召回) | 已接受 |
| [0008](./0008-procedural-evaluation-decoupling.md) | 程序记忆的 trace 评估/提炼与记忆模块解耦 | 已接受 |
| [0009](./0009-single-multi-user-scoping-isolation.md) | 单/多用户记忆作用域与隔离(隔离是机制,共享是策略) | 已接受(0013 更新物理实现) |
| [0010](./0010-auth-api-key-per-tenant.md) | 认证采用 API Key per tenant,Authenticator 契约留 OIDC 扩展位 | 已接受(0023 追记:user_id 改为认证派生) |
| [0011](./0011-model-contract-ownership.md) | 模型能力契约归属:模块内定义 + 组装根适配 | 已接受 |
| [0012](./0012-tenant-context-explicit-passing.md) | TenantContext 显式传参,禁用 contextvar 隐式传递 | 已接受 |
| [0013](./0013-lancedb-tenant-physical-tables.md) | LanceDB 租户隔离采用物理分表(`{tenant_id}__{kind}`) | 已接受 |
| [0014](./0014-six-layer-naming-import-linter.md) | 六层架构分层命名与 import-linter 契约冻结 | 已接受(0019/0021 追记:命名回 snake_case + Cargo crate 边界) |
| [0015](./0015-vector-store-uplift-foundation.md) | 向量存储契约与 RRF 融合上提 foundation(Phase 3) | 已接受(Phase 3 触发) |
| [0016](./0016-subagent-as-tool-call.md) | Sub-agent 统一建模为工具调用(父子式,禁自由拓扑) | 已接受 |
| [0017](./0017-scope-metadata-inference.md) | Scope Metadata 推断规则与降级语义 | 已接受 |
| [0018](./0018-config-file-format-toml.md) | 配置文件格式选用 TOML(项目级 + 用户级双层) | 已接受(0019 追记:Rust 下 TOML 更稳) |
| [0019](./0019-language-migration-python-to-rust-ts.md) | 实现语言由 Python 切换为 Rust(Runtime)+ TypeScript(UI) | 已接受(0022 追记:主部署目标=机构云,本地为构建档) |
| [0020](./0020-cpu-offload-strategy.md) | CPU 密集计算下沉策略(优先现成 Rust 内核库) | 已接受(0019 追记:Runtime 即 Rust,动机消解) |
| [0021](./0021-rust-runtime-ts-ui-architecture.md) | Rust Runtime + TS UI 架构(Monorepo workspace + Cargo crate 边界) | 已接受(0022 追记:部署拓扑=每租户一个 cell) |
| [0022](./0022-deployment-topology-cell-per-tenant.md) | 生产部署拓扑——每租户一个 Runtime cell | 已接受 |
| [0023](./0023-user-level-authentication.md) | 用户(教师)级认证——user_id 由客户端声明升级为认证派生 | 已接受 |
| [0024](./0024-memory-data-versioning-and-compliance-lifecycle.md) | 记忆数据的版本化与合规生命周期 | 已接受 |

## 状态说明

- **提议中 (Proposed)**:待用户决策。
- **已接受 (Accepted)**:已采纳,正在执行。
- **已废弃 (Deprecated)**:被后续决策取代,保留记录(注明被哪条取代)。
- **追记/更新**:历史 ADR 不改写结论,但后续决策可对其术语或局部实现追记说明(如 0006 术语更新、0009 物理实现由 0013 更新)。
