//! 检索器占位 trait(D5 决策)。
//!
//! 临时占位,待 memory/knowledge 模块落地后由其对外契约替换。
//! 替换点登记于 PROGRESS.md。

use async_trait::async_trait;
use foundation::{KairosError, TenantContext};
use serde::{Deserialize, Serialize};

/// 记忆检索器(P5 分区数据源)。
///
/// 临时占位 trait:memory 模块落地后由 `memory::contracts::Retriever` 替换。
/// ContextEngine 持有 `Option<Arc<dyn MemoryRetriever>>`,None → P5 空分区。
#[async_trait]
pub trait MemoryRetriever: Send + Sync {
    /// 检索记忆片段。
    async fn search(
        &self,
        ctx: &TenantContext,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryFragment>, KairosError>;
}

/// 知识检索器(P4 分区数据源)。
///
/// 临时占位 trait:knowledge 模块落地后由 `knowledge::contracts::KnowledgeRetriever` 替换。
/// ContextEngine 持有 `Option<Arc<dyn KnowledgeRetriever>>`,None → P4 空分区。
#[async_trait]
pub trait KnowledgeRetriever: Send + Sync {
    /// 检索知识切片。
    async fn search(
        &self,
        ctx: &TenantContext,
        query: &str,
        limit: usize,
    ) -> Result<Vec<KnowledgeFragment>, KairosError>;
}

/// 记忆片段(P5 分区的单条内容)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFragment {
    /// 记忆 ID(用于 id 级去重)。
    pub id: String,
    /// 记忆种类(episodic / semantic / procedural)。
    pub kind: String,
    /// 记忆内容文本。
    pub content: String,
    /// 检索得分(用于超配额时按得分从低到高丢弃)。
    pub score: f64,
    /// 创建时间(冲突时新者优先)。
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// scope metadata(班级/学科等)。
    pub scope: Option<serde_json::Value>,
}

/// 知识切片(P4 分区的单条内容)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeFragment {
    /// 切片 ID(用于 id 级去重)。
    pub id: String,
    /// 来源引用(pack/文档/位置)。
    pub source: String,
    /// 切片内容文本。
    pub content: String,
    /// 检索得分。
    pub score: f64,
}
