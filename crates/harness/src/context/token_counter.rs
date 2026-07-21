//! TokenCounter:token 估算器(非精确计数器)。
//!
//! 与 memory::Tokenizer(BM25 分词)语义不同:计数 vs 分词、BPE vs 分词器。
//! 初始实现:tiktoken-rs cl100k_base × 1.2 保守系数(宁可早压缩,不可爆窗)。
//! 双轨口径:分区配额用估算值;预算扣减以 gateway 返回的真实 usage 为准。
//! 校准闭环:每次模型调用后用 实际 input_tokens / 估算值 更新 deployment 级校准系数。

/// token 估算器接口。同步:纯 CPU 计算、无 IO。
pub trait TokenCounter: Send + Sync {
    /// 单条文本的估算 token 数。
    fn count(&self, text: &str) -> usize;
    /// 批量估算。
    fn count_batch(&self, texts: &[String]) -> Vec<usize>;
}

/// 保守系数:cl100k_base 对中文高估约 30-50%,对 Claude/GLM/DeepSeek 也有偏差。
/// 乘以 >1 的系数使估算偏大(宁可早压缩,不可爆窗)。
const CONSERVATIVE_FACTOR: f64 = 1.2;

/// 基于 tiktoken cl100k_base 的 token 估算器。
pub struct TiktokenCounter {
    bpe: tiktoken_rs::CoreBPE,
    /// 校准系数(初始为 CONSERVATIVE_FACTOR,随真实 usage 校准收敛)。
    factor: std::sync::atomic::AtomicU64,
}

impl TiktokenCounter {
    /// 构造 tiktoken 估算器(cl100k_base 编码)。
    pub fn new() -> Self {
        let bpe = tiktoken_rs::cl100k_base().expect("cl100k_base 编码应始终可用");
        Self {
            bpe,
            factor: std::sync::atomic::AtomicU64::new(factor_to_bits(CONSERVATIVE_FACTOR)),
        }
    }

    /// 用真实 usage 校准:actual_tokens / estimated_tokens → 更新系数。
    ///
    /// 系数 = 旧系数 × 0.7 + 新比值 × 0.3(指数移动平均,平滑收敛)。
    /// 系数下限 1.0(不低估),上限 2.0(不过度保守)。
    pub fn calibrate(&self, actual_tokens: usize, estimated_tokens: usize) {
        if estimated_tokens == 0 {
            return;
        }
        let ratio = actual_tokens as f64 / estimated_tokens as f64;
        let old = bits_to_factor(self.factor.load(std::sync::atomic::Ordering::Relaxed));
        let new = (old * 0.7 + ratio * 0.3).clamp(1.0, 2.0);
        self.factor
            .store(factor_to_bits(new), std::sync::atomic::Ordering::Relaxed);
    }

    fn current_factor(&self) -> f64 {
        bits_to_factor(self.factor.load(std::sync::atomic::Ordering::Relaxed))
    }
}

impl Default for TiktokenCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenCounter for TiktokenCounter {
    fn count(&self, text: &str) -> usize {
        let raw = self.bpe.encode_with_special_tokens(text).len();
        (raw as f64 * self.current_factor()).ceil() as usize
    }

    fn count_batch(&self, texts: &[String]) -> Vec<usize> {
        texts.iter().map(|t| self.count(t)).collect()
    }
}

/// f64 → u64 位表示(用于 AtomicU64 存储)。
fn factor_to_bits(f: f64) -> u64 {
    f.to_bits()
}

/// u64 位表示 → f64。
fn bits_to_factor(bits: u64) -> f64 {
    f64::from_bits(bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_english_text() {
        let counter = TiktokenCounter::new();
        let count = counter.count("hello world");
        // cl100k_base: "hello world" ≈ 2 tokens, × 1.2 ≈ 3
        assert!(count >= 2, "应至少 2 tokens,实际 {count}");
        assert!(count <= 10, "不应超过 10 tokens,实际 {count}");
    }

    #[test]
    fn counts_chinese_text() {
        let counter = TiktokenCounter::new();
        let count = counter.count("你好世界");
        assert!(count >= 2, "中文应至少 2 tokens,实际 {count}");
    }

    #[test]
    fn empty_text_is_zero() {
        let counter = TiktokenCounter::new();
        assert_eq!(counter.count(""), 0);
    }

    #[test]
    fn batch_counting() {
        let counter = TiktokenCounter::new();
        let texts = vec!["hello".to_string(), "world".to_string()];
        let counts = counter.count_batch(&texts);
        assert_eq!(counts.len(), 2);
        assert!(counts[0] > 0);
        assert!(counts[1] > 0);
    }

    #[test]
    fn calibration_adjusts_factor() {
        let counter = TiktokenCounter::new();
        let before = counter.count("test text for calibration");
        // 模拟真实 token 数是估算的 1.5 倍
        counter.calibrate(before * 3 / 2, before);
        let after = counter.count("test text for calibration");
        // 校准后估算应增大(系数从 1.2 向 1.5 移动)
        assert!(
            after >= before,
            "校准后估算应增大: before={before}, after={after}"
        );
    }

    #[test]
    fn calibration_clamped() {
        let counter = TiktokenCounter::new();
        // 极端比值不超上限
        counter.calibrate(10000, 1);
        let count = counter.count("hello");
        assert!(count < 100, "系数应有上限,实际 count={count}");
    }
}
