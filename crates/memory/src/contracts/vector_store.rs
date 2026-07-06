//! VectorStore 抽象:统一的存储与检索接口。
//!
//! 记忆领域逻辑只依赖此抽象,见不到 lancedb。唯一实现是 LanceDB(见 ADR 0001);
//! 换向量库 = 写一个新实现 + 跑过契约测试,领域逻辑零改动。
//!
//! 注:LanceDB 内核为 Rust,用 `lancedb` crate 原生接入——调用它即在享受 Rust 检索性能(ADR 0020)。

use std::collections::BTreeMap;

use async_trait::async_trait;
use foundation::KairosError;

/// 存储行:主键 + 向量列 + 元数据列的通用记录。用 JSON 值承载异构列。
pub type StoreRow = BTreeMap<String, serde_json::Value>;

/// 向量检索/存储的可选参数(where 前置过滤 + limit)。
#[derive(Debug, Clone, Default)]
pub struct SearchParams<'a> {
    /// SQL 风格前置过滤条件(如 owner_id 等值下推);None 表示不过滤。
    pub where_clause: Option<&'a str>,
    /// 返回上限。
    pub limit: usize,
}

/// 向量库的统一接口(节选关键方法,随实现推进补充)。
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// 按主键 upsert 若干行,返回写入行数。
    async fn upsert(&self, table: &str, rows: &[StoreRow]) -> Result<usize, KairosError>;

    /// 向量(cosine ANN)检索;`params.where_clause` 作为 prefilter。
    async fn vector_search(
        &self,
        table: &str,
        query_vector: &[f32],
        params: &SearchParams<'_>,
    ) -> Result<Vec<StoreRow>, KairosError>;

    /// BM25 全文检索(基于预分词的 token 列)。
    async fn fts_search(
        &self,
        table: &str,
        query_tokens: &[String],
        params: &SearchParams<'_>,
    ) -> Result<Vec<StoreRow>, KairosError>;

    /// 按 SQL 条件删除,返回删除行数(软删除)。
    async fn delete(&self, table: &str, where_clause: &str) -> Result<usize, KairosError>;

    /// 索引维护:把新增数据并入索引,避免 flat scan 退化。
    async fn optimize(&self, table: &str) -> Result<(), KairosError>;
}
