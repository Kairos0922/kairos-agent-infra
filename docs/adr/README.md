# 架构决策记录 (ADR)

本目录记录 Kairos 的重大技术决策。每条 ADR 记录:背景、候选方案、结论、理由、影响。

决策可追溯,避免反复推翻已定结论。规范见 [CLAUDE.md](../../CLAUDE.md) 的 ADR 一节。

## 索引

| 编号 | 标题 | 状态 |
|------|------|------|
| [0001](./0001-vector-store-lancedb.md) | 向量库选用 LanceDB | 已接受 |
| [0002](./0002-hybrid-fusion-rrf.md) | 混合检索融合策略选用 RRF | 已接受 |
| [0003](./0003-abstractions-in-module.md) | 抽象接口归属记忆模块,不预先上提到底座 | 已接受 |
| [0004](./0004-no-knowledge-graph-mvp.md) | MVP 不做知识图谱,先做原子事实 | 已接受 |
| [0005](./0005-decay-ranking-conflict-deletion.md) | 衰减管排序,冲突管删除(两套机制分开) | 已接受 |
| [0006](./0006-memory-classification-by-cognitive-function.md) | 记忆按认知功能分类(工作记忆归应用层,长期记忆分情景/语义/程序) | 已接受 |

## 状态说明

- **提议中 (Proposed)**:待用户决策。
- **已接受 (Accepted)**:已采纳,正在执行。
- **已废弃 (Deprecated)**:被后续决策取代,保留记录(注明被哪条取代)。
