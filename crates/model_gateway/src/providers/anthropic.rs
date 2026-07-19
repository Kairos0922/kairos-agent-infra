//! Anthropic Messages 协议适配。其事件与 OpenAI Chat Completions 不可混用。

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use async_trait::async_trait;
use eventsource_stream::Eventsource;
use foundation::KairosError;
use futures_util::{stream, StreamExt};
use reqwest::{Client, StatusCode, Url};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::contracts::{
    ChatChunk, ChatMessage, ChatModel, ChatRequest, ChatStream, ModelIdentity, ResponseFormat,
    StopReason, ThinkingMode, TokenUsage, ToolCall, ToolChoice,
};

/// Anthropic Messages 协议适配。
///
/// 安全约定:持有 `api_key` 明文,故**刻意不 `derive(Debug)`**——避免将来误打印/panic 泄露密钥。
pub(crate) struct AnthropicChatModel {
    client: Client,
    identity: ModelIdentity,
    base_url: Url,
    api_key: String,
    default_max_output_tokens: u32,
}

impl AnthropicChatModel {
    pub(crate) fn new(
        client: Client,
        identity: ModelIdentity,
        base_url: Url,
        api_key: String,
        default_max_output_tokens: u32,
    ) -> Self {
        Self {
            client,
            identity,
            base_url,
            api_key,
            default_max_output_tokens,
        }
    }

    fn endpoint(&self) -> Result<Url, KairosError> {
        self.base_url.join("v1/messages").map_err(|error| {
            provider_error(
                &self.identity.provider,
                "无效的 Anthropic 端点",
                false,
                error,
            )
        })
    }

    fn request_body(&self, request: &ChatRequest) -> Value {
        let (system, messages) = anthropic_messages(&request.messages);
        let mut body = json!({
            "model": self.identity.model,
            "max_tokens": request.generation.max_output_tokens.unwrap_or(self.default_max_output_tokens),
            "messages": messages,
            "stream": request.generation.stream,
        });
        let object = body.as_object_mut().expect("json object");
        if !system.is_empty() {
            object.insert("system".to_string(), Value::String(system));
        }
        if let Some(value) = request.generation.temperature {
            object.insert("temperature".to_string(), json!(value));
        }
        if let Some(value) = request.generation.top_p {
            object.insert("top_p".to_string(), json!(value));
        }
        if !request.generation.stop_sequences.is_empty() {
            object.insert(
                "stop_sequences".to_string(),
                json!(request.generation.stop_sequences),
            );
        }
        if !request.tools.is_empty() {
            object.insert(
                "tools".to_string(),
                Value::Array(
                    request
                        .tools
                        .iter()
                        .map(|tool| {
                            json!({
                                "name": tool.name,
                                "description": tool.description,
                                "input_schema": tool.parameters,
                            })
                        })
                        .collect(),
                ),
            );
        }
        // Anthropic 支持 {"type":"none"} 显式禁用工具(2025 起,见官方 tool-use 文档),
        // 但要求 tools 非空否则报错;无 tools 时本就没有工具可调用,不发该字段。
        // None 不得并入 Auto(默认 auto 仍会调工具,违背调用方"别调工具"的明确意图)。
        let tool_choice = match &request.tool_choice {
            ToolChoice::None => (!request.tools.is_empty()).then(|| json!({"type": "none"})),
            ToolChoice::Auto => None,
            ToolChoice::Required => Some(json!({"type": "any"})),
            ToolChoice::Specific { name } => Some(json!({"type": "tool", "name": name})),
        };
        if let Some(tool_choice) = tool_choice {
            object.insert("tool_choice".to_string(), tool_choice);
        }
        body
    }
}

/// 本适配器尚未实现的能力:显式请求即 fail-closed 拒绝,绝不静默不兑现。
/// 与 router 的能力筛选互为纵深防御——即便配置误声明能力,也在此响亮报错而非偷偷降级。
/// - thinking/reasoning_effort:Anthropic extended thinking 尚未接入(后续任务);
/// - 结构化输出:Anthropic 无原生 `response_format`,**不以 prompt 冒充**(交接红线),
///   能力档案应声明 `json_object`/`json_schema`=false 由 router 先行拒绝,此处兜底。
fn reject_unsupported(request: &ChatRequest) -> Result<(), KairosError> {
    let thinking = &request.generation.thinking;
    if matches!(thinking.mode, ThinkingMode::Enabled) || thinking.reasoning_effort.is_some() {
        return Err(KairosError::validation(
            "Anthropic 适配器暂未实现 thinking/reasoning_effort,请勿为该部署声明相应能力",
        ));
    }
    if !matches!(request.response_format, ResponseFormat::Text) {
        return Err(KairosError::validation(
            "Anthropic 适配器无原生结构化输出(不以 prompt 冒充),请勿声明 json_object/json_schema 能力",
        ));
    }
    Ok(())
}

#[async_trait]
impl ChatModel for AnthropicChatModel {
    async fn stream(&self, request: ChatRequest) -> Result<ChatStream, KairosError> {
        reject_unsupported(&request)?;
        let response = self
            .client
            .post(self.endpoint()?)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&self.request_body(&request))
            .send()
            .await
            .map_err(|error| request_error(&self.identity.provider, error))?;
        ensure_success(&self.identity.provider, response.status())?;

        if !request.generation.stream {
            let body: AnthropicCompletion = response
                .json()
                .await
                .map_err(|error| request_error(&self.identity.provider, error))?;
            return Ok(Box::pin(stream::iter(non_stream_chunks(
                body,
                self.identity.clone(),
            ))));
        }

        let events = response.bytes_stream().eventsource();
        // identity/provider 包成 Arc:try_unfold 每产出一个 chunk 就重调一次外层闭包,
        // Arc::clone 仅原子引用计数,避免按 chunk 克隆 identity 的三个 String。
        let provider: Arc<str> = self.identity.provider.as_str().into();
        let identity = Arc::new(self.identity.clone());
        let output = stream::try_unfold(
            (
                events,
                BTreeMap::<usize, AnthropicToolState>::new(),
                VecDeque::<ChatChunk>::new(),
            ),
            move |(mut events, mut tools, mut pending)| {
                let provider = Arc::clone(&provider);
                let identity = Arc::clone(&identity);
                async move {
                    loop {
                        if let Some(chunk) = pending.pop_front() {
                            return Ok(Some((chunk, (events, tools, pending))));
                        }
                        let Some(event) = events.next().await else {
                            return Ok(None);
                        };
                        let event = event.map_err(|error| {
                            KairosError::provider(&*provider, "SSE 解析失败", true)
                                .with_detail("reason", error.to_string())
                        })?;
                        let parsed: AnthropicEvent =
                            serde_json::from_str(&event.data).map_err(|error| {
                                KairosError::provider(&*provider, "模型流式响应格式非法", false)
                                    .with_detail("reason", error.to_string())
                            })?;
                        pending.extend(stream_chunks(parsed, &identity, &mut tools)?);
                    }
                }
            },
        );
        Ok(Box::pin(output))
    }
}

fn anthropic_messages(messages: &[ChatMessage]) -> (String, Vec<Value>) {
    let mut system = Vec::new();
    let mut output = Vec::new();
    for message in messages {
        match message {
            ChatMessage::System { content } | ChatMessage::Developer { content } => system.push(content.as_str()),
            ChatMessage::User { content } => output.push(json!({"role": "user", "content": content})),
            ChatMessage::Assistant {
                content, tool_calls, ..
            } => {
                let mut blocks = Vec::new();
                if let Some(content) = content {
                    blocks.push(json!({"type": "text", "text": content}));
                }
                for call in tool_calls {
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": call.id,
                        "name": call.name,
                        "input": call.arguments,
                    }));
                }
                output.push(json!({"role": "assistant", "content": blocks}));
            }
            ChatMessage::Tool {
                tool_call_id,
                content,
            } => output.push(json!({
                "role": "user",
                "content": [{"type": "tool_result", "tool_use_id": tool_call_id, "content": content}],
            })),
        }
    }
    (system.join("\n\n"), output)
}

fn ensure_success(provider: &str, status: StatusCode) -> Result<(), KairosError> {
    if status.is_success() {
        return Ok(());
    }
    let retryable = status == StatusCode::REQUEST_TIMEOUT
        || status == StatusCode::TOO_MANY_REQUESTS
        || status.is_server_error();
    // 刻意**不读取** provider 错误响应体:其内容可能回显请求片段或含内部 trace,读入 details
    // 会制造泄露面。仅记 http_status 元数据。此为安全约束,勿为可调试性改回读 body。
    Err(
        KairosError::provider(provider, "模型服务返回失败状态", retryable)
            .with_detail("http_status", status.as_u16().to_string()),
    )
}

fn request_error(provider: &str, error: reqwest::Error) -> KairosError {
    provider_error(
        provider,
        "模型 HTTP 调用失败",
        error.is_timeout() || error.is_connect() || error.is_request(),
        error,
    )
}

fn provider_error(
    provider: &str,
    message: &str,
    retryable: bool,
    error: impl std::error::Error + Send + Sync + 'static,
) -> KairosError {
    KairosError::provider(provider, message, retryable).with_source(error)
}

#[derive(Debug, Deserialize)]
struct AnthropicEvent {
    #[serde(rename = "type")]
    kind: String,
    index: Option<usize>,
    content_block: Option<AnthropicBlock>,
    delta: Option<AnthropicDelta>,
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
struct AnthropicBlock {
    #[serde(rename = "type")]
    kind: String,
    id: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicDelta {
    /// 仅 `content_block_delta` 的内层 delta 带 `type`(text_delta / input_json_delta);
    /// `message_delta` 的内层 delta 只有 stop_reason、无 type,故此处可选。
    #[serde(rename = "type", default)]
    kind: Option<String>,
    text: Option<String>,
    partial_json: Option<String>,
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
}

#[derive(Debug)]
struct AnthropicToolState {
    id: String,
    name: String,
    arguments: String,
}

fn stream_chunks(
    event: AnthropicEvent,
    identity: &ModelIdentity,
    tools: &mut BTreeMap<usize, AnthropicToolState>,
) -> Result<Vec<ChatChunk>, KairosError> {
    let mut out = Vec::new();
    match event.kind.as_str() {
        "message_start" => {
            if let Some(usage) = event.usage {
                out.push(ChatChunk::Usage {
                    usage: to_usage(usage, identity.clone()),
                });
            }
        }
        "content_block_start" => {
            if let (Some(index), Some(block)) = (event.index, event.content_block) {
                if block.kind == "tool_use" {
                    tools.insert(
                        index,
                        AnthropicToolState {
                            id: block.id.unwrap_or_else(|| format!("tool_call_{index}")),
                            name: block.name.unwrap_or_default(),
                            arguments: String::new(),
                        },
                    );
                }
            }
        }
        "content_block_delta" => {
            if let (Some(index), Some(delta)) = (event.index, event.delta) {
                match delta.kind.as_deref() {
                    Some("text_delta") => {
                        if let Some(text) = delta.text {
                            out.push(ChatChunk::TextDelta { text });
                        }
                    }
                    Some("input_json_delta") => {
                        if let Some(state) = tools.get_mut(&index) {
                            let delta = delta.partial_json.unwrap_or_default();
                            state.arguments.push_str(&delta);
                            out.push(ChatChunk::ToolCallDelta {
                                id: state.id.clone(),
                                name: Some(state.name.clone()),
                                arguments_delta: delta,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        "content_block_stop" => {
            if let Some(index) = event.index {
                if let Some(state) = tools.remove(&index) {
                    let arguments = serde_json::from_str(&state.arguments).map_err(|error| {
                        KairosError::provider("anthropic", "工具参数不是有效 JSON", false)
                            .with_detail("reason", error.to_string())
                    })?;
                    out.push(ChatChunk::ToolCallComplete {
                        call: ToolCall {
                            id: state.id,
                            name: state.name,
                            arguments,
                        },
                        provider_resume_state: None,
                    });
                }
            }
        }
        "message_delta" => {
            if let Some(delta) = event.delta {
                if let Some(reason) = delta.stop_reason {
                    out.push(ChatChunk::Stop {
                        reason: parse_stop_reason(&reason),
                    });
                }
            }
            if let Some(usage) = event.usage {
                out.push(ChatChunk::Usage {
                    usage: to_usage(usage, identity.clone()),
                });
            }
        }
        _ => {}
    }
    Ok(out)
}

#[derive(Debug, Deserialize)]
struct AnthropicCompletion {
    #[serde(default)]
    content: Vec<AnthropicCompletionBlock>,
    stop_reason: Option<String>,
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
struct AnthropicCompletionBlock {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
    id: Option<String>,
    name: Option<String>,
    input: Option<Value>,
}

fn non_stream_chunks(
    response: AnthropicCompletion,
    identity: ModelIdentity,
) -> Vec<Result<ChatChunk, KairosError>> {
    let mut out = Vec::new();
    for block in response.content {
        match block.kind.as_str() {
            "text" => {
                if let Some(text) = block.text {
                    out.push(Ok(ChatChunk::TextDelta { text }));
                }
            }
            "tool_use" => out.push(Ok(ChatChunk::ToolCallComplete {
                call: ToolCall {
                    id: block.id.unwrap_or_default(),
                    name: block.name.unwrap_or_default(),
                    arguments: block.input.unwrap_or(Value::Null),
                },
                provider_resume_state: None,
            })),
            _ => {}
        }
    }
    if let Some(reason) = response.stop_reason {
        out.push(Ok(ChatChunk::Stop {
            reason: parse_stop_reason(&reason),
        }));
    }
    if let Some(usage) = response.usage {
        out.push(Ok(ChatChunk::Usage {
            usage: to_usage(usage, identity),
        }));
    }
    out
}

fn to_usage(usage: AnthropicUsage, identity: ModelIdentity) -> TokenUsage {
    TokenUsage {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        reasoning_tokens: None,
        cached_input_tokens: usage.cache_read_input_tokens,
        model: Some(identity),
    }
}

fn parse_stop_reason(reason: &str) -> StopReason {
    match reason {
        "tool_use" => StopReason::ToolUse,
        "max_tokens" => StopReason::MaxTokens,
        _ => StopReason::EndTurn,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::{
        ChatChunk, ChatMessage, ChatRequest, GenerationOptions, ReasoningEffort, ResponseFormat,
        StopReason, ThinkingMode, ToolChoice, ToolDefinition,
    };
    use futures_util::StreamExt;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn identity() -> ModelIdentity {
        ModelIdentity {
            deployment: "dep".into(),
            provider: "claude".into(),
            model: "claude-test".into(),
        }
    }

    fn model_for(server: &MockServer) -> AnthropicChatModel {
        AnthropicChatModel::new(
            Client::new(),
            identity(),
            Url::parse(&format!("{}/", server.uri())).unwrap(),
            "test-key".into(),
            4096,
        )
    }

    fn user_request() -> ChatRequest {
        ChatRequest {
            messages: vec![ChatMessage::User {
                content: "你好".into(),
            }],
            generation: GenerationOptions::default(),
            tools: Vec::new(),
            tool_choice: ToolChoice::default(),
            response_format: ResponseFormat::default(),
        }
    }

    fn sse(events: &[&str]) -> String {
        let mut out = String::new();
        for event in events {
            out.push_str("data: ");
            out.push_str(event);
            out.push_str("\n\n");
        }
        out
    }

    async fn collect(stream: ChatStream) -> Vec<ChatChunk> {
        stream
            .map(|chunk| chunk.expect("流不应出错"))
            .collect()
            .await
    }

    async fn received_body(server: &MockServer) -> Value {
        let requests = server.received_requests().await.expect("应收到请求");
        serde_json::from_slice(&requests[0].body).expect("请求体应为 JSON")
    }

    #[tokio::test]
    async fn sends_api_key_headers_and_required_max_tokens() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"content": []})))
            .mount(&server)
            .await;
        let mut req = user_request();
        req.generation.stream = false;
        let _ = model_for(&server).stream(req).await.unwrap();

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests[0].headers.get("x-api-key").unwrap(), "test-key");
        assert_eq!(
            requests[0].headers.get("anthropic-version").unwrap(),
            "2023-06-01"
        );
        // max_tokens 为 Anthropic 必填,请求未指定时用缺省值。
        assert_eq!(received_body(&server).await["max_tokens"], json!(4096));
    }

    #[tokio::test]
    async fn hoists_system_and_developer_into_system_field() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"content": []})))
            .mount(&server)
            .await;
        let mut req = user_request();
        req.generation.stream = false;
        req.messages.insert(
            0,
            ChatMessage::System {
                content: "甲".into(),
            },
        );
        req.messages.insert(
            1,
            ChatMessage::Developer {
                content: "乙".into(),
            },
        );
        let _ = model_for(&server).stream(req).await.unwrap();

        let body = received_body(&server).await;
        assert_eq!(body["system"], json!("甲\n\n乙"));
        // messages 只保留 user/assistant 轮次。
        assert_eq!(body["messages"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn non_stream_parses_text_tool_stop_usage() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "content": [
                    {"type": "text", "text": "你好"},
                    {"type": "tool_use", "id": "tu_1", "name": "weather", "input": {"city": "北京"}}
                ],
                "stop_reason": "tool_use",
                "usage": {"input_tokens": 3, "output_tokens": 5, "cache_read_input_tokens": 1}
            })))
            .mount(&server)
            .await;
        let mut req = user_request();
        req.generation.stream = false;
        let chunks = collect(model_for(&server).stream(req).await.unwrap()).await;

        assert!(matches!(&chunks[0], ChatChunk::TextDelta { text } if text == "你好"));
        match &chunks[1] {
            ChatChunk::ToolCallComplete { call, .. } => {
                assert_eq!(call.id, "tu_1");
                assert_eq!(call.name, "weather");
                assert_eq!(call.arguments, json!({"city": "北京"}));
            }
            other => panic!("应为 ToolCallComplete,实际 {other:?}"),
        }
        assert!(matches!(
            &chunks[2],
            ChatChunk::Stop {
                reason: StopReason::ToolUse
            }
        ));
        match &chunks[3] {
            ChatChunk::Usage { usage } => {
                assert_eq!(usage.input_tokens, Some(3));
                assert_eq!(usage.output_tokens, Some(5));
                assert_eq!(usage.cached_input_tokens, Some(1));
            }
            other => panic!("应为 Usage,实际 {other:?}"),
        }
    }

    #[tokio::test]
    async fn stream_parses_text_and_tool_use() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(sse(&[
                        r#"{"type":"message_start","usage":{"input_tokens":4}}"#,
                        r#"{"type":"content_block_start","index":0,"content_block":{"type":"text"}}"#,
                        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#,
                        r#"{"type":"content_block_stop","index":0}"#,
                        r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"tu_1","name":"weather"}}"#,
                        r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"city\":"}}"#,
                        r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"\"北京\"}"}}"#,
                        r#"{"type":"content_block_stop","index":1}"#,
                        r#"{"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":6}}"#,
                    ])),
            )
            .mount(&server)
            .await;
        let chunks = collect(model_for(&server).stream(user_request()).await.unwrap()).await;

        assert!(chunks
            .iter()
            .any(|c| matches!(c, ChatChunk::TextDelta { text } if text == "Hello")));
        assert!(chunks.iter().any(|c| matches!(
            c,
            ChatChunk::ToolCallComplete { call, .. }
                if call.id == "tu_1" && call.name == "weather" && call.arguments == json!({"city": "北京"})
        )));
        assert!(chunks.iter().any(|c| matches!(
            c,
            ChatChunk::Stop {
                reason: StopReason::ToolUse
            }
        )));
    }

    #[tokio::test]
    async fn maps_tool_choice_and_input_schema() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"content": []})))
            .mount(&server)
            .await;
        let mut req = user_request();
        req.generation.stream = false;
        req.tools.push(ToolDefinition {
            name: "weather".into(),
            description: "天气".into(),
            parameters: json!({"type": "object"}),
        });
        req.tool_choice = ToolChoice::Required;
        let _ = model_for(&server).stream(req).await.unwrap();

        let body = received_body(&server).await;
        assert_eq!(body["tools"][0]["input_schema"], json!({"type": "object"}));
        assert_eq!(body["tool_choice"], json!({"type": "any"}));
    }

    #[tokio::test]
    async fn structured_output_is_rejected_fail_closed() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"content": []})))
            .mount(&server)
            .await;
        // Anthropic 无原生结构化输出,不以 prompt 冒充:显式请求即 fail-closed 拒绝。
        for format in [
            ResponseFormat::JsonObject,
            ResponseFormat::JsonSchema {
                name: "out".into(),
                schema: json!({"type": "object"}),
                strict: false,
            },
        ] {
            let mut req = user_request();
            req.generation.stream = false;
            req.response_format = format;
            let err = model_for(&server).stream(req).await.err().unwrap();
            assert!(matches!(err, KairosError::Validation { .. }));
        }
    }

    #[tokio::test]
    async fn explicit_thinking_is_rejected_fail_closed() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"content": []})))
            .mount(&server)
            .await;
        // 显式开启 thinking → 拒绝(尚未实现)。
        let mut req = user_request();
        req.generation.stream = false;
        req.generation.thinking.mode = ThinkingMode::Enabled;
        assert!(matches!(
            model_for(&server).stream(req).await.err().unwrap(),
            KairosError::Validation { .. }
        ));
        // 显式 reasoning_effort → 拒绝。
        let mut req = user_request();
        req.generation.stream = false;
        req.generation.thinking.reasoning_effort = Some(ReasoningEffort::High);
        assert!(matches!(
            model_for(&server).stream(req).await.err().unwrap(),
            KairosError::Validation { .. }
        ));
        // Auto(默认)/Disabled(不带 effort)不触发拒绝,正常发起。
        let mut req = user_request();
        req.generation.stream = false;
        req.generation.thinking.mode = ThinkingMode::Disabled;
        let _ = model_for(&server).stream(req).await.unwrap();
    }

    #[tokio::test]
    async fn tool_choice_none_sends_none_when_tools_present() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"content": []})))
            .mount(&server)
            .await;
        let mut req = user_request();
        req.generation.stream = false;
        req.tools.push(ToolDefinition {
            name: "weather".into(),
            description: "天气".into(),
            parameters: json!({"type": "object"}),
        });
        req.tool_choice = ToolChoice::None;
        let _ = model_for(&server).stream(req).await.unwrap();

        // None 显式禁用工具(不被悄悄降级为 auto);tools 非空时发 {"type":"none"}。
        assert_eq!(
            received_body(&server).await["tool_choice"],
            json!({"type": "none"})
        );
    }

    #[tokio::test]
    async fn server_error_is_retryable() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(529))
            .mount(&server)
            .await;
        let mut req = user_request();
        req.generation.stream = false;
        let err = model_for(&server).stream(req).await.err().unwrap();
        assert!(err.is_retryable());
    }
}
