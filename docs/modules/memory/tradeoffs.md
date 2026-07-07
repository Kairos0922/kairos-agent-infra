# 技术取舍与依据来源

本文记录记忆模块相关的关键技术决策,给出"为什么这样选"、备选方案,以及外部事实的依据来源。凡依赖外部事实(LanceDB 能力、EverOS 实现)的论断都标注来源或"待验证"。

> 项目层/跨模块的取舍(如"抽象接口归模块还是底座""共享上提时机")见 [project/overview](../../project/overview.md) 与 [project/roadmap](../../project/roadmap.md)。本文只谈记忆/检索的具体技术。

## 向量库:为什么选 LanceDB(及其适用边界)

### 选定理由

LanceDB OSS 作为**进程内嵌入式**向量库,单一存储同时承载:向量 ANN、BM25 全文、SQL 元数据过滤、原生 hybrid(默认 RRF)+ 可插拔 reranker、Pydantic schema、完整 CRUD + upsert、Lance 格式版本化/time-travel。这套组合对记忆系统高度契合——**一个库覆盖检索层需要的全部底层能力,无需额外拼 ES(全文)+ 向量库**。

| 需求 | LanceDB | 来源 |
|------|---------|------|
| 嵌入式、无独立服务、本地文件存储 | ✅ 已验证 | docs.lancedb.com/storage |
| 向量 ANN(IVF/HNSW 系)、cosine | ✅ 已验证 | docs.lancedb.com/indexing/vector-index |
| 原生 BM25 全文检索 | ✅ 已验证(Lance-native FTS) | docs.lancedb.com/indexing/fts-index |
| 原生混合检索,默认 RRF | ✅ 已验证(默认 `RRFReranker()`) | docs.lancedb.com/search/hybrid-search |
| 可插拔 + 自定义 reranker | ✅ 已验证(`Reranker` 基类) | docs.lancedb.com/reranking |
| Pydantic schema(LanceModel) | ✅ 已验证 | docs.lancedb.com/search/hybrid-search |
| upsert / merge_insert / 条件删除 | ✅ 已验证 | docs.lancedb.com/tables/update |
| SQL where 过滤 + prefilter/postfilter | ✅ 已验证 | docs.lancedb.com/search/filtering |
| 版本化 / time-travel | ✅ 已验证 | docs.lancedb.com/tables/versioning |
| 中文分词(jieba)FTS | ✅ 已验证(内置 jieba tokenizer) | docs.lancedb.com/indexing/fts-index |

### 适用边界与已知限制(必须正视)

| 限制 | 影响 | 对策 | 来源 |
|------|------|------|------|
| **OSS 无自动索引维护**,新数据需手动 `optimize()`,否则走 flat scan 变慢 | 持续写入的记忆系统的**主要运维负担** | 后台维护任务周期 `optimize()`(见 [memory-types](./memory-types.md));约每 10万行/20次写 optimize | docs.lancedb.com/search/full-text-search |
| **无原生 TTL** | episodic 记忆归档/保留窗需自己清理 | 维护任务 `delete(where="expires_at < now")` 或显式 `forget_session` | 文档未见,**已确认无此能力** |
| **FTS 查询串不支持布尔操作符** OR/AND | OR-mode 召回实现方式受限 | 通过 query 构造 API 实现,**【待验证】**具体手段 | docs.lancedb.com/search/full-text-search |
| `index_cache_size` 不设上限可能 FD 泄漏到 EMFILE | 长期运行进程崩溃风险 | 配 `index_cache_size_bytes` 上限(默认 16MB) | 借鉴 EverOS 实测(settings.py) |
| 本地/块存储不跨实例共享 | 不适合多节点共享同一表 | 本阶段单机,符合 Non-goal | docs.lancedb.com/storage |
| 自动 embedding、自动索引、服务端 embedding 是 **Enterprise** 特性 | OSS 要自己管 embedding 与索引 | 我们本就在模块内管 embedding(Provider 抽象),不依赖库的自动特性 | docs.lancedb.com/embedding |

> **结论**:LanceDB 能力与记忆系统需求高度匹配,最大代价是**索引维护要自己调度**。这个代价可控(一个后台任务),换来"单库覆盖全部检索能力 + 嵌入式零运维部署",对单机 MVP 划算。未来转向多节点高并发需重新评估。

### 备选方案

| 备选 | 为什么没选 |
|------|-----------|
| **Chroma** | 嵌入式向量库,但 BM25/混合检索不如 LanceDB 成熟;Lance 列式格式 + time-travel 是额外优势。 |
| **Qdrant / Weaviate / Milvus** | client-server 架构,需独立服务进程,违背"嵌入式单机"取向;运维更重。 |
| **PostgreSQL + pgvector + tsvector** | 一库搞定向量+全文可行,但需 PG 实例(非嵌入式),且 ANN 与全文融合要自己拼,不如 LanceDB 原生 hybrid 顺手。作为"已有 PG 基础设施"场景的备选保留。 |

## 混合检索融合策略:为什么选 RRF

详细对比见 [retrieval](./retrieval.md)。要点:

- **选 RRF**:只依赖排名、跨路天然可比、单参数鲁棒,且是 LanceDB 默认融合器【已验证】。
- **备选-加权分数融合**:保留为接口参数(`weights`),默认不用——cosine 与 BM25 量纲不同,归一化难调。
- **扩展位-LR 校准融合**:EverOS 用 `cosine_to_lr_score` 把 cosine 与 BM25 校准到同一概率尺度再比较【来源:EverOS `hierarchy.py`,但算法实现在私有包 `everalgo-rank`,**内部实现拿不到**】。更精细,列为后续扩展——RRF 已足够覆盖 MVP,且实现透明可测。

> **取舍哲学**:本阶段优先**透明、可测、鲁棒**的标准方法(RRF),而非精细但黑盒的方法(LR 校准)。等真实数据证明 RRF 不够用再上更复杂融合——避免过早优化。

## 记忆分类:为什么按认知功能分

完整决策见 [ADR 0006](../../adr/0006-memory-classification-by-cognitive-function.md)。要点与依据:

**两级分类**:先按生命周期切出工作记忆(归 harness 层 Context Engine)vs 长期记忆;长期记忆内部按**认知功能**分 semantic/episodic/procedural。

| 候选分类轴 | 为什么不选作主轴 |
|-----------|----------------|
| 按场景/垂直 | 垂直是内容差异不是结构差异,靠通用 scope metadata + Profile 表达(见下文) |
| 按归属(用户/会话/agent) | 归属是字段 `owner_id`;同一归属者可同时有事实/事件/技能 |
| 按生命周期(短/长) | 生命周期是策略(TTL/衰减);拿它当主轴正是旧 `session` 把"短命"焊进分类的病根 |
| **按认知功能(选定)** | 让写入/检索/淘汰/衰减四类行为聚类最干净——这正是分 kind 的根本动机 |

**依据与诚实边界**:认知三分法源自认知科学(Tulving 1972 的 episodic/semantic;Squire 的 procedural),在 language-agent 语境由 CoALA(arxiv 2309.02427)整合;LangMem 直接照此三分。但它**不是无争议的公认标准**:Letta/LangChain 称其为"borrowed from cognitive science",且把生命周期当主轴、认知功能仅作长期记忆的子分类;另有 preprint 批评认知切分会诱导"拿衰减删事实"的 category error。

**我们的应对**:① 采用 LangChain 式的更稳妥形态——生命周期为第一级、认知功能仅在长期记忆内细分;② category error 由 [ADR 0005](../../adr/0005-decay-ranking-conflict-deletion.md)"衰减管排序、冲突管删除"从根上规避;③ 替代轴(归属、形态、生命周期)在我们方案里没丢,只是降级为字段或策略,不当主轴。

> 来源汇总见本文末"记忆分类(认知功能)"小节。

## 记忆时机:机制在内,策略在外

完整决策见 [ADR 0007](../../adr/0007-memory-mechanism-vs-policy-timing.md)(写入/召回时机 + 选择性召回)与 [ADR 0008](../../adr/0008-procedural-evaluation-decoupling.md)(procedural 评估/提炼解耦)。要点与依据:

**组织原则**:记忆模块只提供机制(存、检索、去重、衰减排序);"何时存、何时召回、什么值得记"是策略,留在模块外(harness 层)。

**召回:选择性,不每轮全量。** 全量召回被上下文污染证据证伪——Lost in the Middle(arXiv 2307.03172)、The Power of Noise(arXiv 2401.14887,高分但无关的近似干扰项主动降质)、Context Rot(Chroma)。选择性召回的门控依据:Self-RAG(2310.11511)、Adaptive-RAG(2403.14403)、FLARE(2305.06983)、UAR(2406.12534)。落地为可插拔 `RecallRouter`(MVP 薄启发式)+ memory-as-a-tool 暴露。

**写入:分 kind,不每轮无差别写。** semantic 信息定型时异步抽取、episodic 近实时门控保原始、procedural 经评估才落库。依据:LangMem 三档写入(hot-path/background/delayed 防抖)、Mem0 调用驱动。

**procedural 评估/提炼解耦。** 把"trace 评估、提炼成经验"放模块外(六层下即 harness/distill 管线),记忆模块只收已提炼经验。依据:Letta sleep-time compute(arXiv 2504.13171,主/后台 agent 异步,跨查询成本降约 2.5×)、Auto-Dreamer(arXiv 2605.20616,获取与整合解耦)、ExpeL(arXiv 2308.10144,离线跨任务提炼)、observability 平台(LangSmith/Langfuse/Phoenix)trace+评估独立于存储。诚实张力:*Storage Is Not Memory*(arXiv 2605.04897)警示摄入端加工——以"保留原始 run 兜底(source_run_ids)+ 提炼在模块外可重跑"缓解;LLM-as-judge 不可靠(arXiv 2412.12509 等)——评估不盲信单次打分,以 success 信号兜底。

> 来源汇总见本文末"记忆时机(ADR 0007/0008 依据)"小节。

## 多用户隔离:租户物理分表 + 用户 owner 过滤(ADR 0009/0013)

完整决策见 [ADR 0009](../../adr/0009-single-multi-user-scoping-isolation.md)(作用域与隔离机制)与 [ADR 0013](../../adr/0013-lancedb-tenant-physical-tables.md)(LanceDB 租户物理分表)。要点与依据:

**单用户是多用户的退化情形,共用一套作用域**(租户边界 + `owner_id` 实体),不为单用户砍功能、不为多用户加平行结构。两轴物理实现分工不同:

| 轴 | 实现 | 隔离强度 |
|----|------|---------|
| **租户轴** | 物理分表 `{tenant_id}__{kind}` | 物理隔离,越权需拿错表名 |
| **用户轴** | 表内 `owner_id` 强制 prefilter | 逻辑隔离(provider 内部强制注入) |

**LanceDB 租户隔离两候选(ADR 0013 已裁决选 A)**:

| | 候选 A:租户物理分表(**选定**) | 候选 B:共享表 + 过滤字段 |
|---|---|---|
| 结构 | 表按 `{tenant_id}__{kind}` 拆分;user_id 表内过滤 | 全租户共享 `{kind}` 表;tenant/user 均过滤字段 |
| 隔离强度 | 租户级物理隔离 | 纯逻辑,一处过滤 bug 即跨租户泄漏 |
| 合规删除 | **drop 表即完成**(教育行业数据删除刚需) | 按条件删除,需校验彻底性 |
| 运维 | 表数 = 租户数×3,LanceDB 表即目录,数百租户无压力 | 单表简单,但大小租户互相影响 compaction |
| 索引/淘汰 | 每租户独立,天然按表执行 | 需带条件执行,复杂 |

**选 A 的理由**:合规删除的便利在教育行业价值很高(教育数据删除请求是刚需),代价(表数量随租户线性增长)在数百租户量级可忽略。B 记为被否方案。用户轴仍用 `owner_id` 逻辑过滤——量大、增删频繁,逻辑过滤成本最低。

**逻辑隔离的风险是"漏加过滤",用机制关掉它**:① `owner_id` 过滤在 provider 内部强制注入(不是可选的业务 `metadata_filter`);② fail-closed(缺 ctx/owner 则拒绝,绝不全量);③ 隔离三连落契约测试。业界共识:隔离在确定性的检索层服务端强制、默认拒绝、不信客户端自报,"依赖 LLM 做访问控制是反模式"。对应 OWASP LLM02 / LLM08。

**TenantContext 显式传参,不用隐式全局态(ADR 0012)**:`ctx: &TenantContext` 作为每个读写/检索接口的显式首参,而非藏进 tokio task-local / 线程局部隐式态。理由=**契约可见**(签名即声明"这是租户隔离的操作")、**可被契约测试覆盖**(测试直接构造 ctx 传入)、**无隐式全局态**(避免 async 上下文传播的隐蔽 bug 与跨请求串味)。代价是签名更长,可接受。

> **三类记忆可共享性不同**(ADR 0009):episodic/semantic 私有;procedural 的 owner 是"私有/全局二选一",本阶段仅私有——全局共享必须先去标识(否则把私密提炼进共享池),而去标识是 harness/distill 的职责(ADR 0008),随阶段二一起。

> 来源汇总见本文末"多用户隔离(ADR 0009 依据)"小节。

## 记忆建模:为什么不做知识图谱(MVP)

完整决策见 [ADR 0004](../../adr/0004-no-knowledge-graph-mvp.md)。要点与依据:

**选原子事实 + 向量 + LLM 驱动 ADD/UPDATE/DELETE,不做实体关系图。** 依据来自一手调研:

| 维度 | 知识图谱(Mem0g/Zep/Cognee)的真实表现 | 来源 |
|------|----------------------------------------|------|
| 检索精确率 | 仅**多跳推理 + 时序推理**有明确增益;**单跳事实检索无益甚至略降** | Mem0 论文 arxiv 2504.19413(Mem0g 整体仅 +2%,单跳掉分) |
| token 成本 | **翻倍**(Mem0 ~7k → Mem0g ~14k;Zep 极端到 600k) | 同上 |
| 写入延迟 | 显著上升(实体抽取 + 关系生成 + 冲突消解每步都是 LLM 调用) | 同上 / Zep arxiv 2501.13956 |
| 调优复杂度 | 表面积巨大(Cognee 默认 vs 调优正确率 0.476 → 0.815) | Cognee 基准 cognee.ai/blog |
| 不可替代的硬能力 | **时序事实追踪**(bi-temporal,谁现在在哪工作 vs 历史) | Zep arxiv 2501.13956 |

**结论**:图唯一不可替代的是时序事实追踪,但本项目用更轻的 LLM 驱动 UPDATE/DELETE 覆盖大部分事实变更场景(见 [memory-types](./memory-types.md))。在 benchmark 证明扁平方案在 KU/TR 能力上不足之前,不引入图的全局代价。图作为后续叠加层。

## 记忆衰减:为什么衰减管排序、冲突管删除

完整决策见 [ADR 0005](../../adr/0005-decay-ranking-conflict-deletion.md)。要点与依据:

**衰减只用于检索时降权(soft re-rank,不删);删除由语义冲突或生命周期触发。** 依据:

- **Mem0 Memory Decay(2026-05)明确"nothing gets deleted"**,按 recency + 访问频率把 relevance 缩放到 0.3×~1.5×(来源:mem0.ai/blog)。
- **Generative Agents** 的 recency(指数衰减因子 0.995)也只用于检索打分(来源:arxiv 2304.03442)。
- **MemoryBank** 用 Ebbinghaus R=e^(−t/S),回忆则强度增大(来源:arxiv 2305.10250)——这是程序记忆"强度衰减 + 使用强化"的依据。
- **核心教训**:时间久 ≠ 该删(用户生日不因久未提及而失效)。把时间衰减当删除依据会误删正确的稳定事实,直接违背高精确率目标。

三类记忆据此分化:语义记忆靠冲突更新(recency 仅轻微降权)、情景记忆按 recency+显著性降权并可归档、程序记忆用强度衰减 + 使用强化。

## 为什么不给垂直 fork schema、暂不加 subject_id

Kairos 当前要支持两个垂直:**个人助理**和**教育助手(教师的教学设计/课程设计助手)**,且要可扩展。一个自然但错误的冲动是给每个垂直加专属字段或专属表。我们不这么做。

**分层组合,而非分叉。** 个人助手是基座,垂直 = 基座 + 内容扩展:`教育助手 = 个人助手 + 教育垂直`。落到记忆模块:

| | 做法 | 理由 |
|---|---|---|
| **垂直差异** | 体现在**内容**(写什么事实)、通用 scope `metadata`、Profile/Skill/persona 与 benchmark | 差异本质是内容差异,不是结构差异 |
| **schema** | 三类记忆 + 通用 `category` + scope `metadata`,**跨垂直不变** | 给每垂直加字段 → schema 随业务膨胀,违背解耦与 YAGNI |
| **扩展第三个垂直** | infra 零改动,只在 assembly 层叠内容(Profile/Skill/知识包) | "可插拔/可扩展"在代码层面的兑现 |

**暂不加 `subject_id`。** 曾考虑给基类加一个"记忆主体"字段(默认 = owner_id),应对"教师的记忆多是关于学生"的情形。但经研讨,教育助手定位是**教师的教学设计助手**,不是学生档案系统——记忆主体绝大多数就是教师本人("教高一数学""所带班几何弱"),"主体≠拥有者"不构成主线需求。班级/学科/年级用 scope `metadata` 表达即可。

> **触发条件**:`subject_id` 的真正触发场景是"以单个学生为主体长期追踪学情"。它**不违背 ADR 0004**(主体之间无边、不做多跳,只是与 owner_id 同级的扁平作用域字段),但当前没有这个需求,提前加就是过度设计。等需求出现,再补一条 ADR 引入——这正是 YAGNI 的"共享/字段按需上提"原则。

> **记忆 vs 领域知识的边界**:教材、课标、知识点体系等**静态外部参考数据不进记忆模块**,归 [knowledge 模块](../knowledge.md)。记忆只记"关于用户的/会话的/从执行学到的"且积累出来的内容。详见 [memory-types](./memory-types.md#垂直化基座--垂直分层)。

## 本地 vs 远程模型(embedding / rerank)

**不做非此即彼的选择,用抽象接口让两者都可选、可切换。**

| | 本地模型 | 远程 API |
|---|---|---|
| embedding | sentence_transformer(进程内) | openai_compat(OpenAI/DeepInfra/...) |
| rerank | cross_encoder(进程内) | http_rerank(Cohere/Jina/百炼...) |
| 优点 | 无网络依赖、无 per-call 成本、数据不出域 | 质量可能更高、无需本地算力、易扩容 |
| 缺点 | 占本地算力/内存、质量受模型大小限 | 有延迟/成本/限流、数据出域 |

**关键洞察**(借鉴 EverOS):OpenAI 兼容协议是"本地与远程的最大公约数"——本地 vLLM/Ollama 也讲 OpenAI 协议。所以 `openai_compat` 一个实现 + 不同 `base_url` 就覆盖"远程 API"和"本地自托管服务"两种部署,无需为每家厂商分叉。纯进程内模型(无 HTTP)才单独做 `sentence_transformer`。

> **为什么不替用户决定?** 本地 vs 远程是**部署决策**,取决于数据合规、算力预算、质量要求——infra 不该替用户拍板。infra 的职责是提供干净抽象,让这个决策变成一行配置(`embedding.impl`)。这正是可插拔设计的价值。

## 其它取舍速览

| 决策 | 选择 | 理由 | 备选 |
|------|------|------|------|
| 存储事实源 | LanceDB 单一存储 | 避免 EverOS 式 md+索引双写与 cascade 同步复杂度;infra 定位无需人类直读 | EverOS 的 Markdown 事实源 + 派生索引 |
| 检索算法位置 | 仓库内、可读可改 | 透明、可测、可演进 | EverOS 把算法放私有二进制包 `everalgo-*` |
| 对外 API 形态 | 进程内 async Rust 库(crate 契约) | MVP 单机,零网络开销;server 层预留服务化 | 直接上 HTTP 服务(过早) |
| 分词 | 模块内 jieba,存 `text_tokens` 列 | 换分词器不动 schema/不依赖库内置语言支持 | 依赖 LanceDB 内置 tokenizer |
| trace 评估/提炼位置 | **模块外**(harness/distill 管线) | 评估"经验值不值得记"是策略,不是存储机制;解耦在线/离线节奏(ADR 0008) | 模块内 distiller 一站式 |
| trace 提炼实现(v1) | 规则筛选 + tier=strong 抽取(harness/distill) | 控成本、避免过度设计 | 自动分段 + 反思循环(过重) |
| 召回时机 | 选择性召回 + 可插拔 RecallRouter | 全量召回污染上下文、损精确率(ADR 0007) | 每轮全量召回 |

## 记忆模块的技术风险

(项目级风险见 [project/roadmap](../../project/roadmap.md);此处只列记忆/检索相关。)

| 风险 | 等级 | 说明 | 缓解 |
|------|------|------|------|
| LanceDB 索引维护负担 | 中 | 维护任务失效则检索退化为 flat scan 变慢 | 维护任务可观测;监控 `num_unindexed_rows`;告警 |
| FD 泄漏 / EMFILE | 中 | 长期运行 + 频繁 optimize 耗尽 FD | 配 `index_cache_size_bytes`;压测验证 |
| embedding 模型 / 维度换代 | 中 | 换模型即使**维度不变**,新旧向量也不在同一空间致静默降质;维度变则需重建表 | `MemoryBase.embed_model` 记录溯源,re-embed 触发含"模型不一致"(ADR 0024);维度变走"重建表 + 回填" |
| 经验提炼质量 | 中 | LLM 抽取经验可能噪声大、过时,污染检索 | 规则门控 + 去重 + effectiveness 衰减 + 低效淘汰 |
| FTS OR-mode 实现待验证 | 低-中 | LanceDB FTS 不支持查询串布尔操作符,OR 实现方式未确认 | 落地前 spike 验证;失败则退化为 token 分别查询后合并 |
| 去重阈值难调 | 低 | 0.92 是经验值,过高漏去重、过低误合并 | 设为可配;真实数据校准;记录被合并条目便于回溯 |
| schema 演进迁移 | 中 | per-tenant 物理分表:一次 schema 变更须迁移 N×3 张不同年龄的表 | `MemoryBase.schema_version` 标识版本;新增字段可选 + 默认、按版本增量迁移、失败重入(ADR 0024) |
| per-user 合规删除 | 中 | 软删(`deprecated`)不抹字节,不满足 GDPR / 家长删除;user 是共享表内 `owner_id` 行 | 硬删 by `owner_id` + Lance 版本清理【待验证】;整租户走 `drop_table`(ADR 0013/0024) |
| 静态加密(待确认) | 待定 | 机构 PII 明文存本地卷;若客户合规要求加密而未预留则需返工 | 确认目标机构合规口径;cell-per-tenant 下按机构卷加密 / 密钥隔离(ADR 0022/0024) |

## 依据来源汇总

### LanceDB(官方文档,已验证)

- 存储/嵌入式/并发:https://docs.lancedb.com/storage/index.md
- 向量索引/度量:https://docs.lancedb.com/indexing/vector-index.md
- 全文检索/BM25/增量限制:https://docs.lancedb.com/indexing/fts-index.md 、https://docs.lancedb.com/search/full-text-search.md
- 混合检索/默认 RRF:https://docs.lancedb.com/search/hybrid-search.md
- Reranker:https://docs.lancedb.com/reranking.md
- CRUD/merge_insert/软删除:https://docs.lancedb.com/tables/update.md
- 过滤/prefilter:https://docs.lancedb.com/search/filtering.md
- 版本化:https://docs.lancedb.com/tables/versioning.md
- embedding registry:https://docs.lancedb.com/embedding/index.md
- FTS 规模演示:https://lancedb.com/blog/feature-full-text-search/

### EverOS(实际克隆源码,commit `b7d15f7`)

完整分析见 [everos-analysis](./everos-analysis.md)。关键文件:`docs/how-memory-works.md`、`src/everos/memory/search/{manager,adapter,hierarchy}.py`、`src/everos/component/{embedding,rerank,tokenizer}/protocol.py`、`src/everos/infra/persistence/lancedb/tables/episode.py`。

### 记忆建模 / 衰减(一手论文与官方文档)

- Mem0 *Building Production-Ready AI Agents*:arxiv 2504.19413(Mem0 vs Mem0g 题型分解、token/延迟、ADD/UPDATE/DELETE/NOOP 机制)。
- Zep *A Temporal Knowledge Graph Architecture*:arxiv 2501.13956(bi-temporal、edge invalidation)。
- Cognee 知识图谱记忆基准:cognee.ai/blog(调优表面积、多跳结构化召回)。
- Mem0 *Introducing Memory Decay*:mem0.ai/blog(2026-05,soft re-rank 不删)。
- Generative Agents:arxiv 2304.03442(recency/importance/relevance 三因子,衰减因子 0.995)。
- MemoryBank:arxiv 2305.10250(Ebbinghaus R=e^(−t/S) 强度衰减 + 回忆强化)。

### 记忆分类(认知功能,ADR 0006 依据)

- 认知科学根源:Tulving 1972 episodic/semantic(https://en.wikipedia.org/wiki/Semantic_memory);Squire procedural/declarative(https://en.wikipedia.org/wiki/Procedural_memory)。
- CoALA(language-agent 认知功能分类权威来源):arxiv 2309.02427(https://arxiv.org/abs/2309.02427)。
- 业界实现:MemGPT/Letta arxiv 2310.08560(https://arxiv.org/abs/2310.08560);LangMem 显式三分(https://langchain-ai.github.io/langmem/concepts/conceptual_guide/);Zep episodic/semantic 分离 arxiv 2501.13956;Generative Agents memory stream arxiv 2304.03442;Mem0 arxiv 2504.19413。
- 反方/替代轴:LangChain *How To Give Your Agent Memory*(认知分类为 borrowed,主轴 lifecycle,https://www.langchain.com/blog/how-to-give-your-agent-memory);Letta *Agent Memory*(https://www.letta.com/blog/agent-memory/);*From Human Memory to AI Memory* arxiv 2504.15965;*The Missing Knowledge Layer*(category error 批评,近期 preprint)arxiv 2604.11364。

### 记忆时机(ADR 0007/0008 依据)

- 选择性/自适应召回:Self-RAG arXiv 2310.11511(https://arxiv.org/abs/2310.11511);Adaptive-RAG arXiv 2403.14403(https://arxiv.org/abs/2403.14403);FLARE arXiv 2305.06983(https://arxiv.org/abs/2305.06983);UAR arXiv 2406.12534(https://arxiv.org/abs/2406.12534)。
- 上下文污染/注意力预算:Lost in the Middle arXiv 2307.03172;The Power of Noise arXiv 2401.14887;Redefining Retrieval Evaluation arXiv 2510.21440;Anthropic *Effective context engineering*(https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents);Chroma *Context Rot*(https://research.trychroma.com/context-rot)。
- 写入时机/memory-as-a-tool:LangMem(https://langchain-ai.github.io/langmem/concepts/conceptual_guide/);Mem0 arXiv 2504.19413;MemGPT/Letta arXiv 2310.08560;Anthropic memory tool(https://docs.anthropic.com/en/docs/build-with-claude/tool-use/memory-tool)。
- 获取/整合解耦:Letta *Sleep-time Compute* arXiv 2504.13171(https://arxiv.org/abs/2504.13171);Auto-Dreamer arXiv 2605.20616(近期 preprint);ExpeL arXiv 2308.10144;observability:LangSmith(https://docs.smith.langchain.com/)、Langfuse(https://langfuse.com/docs)、Arize Phoenix(https://docs.arize.com/phoenix)。
- 摄入端加工风险/评估可靠性:*Storage Is Not Memory* arXiv 2605.04897(近期 preprint);LLM-as-judge arXiv 2412.12509 / 2410.20266 / 2403.02839。

### 多用户隔离(ADR 0009 依据)

- 框架多用户作用域:Mem0 Entity-Scoped Memory(https://docs.mem0.ai/platform/features/entity-scoped-memory)、v2 filters(https://docs.mem0.ai/platform/features/v2-memory-filters);LangGraph BaseStore(https://docs.langchain.com/oss/python/langgraph/stores)、LangMem 动态命名空间(https://langchain-ai.github.io/langmem/guides/dynamically_configure_namespaces/);Letta 共享 block(https://docs.letta.com/guides/core-concepts/memory/shared-memory/)。
- 向量库多租户:LanceDB 过滤(https://docs.lancedb.com/search/filtering/)、Namespaces(https://docs.lancedb.com/namespaces/usage);Qdrant 多分区(https://qdrant.tech/documentation/guides/multiple-partitions);Pinecone 多租户(https://docs.pinecone.io/guides/index-data/implement-multitenancy);Weaviate 多租户(https://docs.weaviate.io/weaviate/manage-data/multi-tenancy)。
- 泄漏风险与防御:OWASP LLM Top 10 2025(https://genai.owasp.org/llm-top-10/ ;AWS 逐条 https://docs.aws.amazon.com/prescriptive-guidance/latest/agentic-ai-security/owasp-top-ten.html);Microsoft 安全多租户 RAG(https://learn.microsoft.com/en-us/azure/architecture/ai-ml/guide/secure-multitenant-rag);AWS 服务端集中授权(https://aws.amazon.com/blogs/architecture/secure-multi-tenant-rag-with-amazon-bedrock-and-verified-permissions/)。
- procedural 经验共享:Voyager arXiv 2305.16291;ExpeL arXiv 2308.10144;Agent Workflow Memory arXiv 2409.07429。
- 待检验近期 preprint(未支撑硬结论):Memp 2508.06433、AGENT KB 2507.06229、Multi-User Memory Sharing 2505.18279。

> 注:2605.20616、2605.04897 为 2026 年近期 preprint,作为方向佐证引用,权威度待时间检验。

> 效果验证依赖 benchmark,评测协议见 [benchmark/protocol](../benchmark/protocol.md)。上述论断为现有系统的报告值,Kairos 自身的取舍效果需用自建中文 benchmark 实测。

### 待验证清单(落地前需确认)

- LanceDB 当前版本号与各特性(reranker 类、FTS query API)的最低版本要求(本轮未核 PyPI changelog)。
- LanceDB FTS 的 **OR-mode 查询**具体如何用当前 query API 构造(见 [retrieval](./retrieval.md))。
- 非 S3 对象存储(GCS/Azure)上的并发写原子性(本阶段单机,暂不影响)。
- LanceDB 精确的规模上限/并发上限(文档只给定性描述,无硬数字)。
- EverOS 私有包 `everalgo-*` 的算法内部实现(**不可得**,仅能从调用点推断契约)。

---

下一篇:[everos-analysis](./everos-analysis.md) — EverOS 参考分析。
