# ADR 0007:记忆模块是机制,时机与质量评估是策略(写入/召回时机 + 选择性召回)

- **状态**:已接受
- **日期**:2026-06-27
- **相关文档**:[modules/memory/memory-types.md](../modules/memory/memory-types.md)、[modules/memory/retrieval.md](../modules/memory/retrieval.md)、[modules/memory/api.md](../modules/memory/api.md)
- **上位关系**:本决策确立一条**贯穿记忆模块的组织原则**(机制/策略分离),[ADR 0008](./0008-procedural-evaluation-decoupling.md) 是它在程序记忆上的直接推论。

## 背景

研讨到"不同记忆**何时存、何时召回**"时,出现一个根本问题:这些时机决策该不该放进记忆模块?

记忆效果取决于两件事——**高质量的记忆数据** + **召回的时机**。但二者的性质不同:

- "怎么存、怎么检索、怎么去重、怎么衰减排序" 是**机制(mechanism)**:确定的、可测的、与业务无关的能力。
- "什么时候该把这条对话写成记忆""这一轮 query 要不要去翻记忆""哪条经验值得记" 是**策略(policy)**:依赖业务语义、依赖上下文、会随场景演进的判断。

把策略焊进机制,会让记忆模块绑死某一种使用节奏,丧失可插拔性;也会让"判断"和"存储"两类完全不同的演进节奏互相拖累。需要一条清晰的分界。

## 候选方案

1. **时机决策放进记忆模块**:模块自己决定何时写、何时召回。简单,但把业务判断焊进 infra,换一种使用节奏就要改模块,违背解耦。
2. **每轮无差别全量召回**(把所有相关记忆都塞进 context):实现最省事,但被上下文污染证据证伪(见理由),直接损害精确率。
3. **机制/策略分离(选定)**:记忆模块只提供机制(存、检索、去重、衰减排序),时机与质量评估作为策略留在模块外(应用/适配层),模块可提供**可插拔的策略件**(如召回路由器)供上层选用但不强制。

## 结论

**确立组织原则:记忆模块 = 机制;"何时存、何时召回、什么值得记" = 策略。机制在模块内,策略在模块外。**

落到三件具体决策:

### 1. 写入时机:分 kind,绝不每轮无差别写

| 记忆 | 写入时机 | 形态 |
|------|---------|------|
| **semantic** | 信息**定型**时(话题切换 / 会话结束 / 显式确认),**异步**抽取 | 由上层在合适时机触发 `extract_semantic`,不在对话每轮跑 |
| **episodic** | 近实时追加,但过**显著性门控**,且**保持相对原始、不做重抽取** | 适配层在会话推进时写入,门控打 `salience` |
| **procedural** | 任务边界后、**经评估**才落库(评估/提炼解耦,见 [ADR 0008](./0008-procedural-evaluation-decoupling.md)) | 模块只收"已提炼经验" |

> 高质量记忆的第一道闸是**克制写入**:宁可漏写,不写脏。决定"何时算定型、何时触发"的是上层(策略);模块只提供 `remember` / `extract_semantic` 钩子 / 经验写入口(机制)。

### 2. 召回时机:选择性召回,不每轮全量

**反模式被证据钉死**:每轮把所有相关记忆灌进 context 会从多个机制同时损害精确率(见理由)。因此召回必须**选择性、带门控**:召回前先回答"① 这个 query 要不要记忆?② 要哪一类?③ 取多少?"。

- **触发决策权在应用层**(它拥有 context),但**记忆模块提供一个可插拔的 `RecallRouter` 作为可复用机制**:输入 query,输出"是否召回 + 召回哪些 kind + 建议 top_k"。上层可用它,也可自决。
- **同时把检索暴露成 memory-as-a-tool**:让 agentic 上层把"查记忆"作为工具交给 LLM 自主决定何时调用。它是 `RecallRouter` 的一种对外封装,与之不冲突。
- **MVP 不做训练式分类器**(违背 YAGNI):`RecallRouter` 先给**规则/启发式默认实现** + 留 Protocol 接口,将来可换 LLM 判断或训练模型。

### 3. 质量评估归策略层

"这条记忆 / 这条经验值不值得记、记得对不对" 是**质量评估**,属策略,不在记忆模块内。程序记忆的评估解耦见 [ADR 0008](./0008-procedural-evaluation-decoupling.md);语义记忆的"何时抽取"同理交上层。

## 理由

**1. 选择性召回有充分证据支撑,全量召回有害**:

- **Self-RAG**(arXiv 2310.11511):用反思 token 自适应决定"要不要检索",而非每次都检索。
- **Adaptive-RAG**(arXiv 2403.14403):用复杂度分类器把 query 路由到 no-retrieval / single-step / multi-step 三档。
- **FLARE**(arXiv 2305.06983):仅在模型对下一句不确定时才主动检索。
- **UAR**(*Unified Active Retrieval*,arXiv 2406.12534):把"要不要检索"拆成四个正交、即插即用的分类任务,成本近乎可忽略。

**2. 上下文污染证据证伪"全量召回"**:

- **Lost in the Middle**(arXiv 2307.03172):相关信息埋在长上下文中段时被严重忽略。
- **The Power of Noise**(arXiv 2401.14887):**高分但无关**的"近似干扰项"会主动降低生成质量——召回得多≠召回得好。
- **Redefining Retrieval Evaluation**(arXiv 2510.21440):干扰项"主动降质",检索评测需把这一项计入。
- **Context Rot / attention budget**(Chroma 报告 + Anthropic context engineering):上下文是有限的注意力预算,越长召回越差。

> 综合:召回的目标不是"找全",是"只把真正有用的少量记忆放进有限的注意力预算"。这与记忆模块"高精确率、低噪音"的总取向完全一致(见 [memory-types](../modules/memory/memory-types.md))。

**3. 写入时机分 kind 有业界实践支撑**:

- **LangMem**:三档写入——hot-path(模型主动写)/ background(异步抽取)/ delayed(带 30–60 分钟防抖的 ReflectionExecutor),明确避免对话中途、上下文不完整时写。
- **Mem0**:调用驱动 `add`,Platform 端异步落库。
- **Letta sleep-time compute**:后台 agent 异步整理记忆(详见 [ADR 0008](./0008-procedural-evaluation-decoupling.md))。

**4. memory-as-a-tool 是主流暴露形态**:MemGPT/Letta 把记忆读写作为工具交给 LLM;Anthropic 的 memory tool、LangMem 的 self-directed 模式同理。这印证"召回触发权在上层、模块提供能力"的分工。

**5. 与解耦原则一致**:机制/策略分离让记忆模块不绑定任何使用节奏。换一种召回策略 = 换一个 `RecallRouter` 实现或在上层自决,模块零改动——这正是"可插拔"在行为层面的延伸。

## 反方观点(诚实记录)

- **OpenAI/ChatGPT 式自动注入**:ChatGPT memory 在对话中自动保存、自动注入,体验上"无脑全自动"。但其代价是召回精度不透明、用户常报"记了不该记的"。我们选择**显式、选择性**的路线,把控制权和可解释性留在上层。
- **"门控本身也会错"**:选择性召回引入"判断要不要召回"的新错误源(漏召回)。回应:UAR/Adaptive-RAG 证明这类门控成本低、收益正;且漏召回的代价小于污染——污染会主动误导生成,漏召回只是少一条线索。MVP 用保守启发式起步,由 benchmark 校准。
- **MVP 是否过早引入 router**:有人会问"MVP 直接每轮召回 semantic 不就行了"。回应:`RecallRouter` 的 MVP 实现就是一个**薄启发式**(甚至可配置成"总是召回 semantic"),接口先立、实现可薄——这不是过度设计,而是把"召回是可替换策略"这件事在结构上确立,避免日后把策略硬编码进检索层再回头拆。

## 影响

- **retrieval.md**:新增"选择性召回 + `RecallRouter` + memory-as-a-tool"一节;明确触发决策在应用层、模块提供可插拔 router(MVP 启发式)。
- **api.md**:适配层暴露 router / memory-as-a-tool 形态;`recall` 的触发由上层控制的语义写清。
- **memory-types.md**:写入时机按 kind 明确(semantic 定型时异步、episodic 近实时门控保原始、procedural 经评估)。
- **foundation / README / overview**:补充"机制/策略分离"为记忆模块的组织原则;Non-goal 注明"召回时机策略、质量评估"不在 infra 机制内。
- **不改既有机制决策**:ADR 0004(原子事实)、0005(衰减/删除分离)、0006(认知功能分类)全部继续成立——本 ADR 只规定"时机与评估归策略层",不动存储与检索机制本身。

## 依据来源

选择性 / 自适应召回:
- Self-RAG arXiv 2310.11511:https://arxiv.org/abs/2310.11511
- Adaptive-RAG arXiv 2403.14403:https://arxiv.org/abs/2403.14403
- FLARE arXiv 2305.06983:https://arxiv.org/abs/2305.06983
- UAR(Unified Active Retrieval)arXiv 2406.12534:https://arxiv.org/abs/2406.12534

上下文污染 / 注意力预算:
- Lost in the Middle arXiv 2307.03172:https://arxiv.org/abs/2307.03172
- The Power of Noise arXiv 2401.14887:https://arxiv.org/abs/2401.14887
- Redefining Retrieval Evaluation … distractors arXiv 2510.21440:https://arxiv.org/abs/2510.21440
- Anthropic *Effective context engineering for AI agents*:https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents
- Chroma *Context Rot* 报告:https://research.trychroma.com/context-rot

写入时机 / memory-as-a-tool:
- LangMem(hot-path / background / delayed ReflectionExecutor):https://langchain-ai.github.io/langmem/concepts/conceptual_guide/
- Mem0 arXiv 2504.19413:https://arxiv.org/abs/2504.19413
- MemGPT/Letta arXiv 2310.08560:https://arxiv.org/abs/2310.08560
- Anthropic memory tool:https://docs.anthropic.com/en/docs/build-with-claude/tool-use/memory-tool

> 效果验证依赖 benchmark(见 [benchmark/protocol](../modules/benchmark/protocol.md)):选择性召回 vs 全量召回对 Precision@K / 干扰项抗性的影响,需用自建中文 benchmark 实测。
