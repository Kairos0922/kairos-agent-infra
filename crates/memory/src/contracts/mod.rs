//! memory 模块契约:provider 契约(可插拔实现的抽象 trait)。
//!
//! 对外契约(MemoryStore / Retriever,harness 消费)随领域逻辑落地时补充。
//! 契约位于模块 crate 内(ADR 0011/0014):领域逻辑只依赖这些 trait,具体实现
//! 由组装根 factory 注入;providers mod 私有,harness 不可见。

pub mod embedding;
pub mod rerank;
pub mod tokenizer;
pub mod vector_store;

pub use embedding::EmbeddingProvider;
pub use rerank::{RerankProvider, RerankResult};
pub use tokenizer::Tokenizer;
pub use vector_store::{SearchParams, StoreRow, VectorStore};
