//! RerankProvider 抽象:对候选文档按与 query 的相关度重排。
//!
//! 契约:provider 只排序、不过滤——返回每个输入文档一条结果(按 score 降序),
//! top_k 截断由调用方负责。保证跨 provider 契约稳定(借鉴 EverOS)。

use async_trait::async_trait;
use foundation::KairosError;

/// 单条重排结果。
#[derive(Debug, Clone, PartialEq)]
pub struct RerankResult {
    /// 在输入 documents 列表中的原始下标。
    pub index: usize,
    /// 相关度分数,provider 定义,越高越相关。
    pub score: f32,
}

/// rerank 模型的统一接口。
#[async_trait]
pub trait RerankProvider: Send + Sync {
    /// 对 documents 按与 query 的相关度重排。
    ///
    /// 约定:返回每个输入文档一条结果,按 score 降序;不做过滤/截断。
    /// `instruction` 支持 instruction-tuned reranker(如 Qwen3-Reranker),None 时不传。
    async fn rerank(
        &self,
        query: &str,
        documents: &[String],
        instruction: Option<&str>,
    ) -> Result<Vec<RerankResult>, KairosError>;
}
