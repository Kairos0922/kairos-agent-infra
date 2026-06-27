# ADR 0006:记忆按认知功能分类(工作记忆归应用层,长期记忆分情景/语义/程序)

- **状态**:已接受
- **日期**:2026-06-27
- **相关文档**:[modules/memory/memory-types.md](../modules/memory/memory-types.md)、[modules/memory/tradeoffs.md](../modules/memory/tradeoffs.md)
- **上位关系**:本决策是 [ADR 0004](./0004-no-knowledge-graph-mvp.md)、[ADR 0005](./0005-decay-ranking-conflict-deletion.md) 的上位前提——前者决定"分几类记忆、按什么轴分",后两者在此之上规定每类的建模与衰减策略。

## 背景

记忆模块最初的分类是 `personal / session / experience`。在研讨"短期会话记忆到底有没有必要"时暴露出一个根本问题:**这三类不是沿一条干净的轴切出来的**,而是把三条不同的轴(生命周期、归属、认知功能)碰巧重叠在一起命名:

| 旧 kind | 生命周期轴 | 归属轴 | 认知功能轴 |
|---------|-----------|--------|-----------|
| personal | 长期 | 关于用户 | 事实/语义 |
| session | 短期 | 属于会话 | 情景/事件 |
| experience | 中长期 | 关于 agent | 程序/技能 |

三轴恰好对齐所以"能用",但耦合带来真实的设计困境:`session` 被定义成"短期 + 属于会话",其中"短期"是把**生命周期**焊死进了**分类**——而它在**功能上**其实是**情景记忆**(发生过的事件流),情景记忆并不天然短命(Generative Agents 的 memory stream 就是情景的、长期的)。这导致"为什么需要短期记忆"难以回答:`session` 同时混装了"工作缓冲"(应归应用层的上下文压缩)和"情景召回"(应归 infra 的可检索历史)两件不同的事。

需要确定一条**单一、稳定、可辩护**的分类主轴。

## 候选方案

1. **按场景/垂直分**(个人助理记忆 vs 教育记忆):已被否决——垂直差异是内容差异,不是结构差异,靠 `namespace`/`tags`/应用层表达即可(见 [tradeoffs §为什么不给垂直 fork schema](../modules/memory/tradeoffs.md))。
2. **按归属分**(用户/会话/agent):归属应是**字段**(`owner_id`),不是分类——同一归属者可同时拥有事实、事件、技能三种功能不同的记忆。
3. **按生命周期分**(短期/长期):生命周期是**策略**(TTL/衰减),拿它当分类主轴正是 `session` 翻车的根因。
4. **按认知功能分**(情景/语义/程序):让"写入触发、检索方式、淘汰/衰减"四类操作行为聚类最干净的轴。

## 结论

采用**两级分类**:

- **第一级,按生命周期** 切出 **工作记忆 working memory**(当前 context 窗口,**归应用/适配层**,不是 infra 的存储 kind)vs **长期记忆**(infra 持久化、可检索)。
- **第二级,在长期记忆内部按认知功能** 分三类存储:
  - **semantic 语义记忆** — 关于用户的去情境化事实/偏好("什么是真的")
  - **episodic 情景记忆** — 发生过的对话/事件历史("发生过什么")
  - **procedural 程序记忆** — 从执行 trace 学到的可复用策略("怎么做")

代码标识用英文学界术语 `semantic / episodic / procedural`(符合"标识符英文"规约),文档中文名:语义/情景/程序记忆。

> **与旧命名的映射**:`personal → semantic`、`session → episodic`(解开"短命"耦合)、`experience → procedural`。`session` 中"工作缓冲"的部分上移到应用/适配层的上下文压缩,不再是存储 kind。

## 理由

**1. 认知功能轴让操作行为聚类最干净**(选轴的根本判据——当初分 kind 就是因为不同记忆需要不同的写入/检索/淘汰/衰减):

| 功能类 | 写入 | 检索 | 淘汰/衰减 |
|--------|------|------|-----------|
| working(应用层) | — | 直接在 prompt | 压缩 |
| semantic | LLM 驱动 ADD/UPDATE/DELETE/NOOP 去重 | hybrid,主精确率层 | 冲突删除;recency 轻降权 |
| episodic | append-only + 显著性门控 | recency+relevance,兜底召回层 | 衰减/归档,不靠时长定生死 |
| procedural | trace 蒸馏 + 去重 | hybrid 按 situation 匹配 | 强度衰减+使用强化;低效淘汰 |

四类各自成簇、互不打架,这是"好轴"的标志。

**2. 认知三分法是认知科学的标准术语,非 AI 自造**:episodic vs semantic 来自 Tulving(1972);procedural 来自 declarative/non-declarative 传统(Cohen & Squire 1980,Squire taxonomy)。在 language-agent 语境下最权威的整合是 **CoALA**(Sumers et al. 2023,arXiv 2309.02427),原文明确分 "working memory and several long-term memories: episodic, semantic, and procedural"。

**3. 业界主流实现都是这套的投影**:

- **CoALA**:working/episodic/semantic/procedural 四分,episodic 检索用 recency+importance+relevance。
- **LangMem(LangChain 官方 SDK)**:显式按 Semantic(Facts)/ Episodic(Past Experiences)/ Procedural(System Behavior)三分。
- **MemGPT/Letta**:main context(工作)/ recall(可检索对话历史=情景)/ archival(长期事实=语义),靠 paging 调度。
- **Zep/Graphiti**:显式区分 episodic(原始消息,无损)与 semantic(抽取的实体/事实),bi-temporal。
- **Generative Agents**:memory stream 是情景观察流 + 派生 reflection(偏语义),recency/importance/relevance 三因子检索。
- **Mem0**:务实压扁为长期事实库 + ADD/UPDATE/DELETE/NOOP 维护。

**4. 与既有决策一致,改动小**:三类记忆与现有设计近 1:1,主要是正名 + 解耦;ADR 0004(不做图)、0005(衰减/删除分离)全部继续成立,只是挂到新主轴下。

## 反方观点(诚实记录)

认知功能轴**并非无争议的"行业公认标准"**,必须正视反方:

- **"category error" 批评**:有 preprint(arXiv 2604.11364,*The Missing Knowledge Layer*,2026 近期 preprint)直接点名 CoALA,称认知切分会诱导工程上"拿时间衰减去删事实、用同一套更新机制对待事实与经历",主张按持久化语义分层。
  - **我们的回应**:这恰恰是 [ADR 0005](./0005-decay-ranking-conflict-deletion.md) 已经规避的——我们规定"衰减管排序、冲突管删除",从不拿衰减删事实;且本 ADR 让每类有独立的写入/淘汰策略,正是为避免"同一套机制对待不同记忆"。反方的火力打在"误用认知轴"上,不打在"分认知轴 + 各类策略分化"上。
- **Letta / LangChain 的定性**:Letta 主张"context engineering 优先于硬编码人类记忆结构",实际按位置/生命周期分;LangChain 明确称认知分类是 "borrowed from cognitive science",且把**生命周期当主轴**、认知类型只作长期记忆的子分类。
  - **我们的回应**:我们采纳了这个更稳妥的形态——**生命周期是第一级主轴**(working vs 长期),认知功能只在长期记忆内部细分。这与 LangChain 的实践一致,而非照搬 CoALA 的扁平四分。
- **survey 提出的替代轴**:*From Human Memory to AI Memory*(arXiv 2504.15965)主张"仅按时间维度分类不充分",提三维(归属/形态/时间);*Agent Memory*(arXiv 2603.07670,近期 preprint)提 temporal scope/substrate/control policy 三轴。
  - **我们的回应**:这些替代轴(归属、形态、生命周期)在我们方案里都没丢——它们被降级为**字段或策略**(归属=`owner_id`,形态=存储实现,生命周期=每类淘汰策略),只是不当**分类主轴**。

> **结论的边界**:不宣称"认知功能轴是学界公认唯一标准"(查无此原话)。准确表述是:认知三分法被广泛采用,CoALA 是 language-agent 语境下最权威来源;我们采用"生命周期主轴 + 长期记忆内认知功能细分"的混合形态,并以 ADR 0005 的策略分化规避其已知误用风险。

## 影响

- **三类记忆改名并重定位**(见 [memory-types](../modules/memory/memory-types.md)):
  - `semantic`:原 `personal`,语义不变。
  - `episodic`:原 `session`,**解开"短命"耦合**,重定位为"工作缓冲的无损可检索后备":显著性门控写入、recency+relevance 兜底检索、可跨会话、不靠时长定生死。
  - `procedural`:原 `experience`,语义不变。
- **工作记忆明确归应用/适配层**:context 窗口的保留与压缩是应用层职责,infra 不存工作记忆。这一刀解决了"session 之争"——session 当初混装的"工作缓冲"部分从此归位。
- **边界收窄**:semantic 只含"关于用户的语义事实",**不含世界通用知识**(教材/课标等领域知识归阶段二上下文模块),避免与上下文模块抢地盘。
- **代码层面**:`kind` 字段枚举从 `personal/session/experience` 改为 `semantic/episodic/procedural`;DTO、schema、检索路由、benchmark 术语同步。
- **ADR 0004 / 0005 继续有效**,在本 ADR 的分类下重新表述(语义记忆=原个人记忆的策略,程序记忆=原经验的策略)。

## 依据来源

认知科学根源:
- Tulving 1972, episodic vs semantic:https://en.wikipedia.org/wiki/Semantic_memory
- Procedural / declarative-nondeclarative(Squire):https://en.wikipedia.org/wiki/Procedural_memory 、https://www.ncbi.nlm.nih.gov/pmc/articles/PMC33639

CoALA(language-agent 认知功能分类权威来源):
- arXiv 2309.02427:https://arxiv.org/abs/2309.02427 (全文 https://arxiv.org/html/2309.02427)

业界实现:
- MemGPT/Letta arXiv 2310.08560:https://arxiv.org/abs/2310.08560 ;Letta 文档:https://docs.letta.com/guides/core-concepts/memory/context-hierarchy/
- Zep/Graphiti arXiv 2501.13956:https://arxiv.org/abs/2501.13956
- Generative Agents arXiv 2304.03442:https://arxiv.org/abs/2304.03442
- Mem0 arXiv 2504.19413:https://arxiv.org/abs/2504.19413
- LangMem(显式 episodic/semantic/procedural):https://langchain-ai.github.io/langmem/concepts/conceptual_guide/

反方/替代分类轴:
- LangChain *How To Give Your Agent Memory*(认知分类是 borrowed,主轴 lifecycle):https://www.langchain.com/blog/how-to-give-your-agent-memory
- Letta *Agent Memory*(context engineering 优先):https://www.letta.com/blog/agent-memory/
- *From Human Memory to AI Memory*(三维八象限)arXiv 2504.15965:https://arxiv.org/abs/2504.15965
- *Agent Memory: Mechanisms...*(近期 preprint)arXiv 2603.07670:https://arxiv.org/abs/2603.07670
- *The Missing Knowledge Layer*(category error 批评,近期 preprint)arXiv 2604.11364:https://arxiv.org/abs/2604.11364

> 注:2603.07670、2604.11364 为 2026 年近期 preprint,作为反方观点引用,权威度待时间检验。
