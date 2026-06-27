"""记忆模块:管理三类记忆(personal / session / experience)。

自包含结构:
- contracts/   模块内的抽象接口(VectorStore / EmbeddingProvider / RerankProvider / Tokenizer)
- providers/   抽象的具体实现(可插拔)
- kinds/       三类记忆各自的写入/淘汰逻辑
- retrieval/   统一检索层(向量/BM25/融合/rerank)
- experience/  trace → 经验提炼
- facade.py    模块对外唯一入口
- models.py    领域模型 + LanceDB schema

依赖倒置:领域逻辑(kinds/retrieval/experience)只依赖 contracts 抽象,
不依赖 providers,不 import lancedb;具体实现由 providers/factory 配置注入。

详见 docs/modules/memory/README.md。
"""
