//! 对上层公开的模型调用契约。

mod chat;
mod router;

pub use chat::{
    ChatChunk, ChatMessage, ChatModel, ChatRequest, ChatStream, GenerationOptions, ModelIdentity,
    ProviderResumeState, ReasoningEffort, ResponseFormat, StopReason, ThinkingMode, TokenUsage,
    ToolCall, ToolChoice, ToolDefinition,
};
pub use router::{ModelRouter, ModelTier};
