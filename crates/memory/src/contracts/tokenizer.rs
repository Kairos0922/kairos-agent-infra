//! Tokenizer 抽象:文本 → token 列表,用于 BM25 的预分词。
//!
//! 同步接口:纯 CPU 计算、无 IO,不需要 async(对齐"纯 CPU 计算保持同步"约束)。
//! 分词决策留在应用层:换分词器(jieba → 其他)只需换实现 + 重算 text_tokens 列,
//! 不动 schema、不依赖向量库内置分词器的语言支持。
//!
//! CPU 内核策略(ADR 0020):优先复用现成 Rust 分词库;Runtime 即 Rust,无需跨语言下沉。

/// 分词器的统一接口(同步)。
pub trait Tokenizer: Send + Sync {
    /// 把单条文本切成 token 列表。
    fn tokenize(&self, text: &str) -> Vec<String>;

    /// 批量分词。
    fn tokenize_batch(&self, texts: &[String]) -> Vec<Vec<String>>;
}
