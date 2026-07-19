//! 按档位路由:能力筛选(fail-closed)、重试、有序 fallback。
//!
//! 上层只传 [`ModelTier`] 与规范化 [`ChatRequest`],由本模块解析出具体部署并发起调用。
//! 核心不变量——**禁止静默降级**:候选模型若不具备请求所需能力(工具/思考/结构化输出等),
//! 一律跳过;若某 tier 无任何具备所需能力的候选,则 fail-closed 报错,绝不用弱模型顶替。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use foundation::KairosError;

use crate::config::ModelCapabilities;
use crate::contracts::{
    ChatModel, ChatRequest, ChatStream, ModelRouter, ModelTier, ResponseFormat, ThinkingMode,
    ToolChoice,
};

/// 重试退避基数;第 n 次重试等待 `基数 * 2^min(n-1, MAX_BACKOFF_EXP)`。确定性指数退避,不引入随机源。
const RETRY_BASE_DELAY: Duration = Duration::from_millis(200);

/// 退避指数上限。`max_retries` 为用户可调 u8,若不封顶:指数稍大即 sleep 数小时,
/// `2u32.pow(≥32)` 更会溢出(debug panic / release 回绕)。2^7 * 200ms ≈ 25.6s 为单次退避上限。
const MAX_BACKOFF_EXP: u32 = 7;

/// 一个已装配的模型部署:能力档案 + 实际调用实现。
pub(crate) struct Deployment {
    pub(crate) capabilities: ModelCapabilities,
    pub(crate) model: Arc<dyn ChatModel>,
}

/// 按 tier 路由的 [`ModelRouter`] 实现。
pub(crate) struct TierRouter {
    /// 部署别名 → 已装配部署。
    deployments: BTreeMap<String, Deployment>,
    /// tier → 有序候选部署别名(primary 在最前,其后为 fallback)。
    routes: BTreeMap<ModelTier, Vec<String>>,
    /// 对可重试错误的最大重试次数。
    max_retries: u8,
}

impl TierRouter {
    pub(crate) fn new(
        deployments: BTreeMap<String, Deployment>,
        routes: BTreeMap<ModelTier, Vec<String>>,
        max_retries: u8,
    ) -> Self {
        Self {
            deployments,
            routes,
            max_retries,
        }
    }
}

#[async_trait]
impl ModelRouter for TierRouter {
    async fn stream(
        &self,
        tier: ModelTier,
        request: ChatRequest,
    ) -> Result<ChatStream, KairosError> {
        request.validate()?;
        let candidates = self.routes.get(&tier).ok_or_else(|| {
            KairosError::config("tier 未配置任何路由").with_detail("tier", format!("{tier:?}"))
        })?;

        let mut last_call_error: Option<KairosError> = None;
        let mut last_skip_reason: Option<KairosError> = None;
        let mut saw_eligible = false;

        for alias in candidates {
            let deployment = self.deployments.get(alias).ok_or_else(|| {
                KairosError::config("路由引用了未定义的模型部署").with_detail("model", alias)
            })?;
            // 能力筛选:不具备请求所需能力即跳过,绝不静默降级。
            if let Err(reason) = check_capabilities(&deployment.capabilities, &request) {
                last_skip_reason = Some(reason.with_detail("model", alias));
                continue;
            }
            saw_eligible = true;
            match attempt_with_retry(deployment.model.as_ref(), &request, self.max_retries).await {
                Ok(stream) => return Ok(stream),
                Err(error) => last_call_error = Some(error),
            }
        }

        if saw_eligible {
            // 有具备能力的候选,但全部调用失败:上报最后一次调用错误。
            Err(last_call_error.expect("有候选调用失败则必有错误"))
        } else {
            // 无任何候选具备请求所需能力:fail-closed,给出可读原因。
            Err(last_skip_reason.unwrap_or_else(|| {
                KairosError::validation("该 tier 没有模型具备请求所需的能力")
                    .with_detail("tier", format!("{tier:?}"))
            }))
        }
    }
}

/// 校验模型能力是否满足请求所需。返回首个不满足项的错误(fail-closed),全满足则 Ok。
fn check_capabilities(caps: &ModelCapabilities, request: &ChatRequest) -> Result<(), KairosError> {
    let gen = &request.generation;

    if gen.stream && !caps.stream {
        return Err(cap_err("stream"));
    }

    // 工具能力。
    if !request.tools.is_empty() {
        if !caps.tools {
            return Err(cap_err("tools"));
        }
        match &request.tool_choice {
            ToolChoice::Required if !caps.tool_choice_required => {
                return Err(cap_err("tool_choice_required"))
            }
            ToolChoice::Specific { .. } if !caps.tool_choice_specific => {
                return Err(cap_err("tool_choice_specific"))
            }
            _ => {}
        }
    }

    // 思考与推理强度。
    let thinking_on = matches!(gen.thinking.mode, ThinkingMode::Enabled);
    if thinking_on && !caps.thinking {
        return Err(cap_err("thinking"));
    }
    if let Some(effort) = gen.thinking.reasoning_effort {
        if !caps.thinking {
            return Err(cap_err("thinking"));
        }
        if !caps.reasoning_efforts.contains(&effort) {
            return Err(cap_err("reasoning_effort").with_detail("effort", format!("{effort:?}")));
        }
    }

    // 采样参数。
    if gen.temperature.is_some() && !caps.temperature {
        return Err(cap_err("temperature"));
    }
    if gen.top_p.is_some() && !caps.top_p {
        return Err(cap_err("top_p"));
    }
    let sampling = gen.temperature.is_some() || gen.top_p.is_some();
    if thinking_on && sampling && !caps.sampling_when_thinking {
        return Err(cap_err("sampling_when_thinking"));
    }

    // 输出长度上限。
    if let (Some(requested), Some(limit)) = (gen.max_output_tokens, caps.max_output_tokens) {
        if requested > limit {
            return Err(cap_err("max_output_tokens").with_detail("limit", limit.to_string()));
        }
    }

    // 结构化输出。
    match &request.response_format {
        ResponseFormat::JsonObject if !caps.json_object => return Err(cap_err("json_object")),
        ResponseFormat::JsonSchema { .. } if !caps.json_schema => {
            return Err(cap_err("json_schema"))
        }
        _ => {}
    }

    Ok(())
}

fn cap_err(capability: &str) -> KairosError {
    KairosError::validation("模型不具备请求所需的能力").with_detail("capability", capability)
}

/// 第 `retry`(从 1 计)次重试的退避时长,指数封顶于 [`MAX_BACKOFF_EXP`],杜绝溢出与时长爆炸。
fn backoff_duration(retry: u32) -> Duration {
    RETRY_BASE_DELAY * 2u32.pow((retry - 1).min(MAX_BACKOFF_EXP))
}

/// 对可重试错误做指数退避重试;不可重试错误或重试耗尽即返回。
async fn attempt_with_retry(
    model: &dyn ChatModel,
    request: &ChatRequest,
    max_retries: u8,
) -> Result<ChatStream, KairosError> {
    let mut retries: u32 = 0;
    loop {
        match model.stream(request.clone()).await {
            Ok(stream) => return Ok(stream),
            Err(error) if error.is_retryable() && retries < u32::from(max_retries) => {
                retries += 1;
                tokio::time::sleep(backoff_duration(retries)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::{ChatChunk, GenerationOptions, StopReason, ToolChoice, ToolDefinition};
    use futures_util::StreamExt;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// 假模型每次调用产生的结果。
    enum Outcome {
        Success,
        RetryableErr,
        FatalErr,
    }

    /// 可脚本化的假模型:记录调用次数,按调用序号取结果(超出脚本则重复最后一项)。
    struct FakeModel {
        calls: Arc<AtomicUsize>,
        outcomes: Vec<Outcome>,
    }

    impl FakeModel {
        fn new(outcomes: Vec<Outcome>) -> (Arc<Self>, Arc<AtomicUsize>) {
            let calls = Arc::new(AtomicUsize::new(0));
            let model = Arc::new(Self {
                calls: calls.clone(),
                outcomes,
            });
            (model, calls)
        }
    }

    #[async_trait]
    impl ChatModel for FakeModel {
        async fn stream(&self, _request: ChatRequest) -> Result<ChatStream, KairosError> {
            let idx = self.calls.fetch_add(1, Ordering::SeqCst);
            let outcome = self
                .outcomes
                .get(idx)
                .or_else(|| self.outcomes.last())
                .unwrap();
            match outcome {
                Outcome::Success => Ok(Box::pin(futures_util::stream::iter(vec![Ok(
                    ChatChunk::Stop {
                        reason: StopReason::EndTurn,
                    },
                )]))),
                Outcome::RetryableErr => Err(KairosError::provider("fake", "可重试故障", true)),
                Outcome::FatalErr => Err(KairosError::provider("fake", "致命故障", false)),
            }
        }
    }

    fn caps() -> ModelCapabilities {
        ModelCapabilities {
            stream: true,
            ..Default::default()
        }
    }

    fn request() -> ChatRequest {
        ChatRequest {
            messages: vec![crate::contracts::ChatMessage::User {
                content: "hi".into(),
            }],
            generation: GenerationOptions::default(),
            tools: Vec::new(),
            tool_choice: ToolChoice::default(),
            response_format: crate::contracts::ResponseFormat::default(),
        }
    }

    fn request_with_tools() -> ChatRequest {
        let mut req = request();
        req.tools.push(ToolDefinition {
            name: "weather".into(),
            description: "天气".into(),
            parameters: json!({}),
        });
        req
    }

    fn router_with(deployments: Vec<(&str, ModelCapabilities, Arc<dyn ChatModel>)>) -> TierRouter {
        let mut deps = BTreeMap::new();
        for (alias, capabilities, model) in deployments {
            deps.insert(
                alias.to_string(),
                Deployment {
                    capabilities,
                    model,
                },
            );
        }
        let mut routes = BTreeMap::new();
        routes.insert(ModelTier::Strong, deps.keys().cloned().collect());
        TierRouter::new(deps, routes, 2)
    }

    async fn is_ok(router: &TierRouter, req: ChatRequest) -> bool {
        match router.stream(ModelTier::Strong, req).await {
            Ok(mut stream) => stream.next().await.is_some(),
            Err(_) => false,
        }
    }

    #[tokio::test]
    async fn capability_gap_fails_closed_without_calling_model() {
        let (model, calls) = FakeModel::new(vec![Outcome::Success]);
        // 模型不支持 tools,而请求要求 tools。
        let router = router_with(vec![("a", caps(), model)]);
        let err = router
            .stream(ModelTier::Strong, request_with_tools())
            .await
            .err()
            .unwrap();
        assert!(matches!(err, KairosError::Validation { .. }));
        assert_eq!(err.details().get("capability"), Some(&"tools".to_string()));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "不具备能力的模型不得被调用"
        );
    }

    #[tokio::test]
    async fn falls_back_to_capable_candidate() {
        // primary 不支持 tools(被跳过),fallback 支持且成功。
        let (primary, primary_calls) = FakeModel::new(vec![Outcome::Success]);
        let mut fallback_caps = caps();
        fallback_caps.tools = true;
        let (fallback, fallback_calls) = FakeModel::new(vec![Outcome::Success]);

        let mut deps = BTreeMap::new();
        deps.insert(
            "primary".to_string(),
            Deployment {
                capabilities: caps(),
                model: primary,
            },
        );
        deps.insert(
            "fallback".to_string(),
            Deployment {
                capabilities: fallback_caps,
                model: fallback,
            },
        );
        let mut routes = BTreeMap::new();
        routes.insert(
            ModelTier::Strong,
            vec!["primary".to_string(), "fallback".to_string()],
        );
        let router = TierRouter::new(deps, routes, 2);

        assert!(is_ok(&router, request_with_tools()).await);
        assert_eq!(
            primary_calls.load(Ordering::SeqCst),
            0,
            "无能力的 primary 不得被调用"
        );
        assert_eq!(fallback_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn no_eligible_candidate_fails_closed() {
        let (a, a_calls) = FakeModel::new(vec![Outcome::Success]);
        let (b, b_calls) = FakeModel::new(vec![Outcome::Success]);
        let router = router_with(vec![("a", caps(), a), ("b", caps(), b)]);
        let err = router
            .stream(ModelTier::Strong, request_with_tools())
            .await
            .err()
            .unwrap();
        assert!(matches!(err, KairosError::Validation { .. }));
        assert_eq!(
            a_calls.load(Ordering::SeqCst) + b_calls.load(Ordering::SeqCst),
            0
        );
    }

    #[tokio::test]
    async fn retries_retryable_error_then_succeeds() {
        let (model, calls) = FakeModel::new(vec![
            Outcome::RetryableErr,
            Outcome::RetryableErr,
            Outcome::Success,
        ]);
        let router = router_with(vec![("a", caps(), model)]);
        assert!(is_ok(&router, request()).await);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            3,
            "两次可重试失败后第三次成功"
        );
    }

    #[tokio::test]
    async fn retry_exhausted_falls_back() {
        // primary 恒可重试失败(重试耗尽),fallback 成功。
        let (primary, primary_calls) = FakeModel::new(vec![Outcome::RetryableErr]);
        let (fallback, fallback_calls) = FakeModel::new(vec![Outcome::Success]);
        let mut deps = BTreeMap::new();
        deps.insert(
            "primary".to_string(),
            Deployment {
                capabilities: caps(),
                model: primary,
            },
        );
        deps.insert(
            "fallback".to_string(),
            Deployment {
                capabilities: caps(),
                model: fallback,
            },
        );
        let mut routes = BTreeMap::new();
        routes.insert(
            ModelTier::Strong,
            vec!["primary".to_string(), "fallback".to_string()],
        );
        let router = TierRouter::new(deps, routes, 2);

        assert!(is_ok(&router, request()).await);
        assert_eq!(primary_calls.load(Ordering::SeqCst), 3, "1 次 + 2 次重试");
        assert_eq!(fallback_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn non_retryable_error_does_not_retry() {
        let (primary, primary_calls) = FakeModel::new(vec![Outcome::FatalErr]);
        let (fallback, fallback_calls) = FakeModel::new(vec![Outcome::Success]);
        let mut deps = BTreeMap::new();
        deps.insert(
            "primary".to_string(),
            Deployment {
                capabilities: caps(),
                model: primary,
            },
        );
        deps.insert(
            "fallback".to_string(),
            Deployment {
                capabilities: caps(),
                model: fallback,
            },
        );
        let mut routes = BTreeMap::new();
        routes.insert(
            ModelTier::Strong,
            vec!["primary".to_string(), "fallback".to_string()],
        );
        let router = TierRouter::new(deps, routes, 2);

        assert!(is_ok(&router, request()).await);
        assert_eq!(
            primary_calls.load(Ordering::SeqCst),
            1,
            "不可重试错误不得重试"
        );
        assert_eq!(fallback_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn unconfigured_tier_fails_closed() {
        let router = TierRouter::new(BTreeMap::new(), BTreeMap::new(), 2);
        let err = router
            .stream(ModelTier::Cheap, request())
            .await
            .err()
            .unwrap();
        assert!(matches!(err, KairosError::Config { .. }));
    }

    #[tokio::test]
    async fn invalid_request_is_rejected_before_routing() {
        let (model, calls) = FakeModel::new(vec![Outcome::Success]);
        let router = router_with(vec![("a", caps(), model)]);
        let mut req = request();
        req.generation.temperature = Some(0.5);
        req.generation.top_p = Some(0.9); // 与 temperature 冲突
        assert!(router.stream(ModelTier::Strong, req).await.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    /// 断言:请求所需能力在 caps 中缺失时,router fail-closed 拒绝且不调用模型。
    async fn assert_rejected(caps: ModelCapabilities, req: ChatRequest, expected_cap: &str) {
        let (model, calls) = FakeModel::new(vec![Outcome::Success]);
        let router = router_with(vec![("a", caps, model)]);
        let err = router.stream(ModelTier::Strong, req).await.err().unwrap();
        assert!(matches!(err, KairosError::Validation { .. }));
        assert_eq!(
            err.details().get("capability"),
            Some(&expected_cap.to_string())
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0, "缺能力不得调用模型");
    }

    #[tokio::test]
    async fn capability_gates_are_fail_closed() {
        use crate::contracts::{ReasoningEffort, ResponseFormat, ThinkingMode};

        // temperature / top_p
        let mut req = request();
        req.generation.temperature = Some(0.7);
        assert_rejected(caps(), req, "temperature").await;
        let mut req = request();
        req.generation.top_p = Some(0.9);
        assert_rejected(caps(), req, "top_p").await;

        // max_output_tokens 超上限
        let mut req = request();
        req.generation.max_output_tokens = Some(8192);
        let mut c = caps();
        c.max_output_tokens = Some(4096);
        assert_rejected(c, req, "max_output_tokens").await;

        // json_object / json_schema
        let mut req = request();
        req.response_format = ResponseFormat::JsonObject;
        assert_rejected(caps(), req, "json_object").await;
        let mut req = request();
        req.response_format = ResponseFormat::JsonSchema {
            name: "out".into(),
            schema: json!({"type": "object"}),
            strict: false,
        };
        assert_rejected(caps(), req, "json_schema").await;

        // thinking / reasoning_effort 未声明档位
        let mut req = request();
        req.generation.thinking.mode = ThinkingMode::Enabled;
        assert_rejected(caps(), req, "thinking").await;
        let mut req = request();
        req.generation.thinking.reasoning_effort = Some(ReasoningEffort::Medium);
        let mut c = caps();
        c.thinking = true;
        c.reasoning_efforts = vec![ReasoningEffort::High]; // 不含 Medium
        assert_rejected(c, req, "reasoning_effort").await;

        // tool_choice required / specific
        let mut req = request_with_tools();
        req.tool_choice = ToolChoice::Required;
        let mut c = caps();
        c.tools = true;
        assert_rejected(c, req, "tool_choice_required").await;
        let mut req = request_with_tools();
        req.tool_choice = ToolChoice::Specific {
            name: "weather".into(),
        };
        let mut c = caps();
        c.tools = true;
        assert_rejected(c, req, "tool_choice_specific").await;

        // thinking + 采样 需 sampling_when_thinking
        let mut req = request();
        req.generation.thinking.mode = ThinkingMode::Enabled;
        req.generation.temperature = Some(0.7);
        let mut c = caps();
        c.thinking = true;
        c.temperature = true;
        assert_rejected(c, req, "sampling_when_thinking").await;

        // stream
        let no_stream = ModelCapabilities {
            stream: false,
            ..Default::default()
        };
        assert_rejected(no_stream, request(), "stream").await;
    }

    #[tokio::test]
    async fn fully_capable_request_passes_all_gates() {
        let caps = ModelCapabilities {
            stream: true,
            tools: true,
            tool_choice_required: true,
            tool_choice_specific: true,
            thinking: true,
            reasoning_efforts: vec![crate::contracts::ReasoningEffort::High],
            temperature: true,
            top_p: true,
            sampling_when_thinking: true,
            json_object: true,
            json_schema: true,
            max_output_tokens: Some(8192),
        };
        let (model, _) = FakeModel::new(vec![Outcome::Success]);
        let router = router_with(vec![("a", caps, model)]);
        assert!(is_ok(&router, request()).await);
    }

    #[tokio::test]
    async fn zero_max_retries_does_not_retry() {
        let (primary, primary_calls) = FakeModel::new(vec![Outcome::RetryableErr]);
        let (fallback, fallback_calls) = FakeModel::new(vec![Outcome::Success]);
        let mut deps = BTreeMap::new();
        deps.insert(
            "primary".to_string(),
            Deployment {
                capabilities: caps(),
                model: primary,
            },
        );
        deps.insert(
            "fallback".to_string(),
            Deployment {
                capabilities: caps(),
                model: fallback,
            },
        );
        let mut routes = BTreeMap::new();
        routes.insert(
            ModelTier::Strong,
            vec!["primary".to_string(), "fallback".to_string()],
        );
        let router = TierRouter::new(deps, routes, 0);
        assert!(is_ok(&router, request()).await);
        assert_eq!(
            primary_calls.load(Ordering::SeqCst),
            1,
            "max_retries=0 不得重试"
        );
        assert_eq!(fallback_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn backoff_duration_is_capped_and_never_overflows() {
        // 指数封顶:第 8 次及以后退避时长恒定为 2^7 * 基数。
        let capped = backoff_duration(8);
        assert_eq!(capped, RETRY_BASE_DELAY * 128);
        assert_eq!(backoff_duration(9), capped);
        assert_eq!(backoff_duration(255), capped); // 极大重试次数也不溢出
                                                   // 前几次正常指数增长。
        assert_eq!(backoff_duration(1), RETRY_BASE_DELAY);
        assert_eq!(backoff_duration(2), RETRY_BASE_DELAY * 2);
        assert_eq!(backoff_duration(3), RETRY_BASE_DELAY * 4);
    }
}
