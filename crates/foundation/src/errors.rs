//! 统一错误类型层级。
//!
//! 区分"调用方的错"与"服务端的错";HTTP 状态码映射由 server 层统一执行(见
//! docs/modules/memory/api.md)。底层依赖(lancedb / 模型 SDK 等)的原始错误必须在
//! provider 层封装成 `KairosError::Provider`,不得穿透到上层——否则换实现时上层的
//! 匹配会失效,形成隐性耦合。
//!
//! 所有错误携带人类可读 message 与可选结构化 details;details 仅放元数据(数量、耗时、
//! id/哈希前缀),**绝不放记忆/知识明文与密钥**(安全约定)。

use std::collections::BTreeMap;

use thiserror::Error;

/// 结构化错误上下文;仅放元数据,禁明文/密钥。用 BTreeMap 保证序列化顺序稳定。
pub type ErrorDetails = BTreeMap<String, String>;

/// 所有 Kairos 错误的统一枚举。
///
/// 每个变体对应一类错误来源;`Provider` 变体承载可重试标志与被封装的底层错误,
/// 供 model_gateway 决定重试/降级(见 docs/modules/model-gateway.md §3)。
#[derive(Debug, Error)]
pub enum KairosError {
    /// 配置缺失或非法。启动时返回,fail-fast。
    #[error("{message}")]
    Config {
        message: String,
        details: ErrorDetails,
    },

    /// 调用方输入非法(未来对应 HTTP 422)。
    #[error("{message}")]
    Validation {
        message: String,
        details: ErrorDetails,
    },

    /// 外部 Provider(embedding / rerank / 向量库 / 模型)调用失败(未来对应 5xx)。
    ///
    /// 统一封装底层错误,调用方不直接看到模型 SDK / lancedb 的原始错误类型。
    /// - `provider`:出错的 provider 标识(如 "lancedb"、"openai_compat"),便于定位与记账归因。
    /// - `retryable`:是否可重试(429/5xx/网络类为 true)。
    /// - `source`:被封装的底层原始错误(保留调用链,不外泄其类型)。
    #[error("provider {provider} 调用失败: {message}")]
    Provider {
        provider: String,
        message: String,
        retryable: bool,
        details: ErrorDetails,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// 选用了需要某组件的能力,但该组件未配置。
    /// - `hint`:明确的配置修复指引(如"在 config.toml 设置 embedding.impl"),便于自助修复。
    #[error("{message}")]
    NotConfigured {
        message: String,
        hint: Option<String>,
        details: ErrorDetails,
    },
}

impl KairosError {
    /// 构造 `Config` 错误(无附加 details)。
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
            details: ErrorDetails::new(),
        }
    }

    /// 构造 `Validation` 错误(无附加 details)。
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
            details: ErrorDetails::new(),
        }
    }

    /// 构造 `Provider` 错误。`retryable` 默认由调用方显式给出。
    pub fn provider(
        provider: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self::Provider {
            provider: provider.into(),
            message: message.into(),
            retryable,
            details: ErrorDetails::new(),
            source: None,
        }
    }

    /// 构造 `NotConfigured` 错误,附可选配置指引 hint。
    pub fn not_configured(message: impl Into<String>, hint: Option<String>) -> Self {
        Self::NotConfigured {
            message: message.into(),
            hint,
            details: ErrorDetails::new(),
        }
    }

    /// 附加/覆盖底层原始错误(仅对 `Provider` 变体生效),保留调用链。
    pub fn with_source(mut self, err: impl std::error::Error + Send + Sync + 'static) -> Self {
        if let Self::Provider { source, .. } = &mut self {
            *source = Some(Box::new(err));
        }
        self
    }

    /// 附加一条结构化 details 元数据(键值),链式调用。
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details_mut().insert(key.into(), value.into());
        self
    }

    /// 只读访问 details。
    pub fn details(&self) -> &ErrorDetails {
        match self {
            Self::Config { details, .. }
            | Self::Validation { details, .. }
            | Self::Provider { details, .. }
            | Self::NotConfigured { details, .. } => details,
        }
    }

    fn details_mut(&mut self) -> &mut ErrorDetails {
        match self {
            Self::Config { details, .. }
            | Self::Validation { details, .. }
            | Self::Provider { details, .. }
            | Self::NotConfigured { details, .. } => details,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_carries_message() {
        let e = KairosError::config("boom");
        assert_eq!(e.to_string(), "boom");
        assert!(e.details().is_empty());
    }

    #[test]
    fn with_detail_accumulates() {
        let e = KairosError::validation("bad").with_detail("field", "dim");
        assert_eq!(e.details().get("field"), Some(&"dim".to_string()));
    }

    #[test]
    fn provider_carries_flags_and_source() {
        let cause = std::io::Error::other("orig");
        let e = KairosError::provider("lancedb", "failed", true).with_source(cause);
        match &e {
            KairosError::Provider {
                provider,
                retryable,
                source,
                ..
            } => {
                assert_eq!(provider, "lancedb");
                assert!(*retryable);
                assert!(source.is_some());
            }
            _ => panic!("应为 Provider 变体"),
        }
        // source 保留可经 std::error::Error::source 访问
        assert!(std::error::Error::source(&e).is_some());
    }

    #[test]
    fn provider_defaults_no_source() {
        let e = KairosError::provider("openai_compat", "f", false);
        assert!(std::error::Error::source(&e).is_none());
    }

    #[test]
    fn not_configured_carries_hint() {
        let e = KairosError::not_configured(
            "missing embedding",
            Some("在 .kairos/config.toml 设置 embedding.impl".to_string()),
        );
        match e {
            KairosError::NotConfigured { hint, .. } => {
                assert_eq!(
                    hint.as_deref(),
                    Some("在 .kairos/config.toml 设置 embedding.impl")
                );
            }
            _ => panic!("应为 NotConfigured 变体"),
        }
    }
}
