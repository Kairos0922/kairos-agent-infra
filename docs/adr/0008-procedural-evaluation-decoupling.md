# ADR 0008:程序记忆的 trace 评估/提炼与记忆模块解耦

- **状态**:已接受
- **日期**:2026-06-27
- **相关文档**:[modules/memory/memory-types.md](../modules/memory/memory-types.md)、[modules/memory/api.md](../modules/memory/api.md)、[foundation/foundation.md](../foundation/foundation.md)
- **上位关系**:本决策是 [ADR 0007](./0007-memory-mechanism-vs-policy-timing.md)(机制/策略分离)在**程序记忆**上的直接推论;延续 [ADR 0006](./0006-memory-classification-by-cognitive-function.md) 对 procedural 的定位。

## 背景

程序记忆(procedural)的原料不是用户输入,而是 **Agent 自己的执行 trace**。原先的设计把整条链路放进了记忆模块内部:

```
trace 采集 → 规则门控 → 分段 → LLM 提炼 → 评估 effectiveness → 去重写入   ← 全在 modules/memory/procedural/
```

研讨"要不要把 trace 上报到另一个系统、评估后再落库"时,暴露这个设计的问题:**"把一条原始 trace 评估、提炼成一条值得记的经验" 是质量评估(策略),不是存储能力(机制)**。把它焊进记忆模块,违背刚确立的 [ADR 0007](./0007-memory-mechanism-vs-policy-timing.md),且带来三个具体麻烦:

1. **耦合两种节奏**:在线对话要低延迟,提炼/评估是慢的重活(多次 LLM 调用、跨执行聚合),塞进同一模块互相拖累。
2. **绑死评估方式**:评估逻辑写死在模块内,换一种评估口径(接外部 observability 平台、引入人工审核、用真实执行结果回灌)就要改模块。
3. **质量判断混入存储**:记忆模块本应只管"把已经决定要记的东西存好、检索好",却被迫承担"判断这条经验值不值得记、对不对"的业务判断。

## 候选方案

1. **维持现状**(提炼/评估在记忆模块内):一站式,但耦合在线/离线节奏、绑死评估方式、把策略焊进机制。
2. **立即建独立评估平台**:架构最干净,但本阶段没有真实 trace 量、没有评估需求,提前建是过度设计。
3. **解耦边界 + 模块外最小占位(选定)**:把"trace 采集 / 评估 / 提炼"定义为**记忆模块之外的独立关注点**,记忆模块对 procedural 只暴露"写入一条已提炼、已评估的经验";MVP 用一个**模块外的最小占位生产者**(规则门控 + 单次 LLM 抽取)产出经验,通过标准写接口落库。**不预先绑定**最终形态(外接 observability 平台 vs 后续独立 Kairos 模块),留到真正建设时再定。

## 结论

**程序记忆的 trace 评估/提炼移出记忆模块。记忆模块对 procedural 只暴露"写入一条已提炼、已评估的经验",所有"决定记什么经验"的智能在模块外。**

解耦后的链路:

```
Agent 执行 ─► trace 上报 ─► 【独立评估/提炼 pipeline(模块外)】 ─► 已提炼经验 ─► 记忆模块写接口 ─► procedural 库
                            评估成败 / 提炼可复用模式 /                        ▲ 模块只认
                            跨执行聚合 / 可选人工审核                          "已决定要记的经验"
```

边界划分:

| 关注点 | 归属 | 说明 |
|--------|------|------|
| trace 抽象 / 接入点 | **底座** `foundation/tracing.py` | 横切能力,可观测性与经验提炼共用(见 [foundation](../foundation/foundation.md)) |
| trace 采集 / 评估 / 提炼 | **模块外**(策略) | 独立 pipeline;MVP 为最小占位生产者 |
| 经验写入(去重 / 衰减排序 / 检索 / 强化记账) | **记忆模块**(机制) | `ProceduralMemory` 的存储与检索;`reinforce` 记账是机制,何时调用是策略 |

**MVP 范围(YAGNI)**:本阶段**不真建评估平台**。做法是把**写入契约和边界定清楚**,对 procedural 给一个**模块外**的最小占位生产者(规则门控 + 单次 LLM 抽取),它通过标准接口落库。架构位置对了,实现可以薄。

**最终形态留待定**:独立 pipeline 将来是"外接 observability 平台(LangSmith/Langfuse/Phoenix 等)"还是"规划成 Kairos 后续的独立 infra 模块(如 `evaluation` / `experience-forge`)",**本阶段不绑定**——符合"共享/能力按需上提、不提前预测"。

## 理由

**1. 业界正在把"获取"与"整合"解耦**:

- **Letta sleep-time compute**(arXiv 2504.13171 + 官方):主在线 agent 负责对话,独立的 sleep-time agent 在空闲时异步整理记忆——延迟↓、可靠性↑、质量↑,报告跨相关查询每查询成本降约 2.5×。
- **Auto-Dreamer**(arXiv 2605.20616,2026 近期 preprint):受 CLS(互补学习系统)启发,显式把"快速 per-session 获取"与"慢速跨会话整合"解耦,活跃库规模小约 12×。

**2. 离线提炼是经验记忆的成熟范式**:

- **ExpeL**(arXiv 2308.10144):**离线**在任务池上跨任务提炼经验(insights),冻结后检索应用——这正是 procedural 的范式。
- **Reflexion**(arXiv 2303.11366)、**Voyager**(arXiv 2305.16291):在线反思/技能积累;**Generative Agents**(arXiv 2304.03442):在线 reflection。在线/离线各有适用,但"提炼"本身是独立于"存储"的一步。

**3. 可观测性平台天然把 trace+评估 与存储分离**:LangSmith / Langfuse / Arize Phoenix 都是"收 trace + 评估(含 LLM-as-judge、人工标注、数据集回归)"的独立层,从不与记忆存储耦合。我们的解耦边界与这个成熟分工对齐。

**4. "存储不等于记忆"警示摄入端过度加工的风险**:

- *Storage Is Not Memory*(arXiv 2605.04897,近期 preprint)批评"在摄入时就抽取/加工"是错误原语,会在写入端丢信息、检索端找不回。
- **回应与边界**:这条警示对 **episodic** 直接生效——episodic 保持相对原始、把智能放检索端(见 [ADR 0007](./0007-memory-mechanism-vs-policy-timing.md)、[memory-types](../modules/memory/memory-types.md))。对 **procedural**,提炼是其定义(经验本就是从 trace 蒸馏的可复用模式),但我们用"保留原始 trace + 提炼经验各存一份"的方式缓解:`source_trace_id` 回指原始 trace,提炼丢了的可回溯;且提炼在**模块外**,可独立改进、可重跑,不是一次性焊死。

**5. LLM-as-judge 不可靠 → 评估不能盲信单次打分**:

- 多篇指出 LLM-as-judge 存在偏置与不稳定(arXiv 2412.12509、2410.20266、2403.02839)。
- **约束**:模块外的评估 pipeline 不得盲信单次 LLM 打分,要么多次采样、要么以**真实执行结果(success 信号)**兜底。这条写进评估 pipeline 的设计约束,也是把评估放在模块外、可独立强化的又一理由。

## 反方观点(诚实记录)

- **"一站式更简单"**:把提炼放模块内,调用方一个 `ingest_trace` 就完事。回应:简单是表象——它把在线/离线节奏和评估口径焊死,日后任何评估升级都要动记忆模块。解耦后 MVP 的占位实现同样是"一个入口",只是这个入口在模块外,模块只暴露"写已提炼经验"。
- **"MVP 阶段解耦是过度设计"**:回应:我们**没有**建平台,只是把**边界**画在正确的位置 + 一个薄占位。不画这条线,等真要接 observability 或加评估模块时,得回头从记忆模块里把提炼逻辑拆出来——那才是更贵的返工。画线成本几乎为零,收益是结构正确。
- **procedural 提炼 vs "Storage Is Not Memory"**:有张力——一个说"提炼是 procedural 的本质",一个说"摄入端加工有害"。回应见理由 4:保留原始 trace 兜底 + 提炼在模块外可重跑,两者兼顾。

## 影响

- **目录结构调整**:`modules/memory/procedural/`(`distiller.py` / `trace_schema.py`)从记忆模块**移出**。`trace` 抽象留在底座 `foundation/tracing.py`;提炼/评估的 MVP 占位生产者放**模块外**(适配层或独立的占位模块),通过记忆模块写接口落库。
- **memory-types.md**:procedural 一节从"模块内 distiller 提炼流程"改写为"模块只接收已提炼/已评估经验 + 保留 `source_trace_id` 兜底";提炼流程图移出或标注为"模块外 pipeline"。episodic 强化"保持相对原始、智能放检索端"。
- **api.md**:`ingest_trace`(含规则门控 + 提炼)语义调整——要么移到模块外的占位生产者,要么记忆模块只保留"写入已提炼经验"的入口;`reinforce`(强化记账,机制)保留在模块 API,但"何时调用"由上层决定。
- **foundation.md**:trace 接入点说明里,明确"如何把 trace 提炼成经验"是**模块外的策略**,不在记忆模块业务逻辑内(原文把它归在记忆模块,需更正)。
- **overview.md / roadmap.md**:Non-goal / 路线图标注"trace 评估/提炼 pipeline"为记忆模块之外的独立关注点,最终形态(外接平台 vs 独立模块)待定。
- **不改 procedural 的存储/检索/衰减机制**:ADR 0005(强度衰减 + 使用强化 + 低效淘汰)继续成立——那是机制,留在模块内。

## 依据来源

获取/整合解耦:
- Letta *Sleep-time Compute* arXiv 2504.13171:https://arxiv.org/abs/2504.13171 ;官方:https://www.letta.com/blog/sleep-time-compute
- Auto-Dreamer arXiv 2605.20616(2026 近期 preprint):https://arxiv.org/abs/2605.20616

离线/在线经验提炼:
- ExpeL(离线跨任务提炼)arXiv 2308.10144:https://arxiv.org/abs/2308.10144
- Reflexion arXiv 2303.11366:https://arxiv.org/abs/2303.11366
- Voyager arXiv 2305.16291:https://arxiv.org/abs/2305.16291
- Generative Agents arXiv 2304.03442:https://arxiv.org/abs/2304.03442

可观测性平台(trace+评估 独立于存储):
- LangSmith:https://docs.smith.langchain.com/ ;Langfuse:https://langfuse.com/docs ;Arize Phoenix:https://docs.arize.com/phoenix

摄入端加工风险 / 评估可靠性:
- *Storage Is Not Memory*(近期 preprint)arXiv 2605.04897:https://arxiv.org/abs/2605.04897
- LLM-as-judge 偏置/不稳定:arXiv 2412.12509(https://arxiv.org/abs/2412.12509)、arXiv 2410.20266(https://arxiv.org/abs/2410.20266)、arXiv 2403.02839(https://arxiv.org/abs/2403.02839)

> 注:2605.20616、2605.04897 为 2026 年近期 preprint,作为方向佐证引用,权威度待时间检验。效果验证依赖 benchmark(见 [benchmark/protocol](../modules/benchmark/protocol.md))。
