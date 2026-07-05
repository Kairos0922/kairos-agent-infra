"""记忆模块:管理三类长期记忆(semantic / episodic / procedural)。

自包含结构:
- contracts/   抽象接口。对外契约(store.py:MemoryStore / Retriever,harness 消费)
               + provider 契约(VectorStore / EmbeddingProvider / RerankProvider / Tokenizer)
- store.py     MemoryStore/Retriever 实现(领域逻辑总入口)
- models.py    领域模型(MemoryBase + 三类 kind)
- providers/   provider 契约的具体实现(可插拔),含 LanceDB 租户物理分表路由
- kinds/       三类记忆各自的写入/淘汰逻辑
- retrieval/   统一检索层(向量/BM25/融合/rerank + RecallRouter)

依赖倒置:领域逻辑(store/kinds/retrieval)只依赖 contracts 抽象,
不依赖 providers,不 import lancedb;具体实现由组装根 factory 配置注入(ADR 0011)。

边界:trace 评估/提炼归 harness/distill 管线(ADR 0008),模块对 procedural
只暴露 write_experience;工作记忆归 harness 层 Context Engine(ADR 0006)。

详见 docs/modules/memory/README.md。
"""
