# ADR 0005:衰减管排序,冲突管删除(两套机制分开)

- **状态**:已接受
- **日期**:2026-06-27
- **相关文档**:[modules/memory/memory-types.md](../modules/memory/memory-types.md)

## 背景

记忆会随时间积累,需要某种机制控制"老记忆"对检索的影响,避免过时信息污染结果(噪音)。一个常见但危险的直觉是"时间久 + 很少用 → 删除"。需要明确:衰减用于什么、删除由什么触发。

## 候选方案

1. **衰减管排序、冲突管删除(分离)**:时间/使用频率衰减只用于**检索时降权**(soft re-rank,不删);真正的删除只由**语义冲突**(新事实推翻旧事实)触发。
2. **衰减用于淘汰删除**:时间久 + 低使用频率直接删除。
3. **MVP 不做衰减**:所有记忆等权。

## 结论

选**方案 1:衰减管排序、冲突管删除**。两套机制职责分开。

## 理由

业界主流系统(基于本项目调研)高度一致地倒向"衰减只降权、不删除":

- **Mem0 Memory Decay(2026-05)明确"nothing gets deleted",是 soft re-rank**:按 recency + 最近访问频率把 relevance 分缩放到 0.3×~1.5×,下限保证强匹配的老记忆仍能浮现。
- **Generative Agents(Park et al. 2023)** 的 recency 因子(指数衰减,因子 0.995)也只用于检索打分,不删记忆;最终分 = recency × importance × relevance 等权归一相加。
- **时间久 ≠ 该删**:用户生日不会因为很久没提就失效。把时间衰减当删除依据会误删仍然正确的稳定事实——直接违背"高精确率"目标。
- **删除应由语义冲突驱动**:Mem0 的 DELETE 操作、Zep/Graphiti 的 edge invalidation 都是"新信息推翻旧信息"时才失效旧条目。这是**语义冲突淘汰**,与**时间衰减淘汰**是两回事,必须分清。

## 影响

三类记忆按本决策分化(见 [memory-types](../modules/memory/memory-types.md);命名见 [ADR 0006](./0006-memory-classification-by-cognitive-function.md)):

- **语义记忆 semantic**:删除靠**冲突更新**(LLM 驱动 UPDATE/DELETE,见 ADR 0004);recency 仅作检索时的轻微降权。不按时间自动过期。
- **情景记忆 episodic**:按 recency + 显著性降权,久未命中可**归档**;需要"会话结束即清"用显式 `forget_session`。不再用一刀切 TTL 硬淘汰(那是旧 `session` 把"短命"当本质的产物,已由 ADR 0006 纠正)。
- **程序记忆 procedural**:用**强度衰减 + 使用强化**(MemoryBank 式 R = e^(−t/S),被复用则强度 S 增大、衰减变慢)。经验是"启发式假设",越用越可信、久不用则可疑;长期低有效性可标记废弃(soft delete)。

检索层与维护任务的职责据此划分:
- 检索时:按 recency/strength 对候选**降权排序**(不改变存储)。
- 维护任务:执行 episodic 归档/衰减、procedural 强度衰减更新;**不**因"时间久"删除语义记忆。
- 写入时:LLM 驱动的冲突检测决定是否 UPDATE/DELETE 旧条目。

## 依据来源

- Mem0 *Introducing Memory Decay*(mem0.ai/blog,2026-05)+ 官方文档。
- Generative Agents(arxiv 2304.03442):recency/importance/relevance 三因子打分。
- MemoryBank(arxiv 2305.10250):Ebbinghaus R=e^(−t/S) 强度衰减 + 回忆强化。
- 详见 [tradeoffs](../modules/memory/tradeoffs.md)。
