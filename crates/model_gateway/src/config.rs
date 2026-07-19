//! model_gateway 自己的配置与模型能力档案。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::contracts::{ModelTier, ReasoningEffort};

/// 顶层配置包装，承接 `[model_gateway]` TOML 表。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ModelGatewaySettings {
    pub model_gateway: ModelGatewayConfig,
}

/// 模型网关配置。缺 provider、模型或 tier 时由 factory fail-closed。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelGatewayConfig {
    pub max_retries: u8,
    pub providers: BTreeMap<String, ProviderConfig>,
    pub models: BTreeMap<String, ModelConfig>,
    pub tiers: BTreeMap<ModelTier, TierRoute>,
}

impl Default for ModelGatewayConfig {
    fn default() -> Self {
        Self {
            max_retries: 2,
            providers: BTreeMap::new(),
            models: BTreeMap::new(),
            tiers: BTreeMap::new(),
        }
    }
}

/// 厂商协议与方言。GPT、GLM、DeepSeek 共享 OpenAI 主协议但不共享全部语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderDialect {
    Openai,
    Zhipu,
    Deepseek,
    Anthropic,
}

/// 一个 provider 端点的接入配置。
///
/// 安全约定:`api_key_env` 只存**环境变量名**(非密钥本身),密钥于运行时按名读取——
/// 配置值/代码/日志中永不出现密钥明文。故本类型可安全 `derive(Debug)`。
/// (持有真实密钥的 provider 结构体则刻意不 derive Debug,见 providers/。)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub dialect: ProviderDialect,
    pub base_url: String,
    /// 密钥所在环境变量的**名称**(非密钥本身)。
    pub api_key_env: String,
}

/// 一个可路由的模型部署，别名与具体模型 ID 分离以支持无代码切换。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub capabilities: ModelCapabilities,
}

/// 一组 tier 的主模型和有序 fallback。每个候选仍须通过请求能力筛选。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierRoute {
    pub primary: String,
    #[serde(default)]
    pub fallback: Vec<String>,
}

/// 模型实际承诺的能力。未显式声明即视为不支持，避免静默降级。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelCapabilities {
    pub stream: bool,
    pub tools: bool,
    pub tool_choice_required: bool,
    pub tool_choice_specific: bool,
    pub thinking: bool,
    pub reasoning_efforts: Vec<ReasoningEffort>,
    pub temperature: bool,
    pub top_p: bool,
    pub sampling_when_thinking: bool,
    pub json_object: bool,
    pub json_schema: bool,
    pub max_output_tokens: Option<u32>,
}
