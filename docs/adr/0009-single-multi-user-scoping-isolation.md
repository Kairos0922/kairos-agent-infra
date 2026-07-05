# ADR 0009:单/多用户记忆作用域与隔离(隔离是机制,共享是策略)

- **状态**:已接受
- **日期**:2026-06-27
- **相关文档**:[modules/memory/memory-types.md](../modules/memory/memory-types.md)、[modules/memory/retrieval.md](../modules/memory/retrieval.md)、[modules/memory/api.md](../modules/memory/api.md)、[modules/memory/tradeoffs.md](../modules/memory/tradeoffs.md)
- **上位关系**:延续 [ADR 0007](./0007-memory-mechanism-vs-policy-timing.md) 的机制/策略分界(隔离=机制、共享=策略);procedural 的全局共享依赖 [ADR 0008](./0008-procedural-evaluation-decoupling.md) 的模块外评估/脱敏 pipeline;作用域字段沿用 [ADR 0006](./0006-memory-classification-by-cognitive-function.md) 已有的 `owner_id`/`namespace`/`tags`,不新增。

> **物理实现更新(2026-07,S10 租户化重审后)**:本 ADR 结论 §2、理由 §2 中选择的"**单表 + `(namespace, owner_id)` pre-filter**"物理存储方案,已被 [ADR 0013](./0013-lancedb-tenant-physical-tables.md) 更新为"**租户物理分表 `{tenant_id}__{kind}` + 表内 `owner_id` 过滤**"(合规删除刚需)。本 ADR 的**隔离原则不变**——强制作用域、fail-closed(缺则拒绝)、作用域从可信上下文派生、跨 owner 隔离落契约测试;仅"租户轴的物理落地方式"由 ADR 0013 取代,`namespace` 字段随之退化为表名维度(不再是表内列)。术语上"作用域从可信上下文派生"的载体明确为 `ctx: TenantContext`([ADR 0012](./0012-tenant-context-explicit-passing.md))。

## 背景

记忆模块要同时支持**单用户**(单机个人助理)与**多用户**(一个部署服务多个用户,如多教师)。研讨"要不要为两者分别设计"时,确立一个判断:**不分两套**。单用户是多用户的**退化情形**——单机单用户时所有记忆落在 `(namespace=default, owner_id=该用户)`,多用户只是这两个字段取不同值。这与项目的"基座+垂直分层"同构:不为单用户砍功能,也不为多用户加平行结构。

真正要回答的是两件**不同性质**的事,必须分开:

- **隔离**:用户 A 永远不能看到用户 B 的记忆。这是**安全底线**,确定性、可测——属**机制**。
- **共享**:把一个用户/agent 学到的经验跨用户复用(尤其 procedural)。这是**业务判断**,按场景演进、需配脱敏——属**策略**。

把二者混谈会导致两个错误:要么为了"将来可能共享"而弱化隔离(留下泄漏面),要么为了"先不做多用户"而连隔离断言都省掉(等真多用户时安全边界已无处补)。

## 候选方案

1. **单/多用户分两套设计**:被否决——多用户结构是单用户的超集,分两套违背"基座+分层"且制造重复。
2. **本阶段连隔离都不做**(反正 Non-goal 是"不做多租户"):被否决——"不做完整鉴权/配额"不等于"不做隔离断言"。跨租户泄漏对应 OWASP LLM02/LLM08,根因是"一个缺失的过滤器",后补成本极高,且能廉价转成契约测试。
3. **隔离做对(机制,现在)+ 共享留后(策略,阶段二)(选定)**:用一套作用域机制覆盖单/多用户;强制隔离 prefilter + 契约测试现在就落;跨用户共享与脱敏随 [ADR 0008](./0008-procedural-evaluation-decoupling.md) 的评估 pipeline 在阶段二做。

## 结论

### 1. 一套作用域,两个正交轴,复用现有字段

| 轴 | 字段 | 语义 | 单用户取值 |
|----|------|------|-----------|
| 租户/组织硬边界 | `namespace` | 多租户/多应用分区 | `default` |
| 实体归属 | `owner_id` | user 或 agent | 该用户 |
| 自由维度 | `tags` | 垂直作用域过滤(不参与隔离) | 按需 |

**不新增** `org_id`/`tenant_id`——`namespace` 承担租户边界;`subject_id` 仍按 [tradeoffs](../modules/memory/tradeoffs.md#为什么不给垂直-fork-schema暂不加-subject_id) 缓议。这与业界主流"标签/命名空间作用域"范式一致(Mem0 实体标签、LangGraph namespace 前缀树、Letta block 挂载)。

### 2. 隔离是机制,现在做对(三条硬规则)

1. **强制作用域 prefilter**:检索在 `Searcher`/facade 层强制注入 `(namespace, owner_id)` 过滤(LanceDB `where(...)` pre-filter,检索前缩小工作集)。
2. **fail-closed(默认拒绝)**:缺少有效作用域时**拒绝查询**(抛 `ValidationError`),**绝不返回全量**。"绝不无作用域查询"是铁律。
3. **作用域从可信上下文派生**:`namespace`/`owner_id` 由上层从可信会话上下文给出,**检索层不信任调用方自报的越权标识**;隔离断言落在确定性的检索层,不依赖 LLM。

> 这**不等于**完整鉴权/RBAC/配额(仍是 Non-goal)。本阶段只做"检索必带作用域、缺则拒绝"这条轻量隔离断言 + 契约测试;鉴权/配额留阶段三。

### 3. 三类记忆的 owner 语义与可共享性

| kind | owner 语义 | 本阶段 |
|------|-----------|--------|
| **episodic** | `owner_id=user_id`,namespace 内严格私有 | 仅私有 |
| **semantic** | `owner_id=user_id`,私有("领域/团队事实"不是记忆,归阶段二上下文模块,见 [ADR 0006](./0006-memory-classification-by-cognitive-function.md)) | 仅私有 |
| **procedural** | **显式二选一**:私有 `owner_id=f(agent_id,user_id)` / 全局技能 `owner_id=agent_id`(namespace 内跨用户共享) | **仅实现私有路径**;全局路径留接口位 + TODO |

**procedural 默认 per-(agent,user) 私有,全局共享为显式 opt-in**。理由:① default-deny 的延伸;② 全局共享必须先**去标识化**(否则把私密提炼进共享池),而去标识是**模块外评估 pipeline 的职责**([ADR 0008](./0008-procedural-evaluation-decoupling.md))——记忆模块只存"已脱敏经验"。因此全局共享天然依赖那条尚未建设的 pipeline,本阶段做私有更干净。

### 4. 明确不做(YAGNI)

- **组织/全局三层继承式检索**(查询时合并 个人+组织+全局):业界无成熟范式,靠应用层多查询合并,"防下层污染上层"无公认算法。本阶段不做,但 `namespace` 层级化前缀(未来 `org:school_a`)不堵死。
- **跨用户 PII 脱敏共享池**:依赖 [ADR 0008](./0008-procedural-evaluation-decoupling.md) 的评估 pipeline,阶段二随它一起。

## 理由

**1. 业界三大框架都靠"标签/命名空间作用域 + 检索期过滤",默认隔离、显式共享**:

- **Mem0** Entity-Scoped Memory:`user_id`/`agent_id`/`app_id`/`run_id` 四维实体标签;**默认隐式 null 隔离**(只传 user_id 则结果限定在其他维度为 null 的记录);跨实体须显式通配符/OR。
- **LangGraph/LangMem** namespace 元组(如 `(memories, org, user)`):私有放深层、共享放浅层前缀;**前缀匹配**实现跨层检索。
- **Letta** memory block:默认 per-agent 私有;共享 = 同一 `block_id` 显式 attach 到多 agent。

三者共识:**共享是显式动作,默认隔离**——支持本 ADR"私有默认、全局 opt-in"。

**2. 向量库多租户:业界普遍用 pre-filter / 物理分区,绝不 post-filter**:

- **Qdrant** 官方力推**单 collection + payload 分区**(在 tenant 字段建索引,过滤反而加速),仅"租户少且需强隔离"才多 collection。
- **Pinecone**(每租户一 namespace)、**Weaviate**(每租户一 shard)走物理分区,靠冷热分层压成本。
- **LanceDB** 默认 `where(...)` pre-filter,并有原生 Namespaces 支持按域独立 table;官方未钦定多租户模式,但存算分离/空闲表不常驻内存,两种隔离原语都可用。
- 我们的选择:**单表 + `owner_id`/`namespace` pre-filter**(嵌入式单机、租户量不大时运营成本最低、可在 owner 字段建索引加速),物理分表留作未来大租户的备选。详见 [tradeoffs](../modules/memory/tradeoffs.md)。

**3. 跨用户泄漏是被反复验证的真实风险,且可廉价测试**:

- 共享向量索引若无每租户访问控制,跨租户 RAG 泄漏接近"必然";数据进 LLM 后 DB 层 ACL 不再保护。
- OWASP Top 10 for LLM Applications(2025):**LLM02 Sensitive Information Disclosure**、**LLM08 Vector and Embedding Weaknesses** 直接对应。
- 防御共识:隔离在**确定性的检索层服务端强制 + 默认拒绝 + 不信客户端自报**;"依赖 LLM 做访问控制是反模式"。
- 转成契约测试:注入 A、B 两 owner 数据,断言"A 的查询永不返回 B 的记录""缺作用域时拒绝而非返回全量"。

**4. procedural 经验共享有价值但需剥离个体情境**:

- Voyager(arXiv 2305.16291)技能库、ExpeL(arXiv 2308.10144)离线经验池、AWM(arXiv 2409.07429)workflow 记忆,都证明"抽象后的可复用经验"跨任务/跨场景有效。
- 但朴素跨 agent/跨用户直接搬记忆常**降低**性能(把任务知识与个体偏见/私有情境纠缠)——共享的应是**去标识化的 how-to/约束**,非原始 trace。这支撑"全局共享依赖模块外脱敏、本阶段先私有"。

## 反方观点(诚实记录)

- **"既然 Non-goal 是不做多租户,隔离也该缓"**:回应——Non-goal 指"不做完整鉴权/隔离/配额体系",不指"不做隔离断言"。隔离断言是几行强制过滤 + 一组契约测试,成本极低;省掉它等于在数据层留泄漏面,与"高精确率、可信"取向冲突。
- **"教育垂直明明有多教师共享教案的价值,为何不现在做"**:回应——经用户确认,跨教师共享**不是 MVP 硬需求**;且全局共享必须配去标识脱敏(ADR 0008 的 pipeline 尚未建),现在做私有更干净,共享随阶段二一起。
- **"单表 + 过滤的逻辑隔离不如物理分表安全"**:回应——逻辑隔离的风险是"漏加过滤",我们用 fail-closed + 契约测试把这个风险关掉;物理分表在嵌入式单机、租户量不大时运营成本不划算,留作大租户备选。pre-filter 在 owner 字段有索引时还能加速。

## 影响

- **retrieval.md**:新增"作用域隔离"小节(强制 prefilter、fail-closed、作用域来源);`Searcher` 草案体现强制 `(namespace, owner_id)` 参数。
- **memory-types.md**:三类记忆各补 owner 语义/可共享性;procedural 补"owner 二选一、MVP 只做私有、全局留接口位依赖 ADR 0008 脱敏"。
- **api.md**:`RecallRequest`/`WriteRequest` 的 `owner_id`/`namespace` 标为强制作用域;补错误契约(缺作用域 → `ValidationError`)。
- **foundation.md**:契约测试一节加"跨 owner 隔离断言"为必过项。
- **overview.md**:Non-goal "多租户与权限体系"行细化——区分"隔离断言机制(本阶段做)"与"完整鉴权/配额(不做)"。
- **tradeoffs.md**:新增"向量库多租户:为什么单表 + owner prefilter"取舍 + 来源。
- **不改既有机制**:ADR 0004/0005/0006/0007/0008 全部继续成立;本 ADR 只补"作用域与隔离"这一横切维度。

## 依据来源

框架多用户作用域(官方文档,已验证):
- Mem0 Entity-Scoped Memory:https://docs.mem0.ai/platform/features/entity-scoped-memory ;v2 filters:https://docs.mem0.ai/platform/features/v2-memory-filters ;论文 arXiv 2504.19413:https://arxiv.org/abs/2504.19413(注:论文不含 scoping ID 语义,均出自产品文档)
- LangGraph BaseStore / namespace:https://docs.langchain.com/oss/python/langgraph/stores ;LangMem 动态命名空间:https://langchain-ai.github.io/langmem/guides/dynamically_configure_namespaces/
- Letta 共享 memory block:https://docs.letta.com/guides/core-concepts/memory/shared-memory/

向量库多租户(官方文档,已验证):
- LanceDB 过滤/pre-filter:https://docs.lancedb.com/search/filtering/ ;Namespaces:https://docs.lancedb.com/namespaces/usage
- Qdrant 多分区(力推单集合+payload):https://qdrant.tech/documentation/guides/multiple-partitions
- Pinecone 多租户(每租户一 namespace):https://docs.pinecone.io/guides/index-data/implement-multitenancy
- Weaviate 多租户(每租户一 shard):https://docs.weaviate.io/weaviate/manage-data/multi-tenancy

泄漏风险与防御:
- OWASP Top 10 for LLM Applications 2025(AWS 逐条):https://docs.aws.amazon.com/prescriptive-guidance/latest/agentic-ai-security/owasp-top-ten.html ;官网:https://genai.owasp.org/llm-top-10/
- Microsoft 安全多租户 RAG 参考架构:https://learn.microsoft.com/en-us/azure/architecture/ai-ml/guide/secure-multitenant-rag
- AWS 安全多租户 RAG(服务端集中授权):https://aws.amazon.com/blogs/architecture/secure-multi-tenant-rag-with-amazon-bedrock-and-verified-permissions/

procedural 经验共享(论文,已验证):
- Voyager arXiv 2305.16291:https://arxiv.org/abs/2305.16291
- ExpeL arXiv 2308.10144:https://arxiv.org/abs/2308.10144
- Agent Workflow Memory arXiv 2409.07429:https://arxiv.org/abs/2409.07429

> 注:调研中若干 2025–2026 区间 preprint(如 Memp 2508.06433、AGENT KB 2507.06229、Multi-User Memory Sharing 2505.18279,及 26xx 编号若干)作为方向佐证,**权威度待时间检验**,未用于支撑硬结论。效果验证依赖 benchmark(见 [benchmark/protocol](../modules/benchmark/protocol.md))。
