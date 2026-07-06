# 项目进度 (PROGRESS)

> 本文件是 Kairos 项目**唯一的进度事实源**,由 Code Agent 实时维护。
> 规则见 [AGENTS.md](./AGENTS.md):任务开始标"进行中",完成标"完成"并在变更记录追加一行;新任务/范围变化先报用户批准。
> 状态图例:`[ ]` 未开始 · `[~]` 进行中 · `[x]` 完成 · `[!]` 阻塞(注明原因)

阶段划分与验收标准见 [docs/project/roadmap.md](./docs/project/roadmap.md);分层职责见 [docs/project/architecture.md](./docs/project/architecture.md)。

## Phase 1:系统设计(已完成)

六层架构、事件协议、harness 五篇、模块设计(memory 四件套 S10 定稿)、assembly 两篇、教育垂直、纸上演练(S16)通过、ADR 0001–0017 建档。核心产出:**"零改码扩展"命题在设计层面验证通过**。

- [x] 六层架构与依赖契约([architecture](./docs/project/architecture.md)、ADR 0014)
- [x] 对外事件协议([protocol/agent-events](./docs/protocol/agent-events.md))
- [x] harness 五篇(loop / context / subagent / session-hitl / distill)
- [x] L1 模块设计:memory 四件套定稿 + model_gateway / tools / knowledge / observability / eval
- [x] benchmark 子项目(protocol / dataset)
- [x] assembly 两篇(profile / skills)+ 教育垂直(education)
- [x] 底座设计([foundation](./docs/foundation/foundation.md),六层 + tenancy)
- [x] ADR 0001–0017 建档 + 索引;0006/0007/0008/0009 加术语/实现更新追记
- [x] 项目规范(README / AGENTS / CLAUDE / PROGRESS)对齐六层
- [x] 工程化骨架:Cargo workspace(六层 crate + protocol)、`crates/`、`apps/`、CI 验证链(cargo fmt/clippy/test + crate 边界)

## Phase 2:最小可跑通(目标:个人助手,单租户内验证)

> 详见 [roadmap Phase 2](./docs/project/roadmap.md)。验收:CLI 完成一次"带记忆的多轮对话"完整 run。

- [x] foundation:tenancy / config / errors / logging / factory 落地(Rust)
- [ ] model_gateway:ChatModel(openai_compat)+ tier 路由(strong/fast)+ 基础重试
- [ ] observability:StepSink + TraceQuery 最小实现(SQLite)
- [ ] harness/loop + harness/context:状态机 + 分区组装(压缩/scope 推断最简版)
- [ ] session-hitl:SessionStore(SQLite)+ 审批流
- [ ] tools:builtin 全集 + Executor
- [x] memory:provider 契约(VectorStore/EmbeddingProvider/RerankProvider/Tokenizer)落地为 trait;领域逻辑(MemoryStore/Retriever + LanceDB provider + 写入管线 + hybrid + RRF)待落地
- [ ] memory 契约测试:隔离三连 + 幂等 + filter 下推(覆盖所有 provider 实现)
- [x] 六层依赖强制:Cargo crate 边界随 crate 创建即物理生效(替代 import-linter)
- [ ] **里程碑**:memory 召回三模式(proactive/tool/hybrid)A/B 裁决跑出结论

## Phase 3:个人助手可用

- [ ] knowledge 模块 + 向量存储上提 foundation(ADR 0015)
- [ ] eval 完整化(CaseSet + 回归基线 + CI 阻断)
- [ ] benchmark 中文种子集打通端到端
- [ ] subagent 实现;distill 管线 v1(人工触发)
- [ ] **验收**:个人助手日常可用,eval 基线建立

## Phase 4:教育行业验证(零改码命题的代码级验证)

- [ ] assembly 层(Profile/Skill 加载器 + 装配期校验)
- [ ] server 层(认证 API Key / REST+SSE / 配额)
- [ ] education Profile 落地(1 学科 + 4 Skill)
- [ ] **验收**:上线全程零修改 harness/modules/foundation 代码

## Phase 5+(按需评估,不预排期)

MCP 全面接入 · PPT 渲染(sandbox)· 多学科横向扩展 · tenant 级知识包 · 行业 APP 对接 · distill 自动化 · 灰度实验框架。

## 变更记录

- 2026-07-07 **Phase 1/2 已完成任务符合性 review + 修复**(对照设计文档逐项核查)。结论:六层 crate 边界/依赖倒置/租户 fail-closed/错误封装/配置分层均忠实落地,发现并修复三处问题:① **P0 CI 缺陷**:`ci.yml` 覆盖率步骤包名误写 `kairos-foundation`(连字符),workspace 实为 `kairos_foundation`,导致 `cargo llvm-cov` 报 "not found package" 退出——DoD 覆盖率门槛从未真正生效;已修正,同款命令现正确跑出 foundation 行覆盖 93.47% 达标。② **P1 config 双份默认陷阱**:`KairosSettings` 既派生 `Default`(log_level 得空串)又手写 `with_defaults()`(得 "INFO"),存在误用入口;改为移除派生、单一手写 `Default` 实现(log_level="INFO"),`load_settings` 改用 `default()`,并补测试 `default_impl_uses_info_not_empty` 锁不变量(foundation 单测 24→25)。③ **P2 文档一致性**:memory/api.md 错误映射表 Python 遗留命名(`ValidationError` 等)改为 Rust 枚举变体(`KairosError::Validation` 等);foundation.md 去除未采用的 `figment` 描述,改述为 `toml::Value` 中间态深合并。验证:fmt/clippy 零告警、cargo test 全过、check-docs 全链接 OK。

- 2026-07-06 **语言与架构迁移:Python → Rust Runtime + TypeScript UI**(ADR 0019/0021)。决策链:先评估 Python→Rust 全量重写,再收敛为 **Rust 写 Runtime(L0–L4 + Adapter,tokio)+ TS 写 UI(L5)** 的双语言分层,以 agent-events + 控制 API 为稳定跨语言边界;Runtime 永远一份,CLI/Desktop/Cloud/API 皆客户端;Adapter 在 Rust、MCP 走子进程,Runtime 单一进程。仓库改为 **Monorepo:Cargo workspace(`crates/` 六层各一 crate + `crates/protocol`)+ `apps/`(cli/ui)+ `packages/protocol-ts`**;六层单向依赖由 **Cargo crate 依赖边界编译期物理强制**(替代 import-linter/dependency-cruiser)。① **ADR**:重写 0019(Python→Rust+TS,含纯 TS 中间方案的诚实记录)、新增 0021(Rust Runtime + TS UI 架构 + workspace + crate 边界 + Adapter/MCP)、0020(CPU 下沉动机随 Runtime 即 Rust 消解)、0014/0018/0012 加 Rust 追记(命名回 snake_case、TOML 更稳、task-local 禁用)、0001/0013 说明用 `lancedb` crate;更新 ADR README 索引至 0001–0021。② **代码**:清理全部旧 TS/Python 产物;建 7-crate workspace;`foundation` 落地 errors(`KairosError` 统一枚举 + thiserror)、tenancy(不可变 struct + `new()` 构造校验 fail-closed)、config(serde + toml 分层加载:env>.env>项目>全局>默认)、logging(tracing JSON)、factory(泛型 `Registry<T,A>`);`memory` 落地四个 provider 契约 trait(VectorStore/EmbeddingProvider/RerankProvider/Tokenizer);L2–L5 为占位 crate(空 lib,crate 边界从第一天生效)。③ **验证**:`cargo fmt --check`、`cargo clippy -D warnings` 零告警、`cargo test` 24 单测 + 2 doctest 全过、`cargo llvm-cov` foundation **行覆盖 93.43%**(config 92.15/errors 95.60/factory 90.62/logging 100/tenancy 100),≥80% 门槛达标。④ **文档三同步**:AGENTS(工程化基线/命名硬规则/常用命令/架构纪律全 Rust 化)、architecture(Runtime 即服务分层图 + Adapter/MCP 边界)、foundation(workspace 目录树 + 全代码块 Rust)、README/docs·README、14 篇模块/harness/assembly/protocol 文档代码块 Rust 化(协议 wire 值保留 snake_case,everos-analysis 外部引用加注保留);CI 改 cargo 验证链 + taiki-e 装 llvm-cov 把关覆盖率 + `cargo xtask check-docs` 查断链;原 `tools/check_doc_links.py` 重写为 Rust xtask(全仓无 Python)。

- 2026-07-05 **foundation config/errors 重设计**(复核后发现两处不足)。① `errors.py`:从空异常类升级为携带信息的错误——基类 `KairosError(message, *, details)`;`ProviderError` 增 `provider`/`retryable`/`cause`(满足 model-gateway §3 重试判定与底层异常封装);`NotConfiguredError` 增 `hint` 配置指引;② `config.py`:补 **TOML 配置文件层**,双层级联项目盖全局——项目 `./.kairos/config.toml` 覆盖全局 `~/.kairos/config.toml`(同 `.kairos/` 命名空间,`.gitignore` 改 `.kairos/*` + `!.kairos/config.toml` 放行配置、忽略运行时数据);各作用域共用同一 KairosSettings schema,字段天然一致;加载优先级 环境变量 > .env > 项目 config > 全局 config > 默认;openai_compat 用户配 base_url+model+api_key_env 即可接自有模型,零改码。模型(ChatModel)配置按决策留给 model_gateway 任务;③ **ADR 0018**(配置文件用 TOML):经 web 核实两家一流 Agent 格式分歧——Claude Code=JSON(三级级联)、Codex=TOML,按"给人手改的阈值/provider 配置需注释自解释"这一主场景选 TOML(与 Codex 同侧,诚实记录放弃 JSON 的 IDE-Schema 补全),含来源链接;更新 ADR README、docs/README ADR 摘要;④ `factory.py` 未知 impl 错误改用 `hint` 承载已注册清单;⑤ 补 `tests/unit/test_config.py`(文件分层/优先级)+ test_foundation `TestErrors`,共 27 项全过,foundation 覆盖率≥91%;⑥ 文档:foundation.md 配置管理(加载优先级+TOML+双层路径)、错误处理(新签名)、结构化日志(对齐 get_logger、去除不存在的 ctx.trace_id)同步。
- 2026-07-05 **Phase 2 起步:foundation tenancy/logging/factory 落地**。① `tenancy.py`:`TenantContext(tenant_id, user_id)` frozen+slots dataclass,构造期空作用域 fail-closed(ADR 0009);② `logging.py`:`StructuredFormatter`(单行 JSON)+ `configure_logging`(幂等)+ `get_logger`,标注不落内容明文/密钥红线;③ `factory.py`:通用 `Registry[T]` 实现注册表(impl 名→构造器,重复注册抛 ConfigError、未知 impl 抛 NotConfiguredError,ADR 0011);④ `config.py`/`errors.py` 已于 Phase 1 骨架完成,本轮未改;⑤ 补 `tests/unit/test_foundation.py`(17 项全过,foundation 覆盖率≥91%);⑥ 文档对齐:`foundation.md` 目录 `registry.py`→`factory.py`、tenancy 草案补 fail-closed 守卫、tracing/types 标注后续任务落地。tracing.py/types.py 按 YAGNI 暂不落地。
- 2026-07-05 **项目统一重构(V1→V2 六层架构)**:以 V2 六层架构为唯一事实源,全面取代旧三层表述。① `V2/docs/` 全部并入统一 `docs/` 树(protocol/harness/assembly/verticals + model-gateway/tools/knowledge/observability/eval),删除 `V2/`;② 落实 S16 演练三处回改(context §5.1 scope 推断、loop 文本即 FINISHED、SessionMeta.scope + set_session_scope);③ **S10 memory 四件套定稿**:接口首参 `ctx: TenantContext`、DTO 零租户字段、租户物理分表 `{tenant_id}__{kind}`、MetadataFilter 等值下推、MemorySource 写入来源、通用 scope metadata(去 namespace 列/tags)、按 namespace 独立淘汰、procedural 生产者定为 harness/distill;④ ADR 0010–0017 建档(认证/模型契约归属/TenantContext 显式传参/租户物理分表/六层命名+import-linter/向量存储上提/subagent 为工具/scope 推断),0006/0007/0008 加术语更新追记、0009 加物理实现更新追记,重建 ADR 索引;⑤ 全仓旧三层术语替换为六层;规范文件(README/AGENTS/PROGRESS)改写,AGENTS 增命名硬规则 + 常用命令;⑥ roadmap 改为 Phase 1–5。

<!-- 历史变更(V1 阶段,2026-06-27)见 git log;为保持 PROGRESS 聚焦当前,不在此逐条保留。 -->
