# ADR 0004:MVP 不做知识图谱,先做原子事实

- **状态**:已接受
- **日期**:2026-06-27
- **相关文档**:[modules/memory/memory-types.md](../modules/memory/memory-types.md)、[modules/memory/tradeoffs.md](../modules/memory/tradeoffs.md)

## 背景

记忆模块要存储用户事实/偏好。一个核心架构岔路:是否引入**实体关系图 / 知识图谱**(把记忆建模为实体节点 + 关系边),还是用**原子事实 + 向量检索**的扁平方案。

知识图谱方案以 Mem0g、Zep/Graphiti、Cognee 为代表,能做跨实体的多跳关系推理与时序事实追踪。

## 候选方案

1. **原子事实 + 向量检索 + LLM 驱动去重**:每条记忆是一条独立事实,向量化后检索;写入时用 LLM 在 ADD/UPDATE/DELETE/NOOP 间决策(借鉴 Mem0 基础版)。
2. **知识图谱**:实体抽取 + 关系生成 + 冲突消解,存图数据库(如 Neo4j),图遍历检索(Mem0g / Zep 路线)。
3. **不做图但预留实体/关系字段**:扁平存储,但 schema 预留字段为未来上图铺路。

## 结论

MVP 选**方案 1(原子事实 + 向量 + LLM 去重)**。**不做图,也不预留实体/关系字段**(按 YAGNI,见 ADR 0003)。图作为后续可叠加层,需求出现时再做。

## 理由

依据本项目的调研(基于一手论文/官方文档):

- **图的收益是局部的**:Mem0 论文自身数据显示,graph 版(Mem0g)相比非图版整体仅高约 2%,且**单跳查询略降、多跳查询无增益(论文称存在"冗余")、只有时间推理明显胜出**。对追求"高精确率、低噪音"的扁平事实检索,图未证明更优。
- **代价是全局的、隐藏的**:Mem0g 的 token 消耗翻倍(~7k → ~14k);写入延迟显著上升(实体抽取 + 关系生成 + 冲突消解每步都是 LLM 调用);需引入图数据库;抽取错误会污染图且更难发现;调优表面积巨大(Cognee 自承默认配置与调优配置正确率相差 0.476 → 0.815)。
- **图唯一不可替代的硬能力是时序事实追踪**(谁现在在哪工作 vs 历史)。但本项目可用更轻的方式覆盖大部分场景:用 LLM 驱动的 UPDATE/DELETE 处理事实变更与冲突,而非维护 bi-temporal 图。
- **符合避免过度设计**:在没有验证出"非图方案不够用"之前,不引入图的复杂度。

## 影响

- 个人记忆/执行经验的写入走 **LLM 驱动的 ADD/UPDATE/DELETE/NOOP**(见 [memory-types](../modules/memory/memory-types.md)),这是去重与冲突解决的核心,也支撑 ADR 0005 的"冲突管删除"。
- schema **不含**实体/关系字段;未来若上图,作为叠加层新增,不改动现有扁平存储(届时新增 ADR)。
- benchmark 需覆盖"知识更新(KU)""时序推理(TR)"能力类,以便客观判断扁平方案在这两类上的真实表现——若数据证明不足,再考虑上图。

## 依据来源

- Mem0 论文 *Building Production-Ready AI Agents with Scalable Long-Term Memory*(arxiv 2504.19413):Mem0g 题型分解数据、token/延迟对比。
- Zep 论文(arxiv 2501.13956):bi-temporal 图与 edge invalidation。
- Cognee 基准(cognee.ai/blog):调优表面积、多跳结构化召回优势。
- 详见 [tradeoffs](../modules/memory/tradeoffs.md) 与 [everos-analysis](../modules/memory/everos-analysis.md)。
