//! EmbeddingProvider 抽象:文本 → 向量。
//!
//! 可插拔:openai_compat(任何 OpenAI 兼容端点,含本地 vLLM/Ollama)、
//! sentence_transformer(纯本地进程内)等实现,通过配置切换。

use async_trait::async_trait;
use foundation::KairosError;

/// embedding 模型的统一接口。
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    /// 向量维度,必须与向量库向量列一致。
    fn dim(&self) -> usize;

    /// 把单条文本编码为向量。
    async fn embed(&self, text: &str) -> Result<Vec<f32>, KairosError>;

    /// 批量编码。实现应在内部做分块 + 并发限流。
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, KairosError>;
}
