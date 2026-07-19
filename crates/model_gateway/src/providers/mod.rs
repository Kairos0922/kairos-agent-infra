//! 私有 provider 实现。上层 crate 不可命名本模块。

mod anthropic;
mod openai_compat;

pub(crate) use anthropic::AnthropicChatModel;
pub(crate) use openai_compat::OpenAiCompatChatModel;
