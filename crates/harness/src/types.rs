//! harness 层共享类型:RunInput / RunOutcome / WrapUpReason 等。

use chrono::{DateTime, Utc};
use model_gateway::ModelTier;
use observability::RunStatus;
use serde::{Deserialize, Serialize};

/// 一次 run 的输入参数(server 层构造,传入 LoopEngine::run)。
#[derive(Debug, Clone)]
pub struct RunInput {
    /// 所属 session 标识。
    pub session_id: String,
    /// Profile 引用标识。
    pub profile_ref: String,
    /// 用户本轮消息。
    pub user_message: String,
    /// 人设文本(P1 分区)。
    pub persona: String,
    /// 模型档位(来自 Profile 的 loop_policy.model_tier)。
    pub model_tier: ModelTier,
    /// 预算配置。
    pub budget: BudgetConfig,
    /// 工具白名单(来自 Profile)。
    pub tool_allowlist: Vec<String>,
    /// 分区配额配置。
    pub partition_config: PartitionConfigInput,
}

/// 预算配置(构造 Budget 的输入)。
#[derive(Debug, Clone)]
pub struct BudgetConfig {
    pub max_turns: u32,
    pub max_tokens: u64,
    pub max_cost_micro_usd: u64,
    pub deadline: Option<DateTime<Utc>>,
    /// WRAP_UP 预留比率(默认 0.1 = 10% token)。
    pub wrap_up_reserve: f64,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            max_turns: 20,
            max_tokens: 200_000,
            max_cost_micro_usd: 1_000_000, // $1.00
            deadline: None,
            wrap_up_reserve: 0.1,
        }
    }
}

/// 分区配额输入(构造 PartitionConfig 的参数)。
#[derive(Debug, Clone)]
pub struct PartitionConfigInput {
    /// 上下文窗口总 token 数(由模型能力决定)。
    pub context_window: usize,
    /// 输出预留 token 数(max_output_tokens + reasoning 预算)。
    pub reserved_output: usize,
    /// 各分区配额覆写(默认值见 context::partition)。
    pub quota_overrides: Vec<(String, f64)>,
}

/// 一次 run 的结果。
#[derive(Debug, Clone)]
pub struct RunOutcome {
    /// run 标识。
    pub run_id: String,
    /// 终态状态。
    pub status: RunStatus,
    /// 最终输出文本。
    pub final_text: Option<String>,
    /// 累计 token 用量。
    pub total_usage: observability::StepUsage,
    /// 总轮次。
    pub turns: u32,
    /// 审批 ID(SUSPENDED 时有值,供 server 路由回执)。
    pub approval_id: Option<String>,
}

/// WRAP_UP 触发原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapUpReason {
    /// 预算耗尽(任一维度剩余 ≤ reserve)。
    BudgetExhausted,
    /// 用户取消。
    Cancelled,
}

/// 审批决策。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approved,
    Denied,
}

/// 待执行的工具调用(从 stream_consumer::StreamToolCall 转换)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingToolCall {
    /// 调用 ID(模型分配,恢复重放时作幂等键)。
    pub call_id: String,
    /// 工具名称。
    pub name: String,
    /// 入参(完整 JSON)。
    pub arguments: serde_json::Value,
    /// 是否完整(C5:MaxTokens 截断时 delta 重建的调用为 false)。
    pub is_complete: bool,
}

impl PendingToolCall {
    /// 从 stream_consumer::StreamToolCall 转换(C5:保留完整性标记)。
    pub fn from_stream(stc: crate::loop_engine::stream_consumer::StreamToolCall) -> Self {
        Self {
            call_id: stc.call.id,
            name: stc.call.name,
            arguments: stc.call.arguments,
            is_complete: stc.is_complete,
        }
    }
}
