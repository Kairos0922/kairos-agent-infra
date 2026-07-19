//! 组装根:从配置装配 [`TierRouter`](crate::router::TierRouter)。
//!
//! 把 `[model_gateway]` 配置翻译成可用的 [`ModelRouter`]:按方言构造 provider、按
//! `api_key_env` 从环境变量名读取密钥(密钥绝不落代码/配置/日志)、把 tier 映射到有序部署
//! 候选。任何 provider/模型/tier 缺失都 fail-closed。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use foundation::config::{load_settings, LoadOptions};
use foundation::KairosError;
use reqwest::{Client, Url};

use crate::config::{ModelGatewayConfig, ModelGatewaySettings, ProviderDialect};
use crate::contracts::{ChatModel, ModelIdentity, ModelRouter, ModelTier};
use crate::providers::{AnthropicChatModel, OpenAiCompatChatModel};
use crate::router::{Deployment, TierRouter};

/// Anthropic 协议 `max_tokens` 为必填;请求未指定 `max_output_tokens` 时用的缺省上限。
const DEFAULT_ANTHROPIC_MAX_TOKENS: u32 = 4096;

/// HTTP 建连超时。只约束建连阶段,不限制流式响应整体时长(模型生成/思考可能很长)。
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// 从分层配置加载 `[model_gateway]` 并装配路由器。密钥按 `api_key_env` 从进程环境变量读取。
pub fn build_router(opts: &LoadOptions) -> Result<Box<dyn ModelRouter>, KairosError> {
    let settings: ModelGatewaySettings = load_settings(opts)?;
    build_router_from_config(settings.model_gateway)
}

/// 从已解析的配置装配路由器。密钥按 `api_key_env` 从进程环境变量读取。
pub fn build_router_from_config(
    config: ModelGatewayConfig,
) -> Result<Box<dyn ModelRouter>, KairosError> {
    build_router_with(config, &|name| std::env::var(name).ok())
}

/// 装配核心。密钥解析经 `resolve_key` 注入(生产读环境变量,测试用内存表),使无真实 API
/// key 的契约测试无需触碰进程环境。
fn build_router_with(
    config: ModelGatewayConfig,
    resolve_key: &dyn Fn(&str) -> Option<String>,
) -> Result<Box<dyn ModelRouter>, KairosError> {
    // connect_timeout 使半挂的 TCP(建连不响应)及时 fail-fast,避免拖垮连接池;
    // 不设请求级超时——流式生成/思考的整体时长由模型决定,不应被客户端截断。
    let client = Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .map_err(|error| {
            KairosError::config("HTTP 客户端初始化失败").with_detail("reason", error.to_string())
        })?;
    let mut deployments: BTreeMap<String, Deployment> = BTreeMap::new();

    for (alias, model_cfg) in &config.models {
        let provider_cfg = config.providers.get(&model_cfg.provider).ok_or_else(|| {
            KairosError::config("模型引用了未定义的 provider")
                .with_detail("model", alias)
                .with_detail("provider", &model_cfg.provider)
        })?;
        let api_key = resolve_key(&provider_cfg.api_key_env).ok_or_else(|| {
            KairosError::config("缺少 provider 密钥环境变量")
                .with_detail("provider", &model_cfg.provider)
                .with_detail("env", &provider_cfg.api_key_env)
        })?;
        let base_url = normalize_base_url(&provider_cfg.base_url)?;
        let identity = ModelIdentity {
            deployment: alias.clone(),
            provider: model_cfg.provider.clone(),
            model: model_cfg.model.clone(),
        };
        let model: Arc<dyn ChatModel> = match provider_cfg.dialect {
            ProviderDialect::Anthropic => Arc::new(AnthropicChatModel::new(
                client.clone(),
                identity,
                base_url,
                api_key,
                DEFAULT_ANTHROPIC_MAX_TOKENS,
            )),
            dialect => Arc::new(OpenAiCompatChatModel::new(
                client.clone(),
                identity,
                dialect,
                base_url,
                api_key,
            )),
        };
        deployments.insert(
            alias.clone(),
            Deployment {
                capabilities: model_cfg.capabilities.clone(),
                model,
            },
        );
    }

    let mut routes: BTreeMap<ModelTier, Vec<String>> = BTreeMap::new();
    for (tier, route) in &config.tiers {
        let mut candidates = vec![route.primary.clone()];
        candidates.extend(route.fallback.iter().cloned());
        for alias in &candidates {
            if !deployments.contains_key(alias) {
                return Err(KairosError::config("tier 引用了未定义的模型部署")
                    .with_detail("tier", format!("{tier:?}"))
                    .with_detail("model", alias));
            }
        }
        routes.insert(*tier, candidates);
    }

    Ok(Box::new(TierRouter::new(
        deployments,
        routes,
        config.max_retries,
    )))
}

/// 规范化 base_url 为带尾斜杠的 [`Url`],使 provider 的 `join("chat/completions")` 等相对
/// 拼接不会吞掉路径末段(如 `/v1`——无尾斜杠时 `Url::join` 会丢掉最后一段)。
fn normalize_base_url(raw: &str) -> Result<Url, KairosError> {
    let with_slash = if raw.ends_with('/') {
        raw.to_string()
    } else {
        format!("{raw}/")
    };
    Url::parse(&with_slash).map_err(|error| {
        KairosError::config("provider base_url 非法").with_detail("reason", error.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ModelCapabilities, ModelConfig, ProviderConfig, ProviderDialect, TierRoute,
    };
    use crate::contracts::{
        ChatChunk, ChatMessage, ChatRequest, GenerationOptions, ResponseFormat, ToolChoice,
    };
    use futures_util::StreamExt;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn key_resolver() -> impl Fn(&str) -> Option<String> {
        |name: &str| (name == "TEST_KEY_ENV").then(|| "test-key".to_string())
    }

    fn provider(dialect: ProviderDialect, base_url: &str) -> ProviderConfig {
        ProviderConfig {
            dialect,
            base_url: base_url.to_string(),
            api_key_env: "TEST_KEY_ENV".into(),
        }
    }

    fn model(provider: &str, capabilities: ModelCapabilities) -> ModelConfig {
        ModelConfig {
            provider: provider.into(),
            model: "test-model".into(),
            capabilities,
        }
    }

    fn stream_caps() -> ModelCapabilities {
        ModelCapabilities {
            stream: true,
            ..Default::default()
        }
    }

    fn request() -> ChatRequest {
        let mut req = ChatRequest {
            messages: vec![ChatMessage::User {
                content: "你好".into(),
            }],
            generation: GenerationOptions::default(),
            tools: Vec::new(),
            tool_choice: ToolChoice::default(),
            response_format: ResponseFormat::default(),
        };
        req.generation.stream = false;
        req
    }

    #[test]
    fn missing_provider_fails_closed() {
        let mut config = ModelGatewayConfig::default();
        config
            .models
            .insert("a".into(), model("ghost", stream_caps()));
        let err = build_router_with(config, &key_resolver()).err().unwrap();
        assert!(matches!(err, KairosError::Config { .. }));
    }

    #[test]
    fn missing_api_key_fails_closed() {
        let mut config = ModelGatewayConfig::default();
        config.providers.insert(
            "p".into(),
            provider(ProviderDialect::Openai, "http://localhost"),
        );
        config.models.insert("a".into(), model("p", stream_caps()));
        let err = build_router_with(config, &(|_: &str| -> Option<String> { None }))
            .err()
            .unwrap();
        assert!(matches!(err, KairosError::Config { .. }));
        assert_eq!(err.details().get("env"), Some(&"TEST_KEY_ENV".to_string()));
    }

    #[test]
    fn tier_referencing_unknown_model_fails_closed() {
        let mut config = ModelGatewayConfig::default();
        config.providers.insert(
            "p".into(),
            provider(ProviderDialect::Openai, "http://localhost"),
        );
        config.models.insert("a".into(), model("p", stream_caps()));
        config.tiers.insert(
            ModelTier::Strong,
            TierRoute {
                primary: "missing".into(),
                fallback: Vec::new(),
            },
        );
        let err = build_router_with(config, &key_resolver()).err().unwrap();
        assert!(matches!(err, KairosError::Config { .. }));
    }

    #[test]
    fn invalid_base_url_fails_closed() {
        let mut config = ModelGatewayConfig::default();
        config
            .providers
            .insert("p".into(), provider(ProviderDialect::Openai, "not a url"));
        config.models.insert("a".into(), model("p", stream_caps()));
        let err = build_router_with(config, &key_resolver()).err().unwrap();
        assert!(matches!(err, KairosError::Config { .. }));
    }

    #[tokio::test]
    async fn end_to_end_routes_to_wiremock_provider() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"content": "端到端成功"}, "finish_reason": "stop"}]
            })))
            .mount(&server)
            .await;

        let mut config = ModelGatewayConfig::default();
        config
            .providers
            .insert("p".into(), provider(ProviderDialect::Openai, &server.uri()));
        config.models.insert("a".into(), model("p", stream_caps()));
        config.tiers.insert(
            ModelTier::Strong,
            TierRoute {
                primary: "a".into(),
                fallback: Vec::new(),
            },
        );

        let router = build_router_with(config, &key_resolver()).unwrap();
        let chunks: Vec<ChatChunk> = router
            .stream(ModelTier::Strong, request())
            .await
            .unwrap()
            .map(|chunk| chunk.unwrap())
            .collect()
            .await;
        assert!(matches!(&chunks[0], ChatChunk::TextDelta { text } if text == "端到端成功"));
    }
}
