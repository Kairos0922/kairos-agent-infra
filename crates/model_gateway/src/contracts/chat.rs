//! 规范化对话请求与流式响应。

use std::pin::Pin;

use async_trait::async_trait;
use foundation::KairosError;
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 统一的异步响应流。provider 的原始流式事件不会穿透到上层。
pub type ChatStream = Pin<Box<dyn Stream<Item = Result<ChatChunk, KairosError>> + Send>>;

/// 一个已解析的具体模型部署身份，用于 trace 与 usage 归因。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelIdentity {
    /// 部署别名，稳定地由配置定义。
    pub deployment: String,
    /// provider 配置别名。
    pub provider: String,
    /// 实际发给厂商 API 的模型 ID。
    pub model: String,
}

/// 对所有厂商统一的调用请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub generation: GenerationOptions,
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
    #[serde(default)]
    pub tool_choice: ToolChoice,
    #[serde(default)]
    pub response_format: ResponseFormat,
}

impl ChatRequest {
    /// 校验跨厂商一致的请求不变量。
    pub fn validate(&self) -> Result<(), KairosError> {
        if self.messages.is_empty() {
            return Err(KairosError::validation("messages 不得为空"));
        }
        self.generation.validate()?;
        if self.tools.is_empty() && !matches!(self.tool_choice, ToolChoice::Auto | ToolChoice::None)
        {
            return Err(KairosError::validation("未提供 tools 时不可要求工具调用"));
        }
        if let ToolChoice::Specific { name } = &self.tool_choice {
            if !self.tools.iter().any(|tool| tool.name == *name) {
                return Err(KairosError::validation("指定的工具不存在").with_detail("tool", name));
            }
        }
        Ok(())
    }
}

/// 通用生成参数。没有 `extra_body` 一类绕过规范化的逃生口。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GenerationOptions {
    pub stream: bool,
    pub thinking: ThinkingOptions,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub max_output_tokens: Option<u32>,
    pub stop_sequences: Vec<String>,
}

impl Default for GenerationOptions {
    fn default() -> Self {
        Self {
            stream: true,
            thinking: ThinkingOptions::default(),
            temperature: None,
            top_p: None,
            max_output_tokens: None,
            stop_sequences: Vec::new(),
        }
    }
}

impl GenerationOptions {
    fn validate(&self) -> Result<(), KairosError> {
        if self.temperature.is_some() && self.top_p.is_some() {
            return Err(KairosError::validation("temperature 与 top_p 不可同时设置"));
        }
        // 采样范围取最宽厂商上界 2.0(OpenAI/DeepSeek 允许 temperature≤2);更窄的厂商约束
        // (如 Anthropic temperature≤1)由 provider API 自身拒绝(fail-loud),不在此逐厂商枚举。
        for (name, value) in [("temperature", self.temperature), ("top_p", self.top_p)] {
            if let Some(value) = value {
                if !value.is_finite() || !(0.0..=2.0).contains(&value) {
                    return Err(KairosError::validation("采样参数必须在 0 到 2 之间")
                        .with_detail("field", name));
                }
            }
        }
        if self.max_output_tokens == Some(0) {
            return Err(KairosError::validation("max_output_tokens 必须大于 0"));
        }
        if matches!(self.thinking.mode, ThinkingMode::Disabled)
            && self.thinking.reasoning_effort.is_some()
        {
            return Err(KairosError::validation(
                "thinking 为 disabled 时不可设置 reasoning_effort",
            ));
        }
        Ok(())
    }
}

/// 思考控制的统一语义。provider 不支持时必须由 router 在调用前拒绝。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThinkingOptions {
    pub mode: ThinkingMode,
    pub reasoning_effort: Option<ReasoningEffort>,
}

impl Default for ThinkingOptions {
    fn default() -> Self {
        Self {
            mode: ThinkingMode::Auto,
            reasoning_effort: None,
        }
    }
}

/// 思考开关。`Auto`=由 provider 决定(默认);`Enabled`=显式开启(需能力支持);`Disabled`=显式关闭。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingMode {
    Auto,
    Enabled,
    Disabled,
}

/// 推理强度档位(由低到高)。provider 实际支持的档位由能力档案 `reasoning_efforts` 声明,
/// 请求未声明者 router 调用前拒绝。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}

/// 标准化消息。provider_resume_state 仅供同一 provider 的后续工具轮次使用。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum ChatMessage {
    System {
        content: String,
    },
    Developer {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: Option<String>,
        #[serde(default)]
        tool_calls: Vec<ToolCall>,
        // 思考续接状态为 gateway 私有,绝不序列化进客户端事件/日志(见 ProviderResumeState)。
        #[serde(skip)]
        provider_resume_state: Option<ProviderResumeState>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}

/// provider 必须透传、但上层不解释的续接状态,如思考工具调用所需的原始状态(reasoning_content)。
///
/// **安全边界**:携带思考内容,故刻意**不实现 `Serialize`/`Deserialize`**,且在所属 DTO
/// (ChatMessage/ChatChunk)中以 `#[serde(skip)]` 排除——杜绝经客户端事件、日志、错误 details
/// 外泄。仅供同一 provider 工具轮次的内存透传续接。彻底的 gateway 私有 token 隔离(思考内容
/// 不出 gateway)为后续设计。
#[derive(Debug, Clone)]
pub struct ProviderResumeState {
    pub provider: String,
    pub payload: Value,
}

/// 上层声明给模型的工具定义。`parameters` 为 JSON Schema,描述工具入参结构。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// 模型发起的一次工具调用,`arguments` 为按工具 schema 解析后的入参。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// 工具调用策略。`None`=明确禁止调用;`Auto`=模型自决(默认);`Required`=必须调用;
/// `Specific`=必须调用指定工具。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    None,
    #[default]
    Auto,
    Required,
    Specific {
        name: String,
    },
}

/// 结构化输出格式。`Text`=普通文本(默认);`JsonObject`/`JsonSchema` 需模型能力档案声明
/// 相应支持,否则 router fail-closed 拒绝(不以 prompt 冒充)。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseFormat {
    #[default]
    Text,
    JsonObject,
    JsonSchema {
        name: String,
        schema: Value,
        #[serde(default)]
        strict: bool,
    },
}

/// provider 适配完成后的规范化事件。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatChunk {
    TextDelta {
        text: String,
    },
    ToolCallDelta {
        id: String,
        name: Option<String>,
        arguments_delta: String,
    },
    ToolCallComplete {
        call: ToolCall,
        // 思考续接状态为 gateway 私有,绝不序列化(见 ProviderResumeState)。
        #[serde(skip)]
        provider_resume_state: Option<ProviderResumeState>,
    },
    Usage {
        usage: TokenUsage,
    },
    Stop {
        reason: StopReason,
    },
}

/// 一次调用的 token 用量归因。`reasoning_tokens` 为思考消耗;`model` 标注实际服务的部署
/// (fallback 降级时据此归因)。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelIdentity>,
}

/// 模型停止生成的原因。`EndTurn`=正常结束;`ToolUse`=需工具调用;`MaxTokens`=达输出上限;
/// `ContentFilter`=内容过滤;`Cancelled`=取消。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    ContentFilter,
    Cancelled,
}

/// 单个具体模型的调用能力。router 才处理 tier 与 fallback。
#[async_trait]
pub trait ChatModel: Send + Sync {
    /// 返回规范化事件流。调用建立阶段的错误直接返回，流中错误表示中途故障。
    async fn stream(&self, request: ChatRequest) -> Result<ChatStream, KairosError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ChatRequest {
        ChatRequest {
            messages: vec![ChatMessage::User {
                content: "hello".to_string(),
            }],
            generation: GenerationOptions::default(),
            tools: Vec::new(),
            tool_choice: ToolChoice::Auto,
            response_format: ResponseFormat::Text,
        }
    }

    #[test]
    fn rejects_conflicting_sampling_parameters() {
        let mut req = request();
        req.generation.temperature = Some(0.2);
        req.generation.top_p = Some(0.9);
        assert!(req.validate().is_err());
    }

    #[test]
    fn rejects_effort_when_thinking_disabled() {
        let mut req = request();
        req.generation.thinking.mode = ThinkingMode::Disabled;
        req.generation.thinking.reasoning_effort = Some(ReasoningEffort::High);
        assert!(req.validate().is_err());
    }

    #[test]
    fn rejects_unknown_specific_tool() {
        let mut req = request();
        req.tools.push(ToolDefinition {
            name: "weather".to_string(),
            description: "天气".to_string(),
            parameters: Value::Null,
        });
        req.tool_choice = ToolChoice::Specific {
            name: "missing".to_string(),
        };
        assert!(req.validate().is_err());
    }

    #[test]
    fn accepts_normalized_request() {
        assert!(request().validate().is_ok());
    }
}
