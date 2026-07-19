//! 模型网关(model_gateway,L1):统一模型调用契约、厂商协议适配与按档位路由。
//!
//! 上层只传 `ModelTier` 和规范化 [`ChatRequest`]，不接触具体厂商、模型名或 HTTP 协议。
//! `providers` 为私有实现;harness 只能依赖 `contracts` 与公开 factory。

pub mod config;
pub mod contracts;
pub mod factory;

mod providers;
mod router;

pub use config::{ModelGatewayConfig, ModelGatewaySettings};
pub use contracts::{
    ChatChunk, ChatMessage, ChatModel, ChatRequest, ChatStream, GenerationOptions, ModelIdentity,
    ModelRouter, ModelTier, ProviderResumeState, ReasoningEffort, ResponseFormat, StopReason,
    ThinkingMode, TokenUsage, ToolCall, ToolChoice, ToolDefinition,
};
pub use factory::{build_router, build_router_from_config};
