//! Step 及关联 DTO:全部自包含,不 import 其他 L1 模块类型。
//!
//! Step 是三位一体:trace 单元、checkpoint 单元、事件重建源(见 docs/harness/loop.md §2)。
//! harness 构建 Step 时从 model_gateway/tools 的类型转换为本模块的自包含 DTO。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 一轮的不可变记录。构造后字段只读(无 pub setter),一轮一条。
///
/// 写入时机:OBSERVE 末尾(或 ROUTE 直达 FINISHED 前)经 StepSink 落盘。
/// 明文边界:Step 含工具完整参数与模型输出(服务端侧),但 ContextDigest
/// 不含记忆/知识明文,只含各分区的 id 列表与哈希。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// 所属 run 标识。
    pub run_id: String,
    /// agent 路径:"root" 或 "root/0/1"(sub-agent 派生树位置)。
    pub agent_path: String,
    /// 轮次,从 1 起。
    pub turn: u32,
    /// 分区组装的摘要与哈希(非全文)。
    pub context_digest: ContextDigest,
    /// 本轮模型调用记录。
    pub model_call: ModelCallRecord,
    /// 本轮工具调用记录(可能为空——纯文本回复轮)。
    pub tool_calls: Vec<ToolCallRecord>,
    /// 本轮停止原因(与 model_gateway::StopReason 的 wire 值对齐:
    /// end_turn / tool_use / max_tokens / content_filter / cancelled)。
    pub stop_reason: String,
    /// 本轮结束时预算余量快照。
    pub budget_snapshot: BudgetSnapshot,
    /// 本轮开始时间。
    pub started_at: DateTime<Utc>,
    /// 本轮结束时间。
    pub ended_at: DateTime<Utc>,
}

/// 分区组装的摘要(入 Step,支撑回放重建与 eval 归因)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextDigest {
    /// 各分区的 token 用量(分区名 → token 数)。
    pub partition_tokens: Vec<(String, usize)>,
    /// 内容 id 列表(记忆 id / 知识切片 id / 已加载 Skill name)。
    pub content_ids: Vec<String>,
    /// 各分区的内容哈希(分区名 → 哈希hex)。
    pub partition_hashes: Vec<(String, String)>,
    /// P4/P5 的检索 query 与参数(供 eval 区分"query 构造差"还是"召回差")。
    pub retrieval_queries: Vec<RetrievalQueryRecord>,
}

/// P4/P5 检索的 query 记录(供 eval 归因)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalQueryRecord {
    /// 分区名("knowledge" 或 "memory")。
    pub partition: String,
    /// 检索 query 文本。
    pub query: String,
    /// 检索参数(scope filter 等,JSON)。
    pub params: serde_json::Value,
    /// 返回的结果 id 列表。
    pub result_ids: Vec<String>,
}

/// 模型调用记录(自包含 DTO,tier/deployment/model 为 String)。
///
/// harness 构建时从 model_gateway::ModelTier/ModelIdentity/TokenUsage 转换。
/// tier 取值范围:strong / fast / cheap(与 serde rename_all 后的 wire 值一致)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCallRecord {
    /// 模型档位(强/快/廉)。
    pub tier: String,
    /// 部署别名。
    pub deployment: String,
    /// provider 配置别名。
    pub provider: String,
    /// 实际发给厂商 API 的模型 ID。
    pub model: String,
    /// 请求内容哈希(不含明文,供审计)。
    pub request_hash: String,
    /// 输出摘要(脱敏后)。
    pub output_summary: String,
    /// token 用量。
    pub usage: StepUsage,
}

/// token 用量(自包含,与 model_gateway::TokenUsage 字段对齐但独立定义)。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StepUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    /// 本次路由的总记账(含重试消耗,由 gateway 在流结束时附)。
    pub cost_micro_usd: Option<u64>,
}

/// 工具调用记录(自包含 DTO)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// 调用 ID(模型分配,恢复重放时作幂等键)。
    pub call_id: String,
    /// 工具名称。
    pub name: String,
    /// 完整入参(服务端全文,不进客户端事件)。
    pub arguments: serde_json::Value,
    /// 执行结果内容。
    pub result: String,
    /// 执行状态(ok / error / timeout / cancelled / denied)。
    pub status: String,
    /// 执行耗时(毫秒)。
    pub elapsed_ms: u64,
}

/// 预算余量快照(入 Step)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetSnapshot {
    /// 剩余轮次。
    pub remaining_turns: u32,
    /// 剩余 token 数。
    pub remaining_tokens: u64,
    /// 剩余预算(微美元)。
    pub remaining_cost_micro_usd: u64,
    /// 截止时间(墙上时钟)。
    pub deadline: Option<DateTime<Utc>>,
}

/// Run 级汇总记录(runs 表,run 启动即落,终态时更新)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    /// run 标识。
    pub run_id: String,
    /// 所属 session。
    pub session_id: String,
    /// run 状态。
    pub status: RunStatus,
    /// 累计 token 用量。
    pub usage: StepUsage,
    /// Profile 引用标识。
    pub profile_ref: String,
    /// 总轮次。
    pub turns: u32,
    /// 启动时间。
    pub started_at: DateTime<Utc>,
    /// 结束时间(活跃中为 None)。
    pub ended_at: Option<DateTime<Utc>>,
    /// 最终输出文本(完成时)。
    pub final_text: Option<String>,
    /// 错误信息(失败时,经脱敏)。
    pub error_message: Option<String>,
}

/// Run 状态(归属 observability:runs 汇总是其持久化职责)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// 活跃执行中。
    Active,
    /// 等待审批(挂起)。
    Suspended,
    /// 正常完成。
    Completed,
    /// 预算耗尽。
    BudgetExhausted,
    /// 用户取消。
    Cancelled,
    /// 失败。
    Failed,
}

/// Run 过滤条件(TraceQuery::list_runs)。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunFilter {
    pub user_id: Option<String>,
    pub profile_ref: Option<String>,
    pub status: Option<RunStatus>,
    pub started_after: Option<DateTime<Utc>>,
    pub started_before: Option<DateTime<Utc>>,
}

/// 分页请求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageRequest {
    /// 页码(从 1 起)。
    pub page: u32,
    /// 每页条数。
    pub per_page: u32,
}

impl Default for PageRequest {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 20,
        }
    }
}

/// 分页结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_step() -> Step {
        Step {
            run_id: "run_001".to_string(),
            agent_path: "root".to_string(),
            turn: 1,
            context_digest: ContextDigest {
                partition_tokens: vec![("persona".to_string(), 100)],
                content_ids: vec![],
                partition_hashes: vec![],
                retrieval_queries: vec![],
            },
            model_call: ModelCallRecord {
                tier: "strong".to_string(),
                deployment: "gpt4o".to_string(),
                provider: "openai_compat".to_string(),
                model: "gpt-4o".to_string(),
                request_hash: "abc123".to_string(),
                output_summary: "回答了用户问题".to_string(),
                usage: StepUsage {
                    input_tokens: Some(500),
                    output_tokens: Some(200),
                    ..Default::default()
                },
            },
            tool_calls: vec![],
            stop_reason: "end_turn".to_string(),
            budget_snapshot: BudgetSnapshot {
                remaining_turns: 19,
                remaining_tokens: 199300,
                remaining_cost_micro_usd: 990000,
                deadline: None,
            },
            started_at: Utc::now(),
            ended_at: Utc::now(),
        }
    }

    #[test]
    fn step_serde_roundtrip() {
        let step = sample_step();
        let json = serde_json::to_string(&step).unwrap();
        let back: Step = serde_json::from_str(&json).unwrap();
        assert_eq!(back.run_id, "run_001");
        assert_eq!(back.turn, 1);
        assert_eq!(back.model_call.tier, "strong");
        assert_eq!(back.stop_reason, "end_turn");
    }

    #[test]
    fn run_status_serde_wire_format() {
        assert_eq!(
            serde_json::to_value(RunStatus::Completed).unwrap(),
            serde_json::json!("completed")
        );
        assert_eq!(
            serde_json::to_value(RunStatus::BudgetExhausted).unwrap(),
            serde_json::json!("budget_exhausted")
        );
        assert_eq!(
            serde_json::to_value(RunStatus::Suspended).unwrap(),
            serde_json::json!("suspended")
        );
    }

    #[test]
    fn tool_call_record_serde() {
        let rec = ToolCallRecord {
            call_id: "call_1".to_string(),
            name: "search_memory".to_string(),
            arguments: serde_json::json!({"query": "test"}),
            result: "found 3 results".to_string(),
            status: "ok".to_string(),
            elapsed_ms: 150,
        };
        let json = serde_json::to_value(&rec).unwrap();
        assert_eq!(json["call_id"], "call_1");
        assert_eq!(json["elapsed_ms"], 150);
    }

    #[test]
    fn budget_snapshot_serde() {
        let snap = BudgetSnapshot {
            remaining_turns: 10,
            remaining_tokens: 100000,
            remaining_cost_micro_usd: 500000,
            deadline: Some(Utc::now()),
        };
        let json = serde_json::to_string(&snap).unwrap();
        let back: BudgetSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back.remaining_turns, 10);
        assert!(back.deadline.is_some());
    }

    #[test]
    fn page_request_default() {
        let pr = PageRequest::default();
        assert_eq!(pr.page, 1);
        assert_eq!(pr.per_page, 20);
    }

    #[test]
    fn run_record_serde() {
        let rec = RunRecord {
            run_id: "run_001".to_string(),
            session_id: "ses_001".to_string(),
            status: RunStatus::Active,
            usage: StepUsage::default(),
            profile_ref: "assistant_v1".to_string(),
            turns: 0,
            started_at: Utc::now(),
            ended_at: None,
            final_text: None,
            error_message: None,
        };
        let json = serde_json::to_value(&rec).unwrap();
        assert_eq!(json["status"], "active");
        assert!(json["ended_at"].is_null());
    }
}
