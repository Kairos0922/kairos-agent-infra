//! 记忆模块(memory,L1):管理三类长期记忆(semantic / episodic / procedural)。
//!
//! 自包含结构(随 Phase 2 落地):
//! - `contracts`   抽象 trait(对外契约 + provider 契约:VectorStore / EmbeddingProvider /
//!   RerankProvider / Tokenizer)
//! - `store`       MemoryStore/Retriever 实现(领域逻辑总入口)
//! - `models`      领域模型(MemoryBase + 三类 kind)
//! - `providers`   provider 契约的具体实现(可插拔,私有 mod),含 LanceDB 租户物理分表路由
//! - `kinds`       三类记忆各自的写入/淘汰逻辑
//! - `retrieval`   统一检索层(向量/BM25/融合/rerank + RecallRouter)
//!
//! 依赖倒置:领域逻辑只依赖 `contracts` 抽象,不依赖 `providers`、不依赖 `lancedb` crate;
//! 具体实现由组装根 factory 配置注入(ADR 0011)。只依赖 foundation(L1 模块间零依赖)。
//!
//! 详见 docs/modules/memory/README.md。

pub mod contracts;
