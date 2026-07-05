# ADR 0015:向量存储契约与 RRF 融合上提 foundation(Phase 3)

- **状态**:已接受(触发条件在 Phase 3)
- **日期**:2026-07-05
- **相关文档**:[modules/memory/retrieval.md](../modules/memory/retrieval.md)、[modules/knowledge.md](../modules/knowledge.md)、[modules/memory/README.md](../modules/memory/README.md)
- **上位关系**:是 [ADR 0003](./0003-abstractions-in-module.md)"抽象归模块、出现第二消费者再上提"判据的**兑现**,不是推翻——ADR 0003 明确"共享是被发现的,不是被预测的",本 ADR 记录发现时刻。

## 背景

ADR 0003 决定 `VectorStore`/`EmbeddingProvider` 等抽象先归 memory 模块,不预先上提 foundation,并明确"等出现第二个使用者且确有复用需求时再上提"。Phase 3 引入 knowledge 模块(资料型知识检索),它同样需要向量存储 + hybrid 检索 + RRF 融合——**第二个消费者出现了**。

## 候选方案

1. **knowledge 复制一套 memory 的向量/融合抽象**:被否——两份几乎相同的 `VectorStore`/RRF,违反 DRY,且两模块演进易分叉。
2. **knowledge 直接 import memory 的 contracts**:被否——违反 L1 模块独立(import-linter 契约二);模块间横向依赖是架构红线。
3. **上提到 foundation(选定)**:把 `VectorStore` 契约与 `reciprocal_rank_fusion`(纯计算)上提为 `foundation` 的共享抽象,memory 与 knowledge 都依赖 foundation 的版本。

## 结论

- **触发时机**:Phase 3 knowledge 模块落地时,不早于此(避免预先抽象猜错)。
- **上提内容**:`VectorStore` 契约(向量/FTS/upsert/delete/optimize)+ `reciprocal_rank_fusion` 纯函数。**不上提**的:`EmbeddingProvider`/`RerankProvider`/`Tokenizer` 若届时仍只 memory 用,则留 memory(逐个按同一判据评估,不打包上提)。
- **上提后**:memory 与 knowledge 的领域逻辑都依赖 `foundation` 的向量抽象;各自的 provider 实现仍在各自模块(或共享一个 lancedb 实现,届时评估)。
- **契约测试**:上提后契约测试也随抽象搬到 foundation,memory/knowledge 的 provider 实现都必须跑过。

## 理由

- 兑现 ADR 0003 的按需上提承诺:此时已知两个真实消费者的需求,抽象不再是猜测。
- 保持 L1 模块独立:共享逻辑上提到 L0,而非模块间横向依赖。
- RRF 是纯计算(无状态、同步),上提零风险;`VectorStore` 是稳定契约,两模块用法一致。

## 影响

- Phase 3 执行:`foundation/` 新增向量存储契约与融合纯函数;memory 的 `contracts/vector_store.py`、`retrieval/fusion.py` 迁移并改为 re-export 或直接依赖 foundation。
- import-linter:memory/knowledge → foundation 依赖合法(契约一允许);模块间仍零依赖。
- 在此之前(Phase 2),向量抽象保持在 memory 模块内,本 ADR 只登记决策与触发条件。
