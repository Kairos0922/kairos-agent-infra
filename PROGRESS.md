# 项目进度 (PROGRESS)

> 本文件是 Kairos 项目**唯一的进度事实源**,由 Code Agent 实时维护。
> 规则见 [CLAUDE.md](./CLAUDE.md):任务开始标"进行中",完成标"完成"并在变更记录追加一行;新任务/范围变化先报用户批准。
> 状态图例:`[ ]` 未开始 · `[~]` 进行中 · `[x]` 完成 · `[!]` 阻塞(注明原因)

## 当前阶段:阶段一 MVP(底座 + 记忆模块 + benchmark)

范围与设计依据见 [docs/project/roadmap.md](./docs/project/roadmap.md)。设计研讨结论见 ADR 0004(不做图)、0005(衰减/删除分离)、0006(认知功能分类)、0007(机制/策略分离与选择性召回)、0008(procedural 评估/提炼解耦)与 [benchmark 子项目](./docs/modules/benchmark/README.md)。

### 工程化骨架

- [x] 初始化 `pyproject.toml`(依赖、ruff、mypy、pytest-cov、import-linter 配置)
- [x] 创建 `src/kairos/` 目录骨架(foundation / modules/memory / adapter)
- [x] 创建 `tests/` 骨架(unit / contracts / integration / conftest)
- [x] 配置 CI:lint + 类型检查 + 测试 + import-linter 依赖方向检查(GitHub Actions 验证链,push/PR 到 main 触发,已跑通)

### 底座 (foundation)

- [x] 配置机制 `KairosSettings`(实现选择走配置,密钥走环境变量,dim 一致性校验)
- [x] 统一错误层级 `errors.py`(KairosError / ConfigError / ValidationError / ProviderError / NotConfiguredError)
- [ ] 结构化日志 `logging.py`
- [ ] trace 接入点 `tracing.py`(默认 no-op)
- [ ] 模块注册机制 `registry.py`
- [ ] 跨模块基础类型 `types.py`
- [~] import-linter 依赖方向规则成文并接入 CI(底座边界契约已激活;模块级契约待对应包创建后激活)

### 记忆模块 — 抽象与实现 (modules/memory)

- [x] 模块内抽象 `contracts/`:VectorStore / EmbeddingProvider / RerankProvider / Tokenizer
- [ ] 契约测试套件(任何实现必须通过)
- [ ] LanceDB 实现 `providers/vector/lancedb_store.py`
- [ ] embedding 实现:`openai_compat` + `sentence_transformer`
- [ ] rerank 实现:`cross_encoder` + `http_rerank`
- [ ] jieba tokenizer 实现
- [ ] 配置驱动工厂 `providers/factory.py`

### 记忆模块 — 领域逻辑

- [ ] 共享 `MemoryBase` schema + 三类 kind schema(`models.py`)
- [ ] 统一写入管线(校验→冲突决策→分词→embed→hash→upsert)
- [ ] semantic:显式写入 + 抽取钩子 + LLM 驱动 ADD/UPDATE/DELETE/NOOP + recency 降权
- [ ] episodic:显著性门控追加 + recency/salience 降权 + 归档 + forget_session + 晋升钩子
- [ ] procedural:已提炼经验写入口(去重)+ reinforce 回调 + 强度衰减/低效淘汰(评估/提炼在模块外,ADR 0008)
- [ ] **模块外**:procedural 经验占位生产者(规则门控 + LLM 抽取 → write_experience,ADR 0008)
- [ ] 后台维护任务(optimize + episodic 归档/衰减 + procedural 强度衰减)

### 记忆模块 — 检索层 (retrieval)

- [ ] 向量召回 + BM25 召回 `recall.py`
- [ ] RRF 融合 `fusion.py`(纯计算,同步)
- [ ] 可选 rerank 接入
- [ ] recency/strength 降权排序(衰减管排序,ADR 0005)
- [ ] 方法→管线路由 + 检索编排 `searcher.py`
- [ ] 选择性召回 `RecallRouter`(MVP 薄启发式)+ memory-as-a-tool 暴露(ADR 0007)

### Benchmark 子项目 (modules/benchmark)

- [ ] 场景本体 + 用户属性本体(垂直已定:个人助理 + 教育助手;属性本体待细化)
- [ ] harness 框架代码(通过记忆模块对外接口喂入/检索/打分)
- [ ] 小规模中文种子集(几十题,覆盖 IE/MR/KU/TR/ABS)
- [ ] 评测指标实现:Precision@K / Recall@K / nDCG、abstention 正确率、distractor 衰减曲线
- [ ] 写入/检索分离归因(固定 reader 四条件)
- [ ] 数据集迭代扩充

### 适配层 (adapter)

- [ ] 对外 DTO `dto.py`(与领域模型隔离)
- [ ] `MemoryAdapter`(remember / recall(含 RecallRouter)/ forget_session / write_experience / reinforce / maintain)+ memory 工具暴露
- [ ] 错误翻译(内部错误 → 对调用方有意义的形式)

### 验证与文档

- [ ] **验证 spike**:LanceDB FTS OR-mode 实现确认(见 [retrieval](./docs/modules/memory/retrieval.md))
- [ ] 端到端集成测试(真实 LanceDB + 本地模型,写入→检索)
- [x] 设计文档集(project / foundation / modules/memory / modules/benchmark)
- [x] 项目规范(README / CLAUDE / AGENTS / PROGRESS)
- [x] ADR 0001-0008(LanceDB / RRF / 抽象归模块 / 不做图 / 衰减删除分离 / 认知功能分类 / 机制策略分离与选择性召回 / procedural 评估提炼解耦)

## 变更记录

- 2026-06-27 创建 PROGRESS.md;完成设计文档集、项目规范文件、首批 ADR。
- 2026-06-27 搭建工程化骨架:pyproject.toml(uv 管理)、src/tests 目录、底座 config/errors、记忆模块 contracts 抽象、骨架冒烟测试。验证链全绿(ruff format/lint、mypy strict、import-linter、pytest 覆盖率 100%)。
- 2026-06-27 git 仓库初始化并推送到 GitHub(Kairos0922/kairos-agent-infra,public);接入 GitHub Actions CI 验证链,已跑通(18s 全绿)。
- 2026-06-27 升级 CI actions 消除 Node 20 弃用告警(checkout v7、setup-uv v8.2.0),CI 全绿零告警;origin 切换为 SSH(https 通道网络不通)。
- 2026-06-27 记忆模块设计研讨定案并更新文档:新增 ADR 0004(不做知识图谱)、0005(衰减管排序/冲突管删除);新增 benchmark 一等子项目文档(协议 + 中文数据集规范);修订 memory-types/retrieval/tradeoffs(LLM 驱动 ADD/UPDATE/DELETE、三类衰减分化、记什么)。下一步:benchmark 场景本体需业务输入;或进入记忆模块 models 实现。
- 2026-06-27 垂直化研讨定案:确立"基座+垂直分层"(教育助手=个人助手+教育垂直),infra 保持通用、垂直靠 namespace/tags/应用层叠加,不 fork schema、暂不加 subject_id。更新 memory-types(垂直化分层节 + tags 字段 + 教育例 + 记忆/领域知识边界)、tradeoffs(为什么不 fork schema/暂不加 subject_id)、benchmark dataset/README(场景本体定两类垂直、教育领域知识不进记忆数据集)、roadmap/PROGRESS。
- 2026-06-27 记忆分类轴重审定案:新增 ADR 0006(按认知功能分类)。从 personal/session/experience 改为两级分类——工作记忆(context 压缩)归应用/适配层,长期记忆按认知功能分 semantic/episodic/procedural。关键澄清:旧 session 混装"工作缓冲(应用层)+情景召回(infra)",拆开后 episodic 解开"短命"耦合,重定位为工作缓冲的无损可检索后备(显著性门控写入 + recency/relevance 兜底检索)。经联网核源(CoALA 2309.02427 等 6 篇 arXiv 全部核实 + 反方观点)。全量改名同步:memory-types/retrieval/api/tradeoffs/两个 README/everos-analysis/benchmark dataset/roadmap/PROGRESS;ADR 0004/0005 在新轴下重新表述。
- 2026-06-27 记忆时机研讨定案:新增 ADR 0007(机制/策略分离 + 写入分 kind + 选择性召回)、0008(procedural 评估/提炼与记忆模块解耦)。确立组织原则"记忆模块=机制,何时存/何时召回/什么值得记=策略(在模块外)";召回从全量改为选择性 + 可插拔 RecallRouter(MVP 薄启发式)+ memory-as-a-tool;procedural 的 trace 评估/提炼移出模块,模块只收已提炼经验,MVP 占位生产者在模块外(distiller/trace_schema 从模块目录移除)。经联网核源(Self-RAG/Adaptive-RAG/FLARE/UAR、Lost-in-the-Middle/Power-of-Noise/Context-Rot、LangMem/Mem0、Letta sleep-time/Auto-Dreamer/ExpeL/observability、Storage-Is-Not-Memory/LLM-as-judge)。同步:新增两 ADR + 索引;改 memory-types/retrieval/api/tradeoffs/foundation/memory README/根 README/overview/roadmap/PROGRESS。
