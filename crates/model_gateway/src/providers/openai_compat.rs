//! OpenAI Chat Completions 协议及其 GLM、DeepSeek 方言适配。

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use async_trait::async_trait;
use eventsource_stream::Eventsource;
use foundation::KairosError;
use futures_util::{stream, StreamExt};
use reqwest::{Client, StatusCode, Url};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::ProviderDialect;
use crate::contracts::{
    ChatChunk, ChatMessage, ChatModel, ChatRequest, ChatStream, ModelIdentity, ProviderResumeState,
    ReasoningEffort, ResponseFormat, StopReason, ThinkingMode, TokenUsage, ToolCall, ToolChoice,
};

/// OpenAI Chat Completions 协议适配(含 GPT/GLM/DeepSeek 方言)。
///
/// 安全约定:持有 `api_key` 明文,故**刻意不 `derive(Debug)`**——避免将来误打印/panic 泄露密钥。
pub(crate) struct OpenAiCompatChatModel {
    client: Client,
    identity: ModelIdentity,
    dialect: ProviderDialect,
    base_url: Url,
    api_key: String,
}

impl OpenAiCompatChatModel {
    pub(crate) fn new(
        client: Client,
        identity: ModelIdentity,
        dialect: ProviderDialect,
        base_url: Url,
        api_key: String,
    ) -> Self {
        Self {
            client,
            identity,
            dialect,
            base_url,
            api_key,
        }
    }

    fn endpoint(&self) -> Result<Url, KairosError> {
        self.base_url.join("chat/completions").map_err(|error| {
            provider_error(
                &self.identity.provider,
                "无效的 OpenAI 兼容端点",
                false,
                error,
            )
        })
    }

    fn request_body(&self, request: &ChatRequest) -> Value {
        let mut body = json!({
            "model": self.identity.model,
            "messages": openai_messages(&request.messages, self.dialect, &self.identity.provider),
            "stream": request.generation.stream,
        });
        let object = body.as_object_mut().expect("json object");
        if request.generation.stream {
            object.insert("stream_options".to_string(), json!({"include_usage": true}));
        }
        if let Some(value) = request.generation.temperature {
            object.insert("temperature".to_string(), json!(value));
        }
        if let Some(value) = request.generation.top_p {
            object.insert("top_p".to_string(), json!(value));
        }
        if let Some(value) = request.generation.max_output_tokens {
            object.insert("max_tokens".to_string(), json!(value));
        }
        if !request.generation.stop_sequences.is_empty() {
            object.insert("stop".to_string(), json!(request.generation.stop_sequences));
        }
        insert_thinking(object, request, self.dialect);
        insert_tools(object, request);
        insert_response_format(object, &request.response_format);
        if matches!(self.dialect, ProviderDialect::Zhipu)
            && request.generation.stream
            && !request.tools.is_empty()
        {
            object.insert("tool_stream".to_string(), Value::Bool(true));
        }
        body
    }
}

#[async_trait]
impl ChatModel for OpenAiCompatChatModel {
    async fn stream(&self, request: ChatRequest) -> Result<ChatStream, KairosError> {
        let response = self
            .client
            .post(self.endpoint()?)
            .bearer_auth(&self.api_key)
            .json(&self.request_body(&request))
            .send()
            .await
            .map_err(|error| request_error(&self.identity.provider, error))?;
        ensure_success(&self.identity.provider, response.status())?;

        if !request.generation.stream {
            let body: OpenAiCompletion = response
                .json()
                .await
                .map_err(|error| request_error(&self.identity.provider, error))?;
            return Ok(Box::pin(stream::iter(non_stream_chunks(
                body,
                self.identity.clone(),
                &self.identity.provider,
                self.dialect,
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
                BTreeMap::<usize, String>::new(),
                VecDeque::<ChatChunk>::new(),
            ),
            move |(mut events, mut call_ids, mut pending)| {
                let provider = Arc::clone(&provider);
                let identity = Arc::clone(&identity);
                async move {
                    loop {
                        if let Some(chunk) = pending.pop_front() {
                            return Ok(Some((chunk, (events, call_ids, pending))));
                        }
                        let Some(event) = events.next().await else {
                            return Ok(None);
                        };
                        let event = event.map_err(|error| {
                            KairosError::provider(&*provider, "SSE 解析失败", true)
                                .with_detail("reason", error.to_string())
                        })?;
                        if event.data == "[DONE]" {
                            return Ok(None);
                        }
                        let parsed: OpenAiChunk =
                            serde_json::from_str(&event.data).map_err(|error| {
                                KairosError::provider(&*provider, "模型流式响应格式非法", false)
                                    .with_detail("reason", error.to_string())
                            })?;
                        pending.extend(stream_chunks(parsed, &identity, &mut call_ids)?);
                    }
                }
            },
        );
        Ok(Box::pin(output))
    }
}

fn openai_messages(
    messages: &[ChatMessage],
    dialect: ProviderDialect,
    provider: &str,
) -> Vec<Value> {
    messages
        .iter()
        .map(|message| match message {
            ChatMessage::System { content } => json!({"role": "system", "content": content}),
            ChatMessage::Developer { content } if matches!(dialect, ProviderDialect::Openai) => {
                json!({"role": "developer", "content": content})
            }
            ChatMessage::Developer { content } => json!({"role": "system", "content": content}),
            ChatMessage::User { content } => json!({"role": "user", "content": content}),
            ChatMessage::Assistant {
                content,
                tool_calls,
                provider_resume_state,
            } => {
                let mut value = json!({"role": "assistant", "content": content});
                let object = value.as_object_mut().expect("json object");
                if !tool_calls.is_empty() {
                    object.insert(
                        "tool_calls".to_string(),
                        Value::Array(
                            tool_calls
                                .iter()
                                .map(|call| {
                                    json!({
                                        "id": call.id,
                                        "type": "function",
                                        "function": {
                                            "name": call.name,
                                            "arguments": call.arguments.to_string(),
                                        }
                                    })
                                })
                                .collect(),
                        ),
                    );
                }
                if matches!(dialect, ProviderDialect::Deepseek) {
                    if let Some(ProviderResumeState {
                        provider: state_provider,
                        payload,
                    }) = provider_resume_state
                    {
                        if state_provider == provider {
                            if let Some(reasoning) = payload.get("reasoning_content") {
                                object.insert("reasoning_content".to_string(), reasoning.clone());
                            }
                        }
                    }
                }
                value
            }
            ChatMessage::Tool {
                tool_call_id,
                content,
            } => json!({"role": "tool", "tool_call_id": tool_call_id, "content": content}),
        })
        .collect()
}

fn insert_thinking(
    object: &mut serde_json::Map<String, Value>,
    request: &ChatRequest,
    dialect: ProviderDialect,
) {
    let thinking = &request.generation.thinking;
    match dialect {
        ProviderDialect::Zhipu | ProviderDialect::Deepseek => {
            if !matches!(thinking.mode, ThinkingMode::Auto) {
                let state = match thinking.mode {
                    ThinkingMode::Enabled => "enabled",
                    ThinkingMode::Disabled => "disabled",
                    ThinkingMode::Auto => unreachable!(),
                };
                object.insert("thinking".to_string(), json!({"type": state}));
            }
            if let Some(effort) = thinking.reasoning_effort {
                // DeepSeek 仅支持 high/max 两档 reasoning_effort:低档(Minimal/Low/Medium)向上
                // 归并为 high、Xhigh 映射为 max。规范做法是能力档案 reasoning_efforts 只声明
                // [High, Xhigh] 让 router 先行拒绝低档;此处归并为兜底,操作方应据实声明能力。
                object.insert(
                    "reasoning_effort".to_string(),
                    Value::String(
                        match (dialect, effort) {
                            (
                                ProviderDialect::Deepseek,
                                ReasoningEffort::Minimal
                                | ReasoningEffort::Low
                                | ReasoningEffort::Medium,
                            ) => "high",
                            (ProviderDialect::Deepseek, ReasoningEffort::Xhigh) => "max",
                            (_, ReasoningEffort::Minimal) => "minimal",
                            (_, ReasoningEffort::Low) => "low",
                            (_, ReasoningEffort::Medium) => "medium",
                            (_, ReasoningEffort::High) => "high",
                            (_, ReasoningEffort::Xhigh) => "xhigh",
                        }
                        .to_string(),
                    ),
                );
            }
        }
        ProviderDialect::Openai => {
            let effort = thinking.reasoning_effort.or(match thinking.mode {
                ThinkingMode::Enabled => Some(ReasoningEffort::Medium),
                ThinkingMode::Disabled => Some(ReasoningEffort::Minimal),
                ThinkingMode::Auto => None,
            });
            if let Some(effort) = effort {
                object.insert(
                    "reasoning_effort".to_string(),
                    Value::String(format_effort(effort).to_string()),
                );
            }
        }
        ProviderDialect::Anthropic => {}
    }
}

fn format_effort(effort: ReasoningEffort) -> &'static str {
    match effort {
        ReasoningEffort::Minimal => "minimal",
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
        ReasoningEffort::Xhigh => "xhigh",
    }
}

fn insert_tools(object: &mut serde_json::Map<String, Value>, request: &ChatRequest) {
    if !request.tools.is_empty() {
        object.insert(
            "tools".to_string(),
            Value::Array(
                request
                    .tools
                    .iter()
                    .map(|tool| {
                        json!({
                            "type": "function",
                            "function": {
                                "name": tool.name,
                                "description": tool.description,
                                "parameters": tool.parameters,
                            }
                        })
                    })
                    .collect(),
            ),
        );
    }
    let choice = match &request.tool_choice {
        ToolChoice::None => Value::String("none".to_string()),
        ToolChoice::Auto => Value::String("auto".to_string()),
        ToolChoice::Required => Value::String("required".to_string()),
        ToolChoice::Specific { name } => json!({"type": "function", "function": {"name": name}}),
    };
    object.insert("tool_choice".to_string(), choice);
}

fn insert_response_format(object: &mut serde_json::Map<String, Value>, format: &ResponseFormat) {
    let value = match format {
        ResponseFormat::Text => return,
        ResponseFormat::JsonObject => json!({"type": "json_object"}),
        ResponseFormat::JsonSchema {
            name,
            schema,
            strict,
        } => json!({
            "type": "json_schema",
            "json_schema": {"name": name, "schema": schema, "strict": strict},
        }),
    };
    object.insert("response_format".to_string(), value);
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
    let retryable = error.is_timeout() || error.is_connect() || error.is_request();
    provider_error(provider, "模型 HTTP 调用失败", retryable, error)
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
struct OpenAiChunk {
    #[serde(default)]
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    #[serde(default)]
    delta: OpenAiDelta,
    finish_reason: Option<String>,
}

/// 流式增量。注意:DeepSeek 的思考内容 `reasoning_content` 仅在**非流式**路径捕获并写入
/// `ProviderResumeState` 供工具续接;流式路径暂不解析该字段——即「流式 + 思考 + 工具续接」
/// 组合不会 round-trip 思考状态,需该组合时请走非流式。流式思考续接为后续任务。
#[derive(Debug, Default, Deserialize)]
struct OpenAiDelta {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OpenAiToolCallDelta>,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCallDelta {
    #[serde(default)]
    index: usize,
    id: Option<String>,
    function: Option<OpenAiFunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct OpenAiFunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    #[serde(default)]
    prompt_tokens_details: OpenAiPromptTokenDetails,
    #[serde(default)]
    completion_tokens_details: OpenAiCompletionTokenDetails,
}

#[derive(Debug, Default, Deserialize)]
struct OpenAiPromptTokenDetails {
    cached_tokens: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenAiCompletionTokenDetails {
    reasoning_tokens: Option<u64>,
}

fn stream_chunks(
    response: OpenAiChunk,
    identity: &ModelIdentity,
    call_ids: &mut BTreeMap<usize, String>,
) -> Result<Vec<ChatChunk>, KairosError> {
    let mut out = Vec::new();
    for choice in response.choices {
        if let Some(text) = choice.delta.content {
            out.push(ChatChunk::TextDelta { text });
        }
        for tool in choice.delta.tool_calls {
            // 记录该 index 的调用 id:首个 delta 可能缺 id(GLM/DeepSeek 等方言不保证首帧带 id),
            // 先用占位 id;一旦后续 delta 带来真实 id 即覆盖,避免真实 id 被占位 id 永久顶替。
            if let Some(real_id) = &tool.id {
                call_ids.insert(tool.index, real_id.clone());
            }
            let id = call_ids
                .entry(tool.index)
                .or_insert_with(|| format!("tool_call_{}", tool.index))
                .clone();
            let function = tool.function.unwrap_or(OpenAiFunctionDelta {
                name: None,
                arguments: None,
            });
            out.push(ChatChunk::ToolCallDelta {
                id,
                name: function.name,
                arguments_delta: function.arguments.unwrap_or_default(),
            });
        }
        if let Some(reason) = choice.finish_reason {
            out.push(ChatChunk::Stop {
                reason: parse_stop_reason(&reason),
            });
        }
    }
    if let Some(usage) = response.usage {
        out.push(ChatChunk::Usage {
            usage: to_usage(usage, identity.clone()),
        });
    }
    Ok(out)
}

#[derive(Debug, Deserialize)]
struct OpenAiCompletion {
    #[serde(default)]
    choices: Vec<OpenAiCompletionChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiCompletionChoice {
    message: OpenAiMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OpenAiToolCall>,
    reasoning_content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCall {
    id: String,
    function: OpenAiFunction,
}

#[derive(Debug, Deserialize)]
struct OpenAiFunction {
    name: String,
    arguments: String,
}

fn non_stream_chunks(
    response: OpenAiCompletion,
    identity: ModelIdentity,
    provider: &str,
    dialect: ProviderDialect,
) -> Vec<Result<ChatChunk, KairosError>> {
    let mut out = Vec::new();
    if let Some(choice) = response.choices.into_iter().next() {
        if let Some(content) = choice.message.content {
            out.push(Ok(ChatChunk::TextDelta { text: content }));
        }
        let resume = if matches!(dialect, ProviderDialect::Deepseek)
            && choice.message.reasoning_content.is_some()
            && !choice.message.tool_calls.is_empty()
        {
            Some(ProviderResumeState {
                provider: provider.to_string(),
                payload: json!({"reasoning_content": choice.message.reasoning_content}),
            })
        } else {
            None
        };
        for call in choice.message.tool_calls {
            match serde_json::from_str(&call.function.arguments) {
                Ok(arguments) => out.push(Ok(ChatChunk::ToolCallComplete {
                    call: ToolCall {
                        id: call.id,
                        name: call.function.name,
                        arguments,
                    },
                    provider_resume_state: resume.clone(),
                })),
                Err(error) => out.push(Err(KairosError::provider(
                    provider,
                    "工具参数不是有效 JSON",
                    false,
                )
                .with_detail("reason", error.to_string()))),
            }
        }
        if let Some(reason) = choice.finish_reason {
            out.push(Ok(ChatChunk::Stop {
                reason: parse_stop_reason(&reason),
            }));
        }
    }
    if let Some(usage) = response.usage {
        out.push(Ok(ChatChunk::Usage {
            usage: to_usage(usage, identity),
        }));
    }
    out
}

fn to_usage(usage: OpenAiUsage, identity: ModelIdentity) -> TokenUsage {
    TokenUsage {
        input_tokens: usage.prompt_tokens,
        output_tokens: usage.completion_tokens,
        reasoning_tokens: usage.completion_tokens_details.reasoning_tokens,
        cached_input_tokens: usage.prompt_tokens_details.cached_tokens,
        model: Some(identity),
    }
}

fn parse_stop_reason(reason: &str) -> StopReason {
    match reason {
        "tool_calls" | "function_call" => StopReason::ToolUse,
        "length" => StopReason::MaxTokens,
        "content_filter" => StopReason::ContentFilter,
        _ => StopReason::EndTurn,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProviderDialect;
    use crate::contracts::{
        ChatChunk, ChatMessage, ChatRequest, GenerationOptions, ProviderResumeState,
        ReasoningEffort, ResponseFormat, StopReason, ThinkingMode, ToolChoice, ToolDefinition,
    };
    use futures_util::StreamExt;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn identity() -> ModelIdentity {
        ModelIdentity {
            deployment: "dep".into(),
            provider: "prov".into(),
            model: "test-model".into(),
        }
    }

    fn model_for(dialect: ProviderDialect, server: &MockServer) -> OpenAiCompatChatModel {
        OpenAiCompatChatModel::new(
            Client::new(),
            identity(),
            dialect,
            Url::parse(&format!("{}/", server.uri())).unwrap(),
            "test-key".into(),
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

    /// 把若干 JSON 事件串成 SSE 报文(以 `[DONE]` 收尾)。
    fn sse(events: &[&str]) -> String {
        let mut out = String::new();
        for event in events {
            out.push_str("data: ");
            out.push_str(event);
            out.push_str("\n\n");
        }
        out.push_str("data: [DONE]\n\n");
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

    async fn mount_json(server: &MockServer, body: Value) {
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(server)
            .await;
    }

    async fn mount_sse(server: &MockServer, body: String) {
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body),
            )
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn sends_bearer_auth_and_model() {
        let server = MockServer::start().await;
        mount_json(&server, json!({"choices": []})).await;
        let mut req = user_request();
        req.generation.stream = false;
        let _ = model_for(ProviderDialect::Openai, &server)
            .stream(req)
            .await
            .unwrap();

        let requests = server.received_requests().await.unwrap();
        let auth = requests[0]
            .headers
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(auth, "Bearer test-key");
        assert_eq!(received_body(&server).await["model"], json!("test-model"));
    }

    #[tokio::test]
    async fn non_stream_parses_text_stop_usage() {
        let server = MockServer::start().await;
        mount_json(
            &server,
            json!({
                "choices": [{"message": {"content": "你好！"}, "finish_reason": "stop"}],
                "usage": {
                    "prompt_tokens": 5, "completion_tokens": 3,
                    "completion_tokens_details": {"reasoning_tokens": 1},
                    "prompt_tokens_details": {"cached_tokens": 2}
                }
            }),
        )
        .await;
        let mut req = user_request();
        req.generation.stream = false;
        let chunks = collect(
            model_for(ProviderDialect::Openai, &server)
                .stream(req)
                .await
                .unwrap(),
        )
        .await;

        assert!(matches!(&chunks[0], ChatChunk::TextDelta { text } if text == "你好！"));
        assert!(matches!(
            &chunks[1],
            ChatChunk::Stop {
                reason: StopReason::EndTurn
            }
        ));
        match &chunks[2] {
            ChatChunk::Usage { usage } => {
                assert_eq!(usage.input_tokens, Some(5));
                assert_eq!(usage.output_tokens, Some(3));
                assert_eq!(usage.reasoning_tokens, Some(1));
                assert_eq!(usage.cached_input_tokens, Some(2));
            }
            other => panic!("应为 Usage,实际 {other:?}"),
        }
    }

    #[tokio::test]
    async fn stream_parses_text_deltas_usage_stop() {
        let server = MockServer::start().await;
        mount_sse(
            &server,
            sse(&[
                r#"{"choices":[{"delta":{"content":"Hello"}}]}"#,
                r#"{"choices":[{"delta":{"content":" world"}}]}"#,
                r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
                r#"{"choices":[],"usage":{"prompt_tokens":3,"completion_tokens":2}}"#,
                "[DONE]",
            ]),
        )
        .await;
        let chunks = collect(
            model_for(ProviderDialect::Openai, &server)
                .stream(user_request())
                .await
                .unwrap(),
        )
        .await;

        let text: String = chunks
            .iter()
            .filter_map(|c| match c {
                ChatChunk::TextDelta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "Hello world");
        assert!(chunks.iter().any(|c| matches!(
            c,
            ChatChunk::Stop {
                reason: StopReason::EndTurn
            }
        )));
        assert!(chunks
            .iter()
            .any(|c| matches!(c, ChatChunk::Usage { usage } if usage.output_tokens == Some(2))));
    }

    #[tokio::test]
    async fn stream_assembles_tool_call_deltas() {
        let server = MockServer::start().await;
        mount_sse(
            &server,
            sse(&[
                r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"weather","arguments":""}}]}}]}"#,
                r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"city\":"}}]}}]}"#,
                r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"北京\"}"}}]}}]}"#,
                r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
                "[DONE]",
            ]),
        )
        .await;
        let chunks = collect(
            model_for(ProviderDialect::Openai, &server)
                .stream(user_request())
                .await
                .unwrap(),
        )
        .await;

        let args: String = chunks
            .iter()
            .filter_map(|c| match c {
                ChatChunk::ToolCallDelta {
                    id,
                    arguments_delta,
                    ..
                } => {
                    assert_eq!(id, "call_1");
                    Some(arguments_delta.as_str())
                }
                _ => None,
            })
            .collect();
        assert_eq!(args, r#"{"city":"北京"}"#);
        assert!(chunks.iter().any(|c| matches!(
            c,
            ChatChunk::Stop {
                reason: StopReason::ToolUse
            }
        )));
    }

    #[tokio::test]
    async fn non_stream_parses_tool_call_complete() {
        let server = MockServer::start().await;
        mount_json(
            &server,
            json!({
                "choices": [{
                    "message": {"tool_calls": [{"id": "call_1", "function": {"name": "weather", "arguments": "{\"city\":\"北京\"}"}}]},
                    "finish_reason": "tool_calls"
                }]
            }),
        )
        .await;
        let mut req = user_request();
        req.generation.stream = false;
        let chunks = collect(
            model_for(ProviderDialect::Openai, &server)
                .stream(req)
                .await
                .unwrap(),
        )
        .await;

        match &chunks[0] {
            ChatChunk::ToolCallComplete { call, .. } => {
                assert_eq!(call.id, "call_1");
                assert_eq!(call.name, "weather");
                assert_eq!(call.arguments, json!({"city": "北京"}));
            }
            other => panic!("应为 ToolCallComplete,实际 {other:?}"),
        }
    }

    #[tokio::test]
    async fn openai_dialect_maps_developer_role() {
        let server = MockServer::start().await;
        mount_json(&server, json!({"choices": []})).await;
        let mut req = user_request();
        req.generation.stream = false;
        req.messages.insert(
            0,
            ChatMessage::Developer {
                content: "系统约定".into(),
            },
        );
        let _ = model_for(ProviderDialect::Openai, &server)
            .stream(req)
            .await
            .unwrap();

        let messages = &received_body(&server).await["messages"];
        assert_eq!(messages[0]["role"], json!("developer"));
    }

    #[tokio::test]
    async fn zhipu_dialect_downgrades_developer_to_system() {
        let server = MockServer::start().await;
        mount_json(&server, json!({"choices": []})).await;
        let mut req = user_request();
        req.generation.stream = false;
        req.messages.insert(
            0,
            ChatMessage::Developer {
                content: "系统约定".into(),
            },
        );
        let _ = model_for(ProviderDialect::Zhipu, &server)
            .stream(req)
            .await
            .unwrap();

        assert_eq!(
            received_body(&server).await["messages"][0]["role"],
            json!("system")
        );
    }

    #[tokio::test]
    async fn zhipu_sends_thinking_and_tool_stream_when_streaming() {
        let server = MockServer::start().await;
        mount_sse(&server, sse(&["[DONE]"])).await;
        let mut req = user_request(); // stream 默认 true
        req.generation.thinking.mode = ThinkingMode::Enabled;
        req.tools.push(ToolDefinition {
            name: "weather".into(),
            description: "天气".into(),
            parameters: json!({}),
        });
        let _ = model_for(ProviderDialect::Zhipu, &server)
            .stream(req)
            .await
            .unwrap();

        let body = received_body(&server).await;
        assert_eq!(body["thinking"], json!({"type": "enabled"}));
        assert_eq!(body["tool_stream"], json!(true));
        assert_eq!(body["tool_choice"], json!("auto"));
    }

    #[tokio::test]
    async fn deepseek_maps_reasoning_effort() {
        for (effort, expected) in [
            (ReasoningEffort::Low, "high"),
            (ReasoningEffort::High, "high"),
            (ReasoningEffort::Xhigh, "max"),
        ] {
            let server = MockServer::start().await;
            mount_json(&server, json!({"choices": []})).await;
            let mut req = user_request();
            req.generation.stream = false;
            req.generation.thinking.reasoning_effort = Some(effort);
            let _ = model_for(ProviderDialect::Deepseek, &server)
                .stream(req)
                .await
                .unwrap();
            assert_eq!(
                received_body(&server).await["reasoning_effort"],
                json!(expected),
                "effort {effort:?} 应映射为 {expected}"
            );
        }
    }

    #[tokio::test]
    async fn deepseek_resume_state_round_trips_reasoning_content() {
        let server = MockServer::start().await;
        mount_json(&server, json!({"choices": []})).await;
        let mut req = user_request();
        req.generation.stream = false;
        req.messages.push(ChatMessage::Assistant {
            content: Some("思考后回答".into()),
            tool_calls: Vec::new(),
            provider_resume_state: Some(ProviderResumeState {
                provider: "prov".into(),
                payload: json!({"reasoning_content": "内部推理"}),
            }),
        });
        let _ = model_for(ProviderDialect::Deepseek, &server)
            .stream(req)
            .await
            .unwrap();

        let messages = &received_body(&server).await["messages"];
        let assistant = &messages[1];
        assert_eq!(assistant["reasoning_content"], json!("内部推理"));
    }

    #[tokio::test]
    async fn json_schema_response_format_is_sent() {
        let server = MockServer::start().await;
        mount_json(&server, json!({"choices": []})).await;
        let mut req = user_request();
        req.generation.stream = false;
        req.response_format = ResponseFormat::JsonSchema {
            name: "out".into(),
            schema: json!({"type": "object"}),
            strict: true,
        };
        let _ = model_for(ProviderDialect::Openai, &server)
            .stream(req)
            .await
            .unwrap();

        assert_eq!(
            received_body(&server).await["response_format"],
            json!({"type": "json_schema", "json_schema": {"name": "out", "schema": {"type": "object"}, "strict": true}})
        );
    }

    #[tokio::test]
    async fn server_error_is_retryable() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let mut req = user_request();
        req.generation.stream = false;
        let err = model_for(ProviderDialect::Openai, &server)
            .stream(req)
            .await
            .err()
            .unwrap();
        assert!(err.is_retryable());
    }

    #[tokio::test]
    async fn bad_request_is_not_retryable() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(400))
            .mount(&server)
            .await;
        let mut req = user_request();
        req.generation.stream = false;
        let err = model_for(ProviderDialect::Openai, &server)
            .stream(req)
            .await
            .err()
            .unwrap();
        assert!(!err.is_retryable());
    }

    #[tokio::test]
    async fn zhipu_stream_parses_text() {
        let server = MockServer::start().await;
        mount_sse(
            &server,
            sse(&[
                r#"{"choices":[{"delta":{"content":"智谱"}}]}"#,
                r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
                "[DONE]",
            ]),
        )
        .await;
        let chunks = collect(
            model_for(ProviderDialect::Zhipu, &server)
                .stream(user_request())
                .await
                .unwrap(),
        )
        .await;
        let text: String = chunks
            .iter()
            .filter_map(|c| match c {
                ChatChunk::TextDelta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "智谱");
        assert!(chunks.iter().any(|c| matches!(
            c,
            ChatChunk::Stop {
                reason: StopReason::EndTurn
            }
        )));
    }

    #[tokio::test]
    async fn deepseek_stream_parses_text() {
        let server = MockServer::start().await;
        mount_sse(
            &server,
            sse(&[
                r#"{"choices":[{"delta":{"content":"深度"}}]}"#,
                r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
                "[DONE]",
            ]),
        )
        .await;
        let chunks = collect(
            model_for(ProviderDialect::Deepseek, &server)
                .stream(user_request())
                .await
                .unwrap(),
        )
        .await;
        let text: String = chunks
            .iter()
            .filter_map(|c| match c {
                ChatChunk::TextDelta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "深度");
    }

    #[tokio::test]
    async fn zhipu_server_error_is_retryable() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let mut req = user_request();
        req.generation.stream = false;
        let err = model_for(ProviderDialect::Zhipu, &server)
            .stream(req)
            .await
            .err()
            .unwrap();
        assert!(err.is_retryable());
    }

    #[tokio::test]
    async fn deepseek_server_error_is_retryable() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let mut req = user_request();
        req.generation.stream = false;
        let err = model_for(ProviderDialect::Deepseek, &server)
            .stream(req)
            .await
            .err()
            .unwrap();
        assert!(err.is_retryable());
    }

    #[tokio::test]
    async fn stream_tool_call_real_id_overwrites_placeholder() {
        let server = MockServer::start().await;
        // 首帧缺 id(GLM/DeepSeek 等方言不保证首帧带 id),后续帧带真实 id:真实 id 应顶替占位 id。
        mount_sse(
            &server,
            sse(&[
                r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"weather","arguments":""}}]}}]}"#,
                r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"real_id","function":{"arguments":"{}"}}]}}]}"#,
                r#"{"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
                "[DONE]",
            ]),
        )
        .await;
        let chunks = collect(
            model_for(ProviderDialect::Deepseek, &server)
                .stream(user_request())
                .await
                .unwrap(),
        )
        .await;
        let last_tool_delta = chunks
            .iter()
            .rev()
            .find(|c| matches!(c, ChatChunk::ToolCallDelta { .. }));
        assert!(
            matches!(last_tool_delta, Some(ChatChunk::ToolCallDelta { id, .. }) if id == "real_id")
        );
    }
}
