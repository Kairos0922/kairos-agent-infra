//! RunContext:run 运行态(主 task 独占)。
//!
//! 不变量:
//! - Budget 由 run 主 task 独占,并发任务只回传 usage,禁止共享读写。
//! - live_messages 是 run 内权威数据源(T1-04),SessionStore 是跨 run 持久层。
//! - 全部字段 Send(run future 会被 server 层 tokio::spawn)。

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use model_gateway::{ChatMessage, StopReason};
use observability::{ContextDigest, ModelCallRecord, StepUsage};
use tokio_util::sync::CancellationToken;

use super::budget::Budget;
use crate::context::assembly::AssembledContext;
use crate::context::retrievers::{KnowledgeFragment, MemoryFragment};
use crate::types::PendingToolCall;

/// run 运行态(主 task 独占)。
pub struct RunContext {
    /// run 标识。
    pub run_id: String,
    /// 所属 session。
    pub session_id: String,
    /// Profile 引用。
    pub profile_ref: String,
    /// 预算(主 task 独占)。
    pub budget: Budget,
    /// run 内权威消息日志(含 provider_resume_state,T1-04)。
    pub live_messages: Vec<ChatMessage>,
    /// 当前轮工具观察。
    pub observations: Vec<ChatMessage>,
    /// 待执行工具调用。
    pub pending_tool_calls: Vec<PendingToolCall>,
    /// 本轮停止原因。
    pub last_stop_reason: Option<StopReason>,
    /// 本轮模型调用记录。
    pub last_model_call: Option<ModelCallRecord>,
    /// 本轮 ContextDigest。
    pub last_digest: Option<ContextDigest>,
    /// 本轮组装结果(C1 修复:on_model_call 用此构建 ChatRequest)。
    pub assembled: Option<AssembledContext>,
    /// 统一取消树(D2)。
    pub cancel_token: CancellationToken,
    /// MaxTokens 续写计数(M1 上限 3 次)。
    pub continuation_count: u32,
    /// 工具连续失败计数(同一工具连续 3 次 → 注入停止指令)。
    pub consecutive_failures: HashMap<String, u32>,
    /// 当前轮次。
    pub turn: u32,
    /// 本轮开始时间(M7 修复:Step.started_at 用此,非 run 创建时间)。
    pub turn_started_at: DateTime<Utc>,
    /// 最终输出文本。
    pub final_text: Option<String>,
    /// 累计用量。
    pub total_usage: StepUsage,
    /// 缓存的 P5 记忆片段(run 内复用,H8 修复:assemble 后写回)。
    pub cached_memory: Option<Vec<MemoryFragment>>,
    /// 缓存的 P4 知识片段(run 内复用,H8 修复:assemble 后写回)。
    pub cached_knowledge: Option<Vec<KnowledgeFragment>>,
    /// 是否为新用户消息(决定是否重新检索 P4/P5)。
    pub is_new_user_message: bool,
    /// 用户原始消息(P4/P5 检索 query 用,不随 observations.clear() 丢失,H4 修复)。
    pub user_message: String,
}

impl RunContext {
    /// 构造运行态。
    pub fn new(
        run_id: String,
        session_id: String,
        profile_ref: String,
        budget: Budget,
        cancel_token: CancellationToken,
        user_message: String,
    ) -> Self {
        Self {
            run_id,
            session_id,
            profile_ref,
            budget,
            live_messages: Vec::new(),
            observations: Vec::new(),
            pending_tool_calls: Vec::new(),
            last_stop_reason: None,
            last_model_call: None,
            last_digest: None,
            assembled: None,
            cancel_token,
            continuation_count: 0,
            consecutive_failures: HashMap::new(),
            turn: 0,
            turn_started_at: Utc::now(),
            final_text: None,
            total_usage: StepUsage::default(),
            cached_memory: None,
            cached_knowledge: None,
            is_new_user_message: true,
            user_message,
        }
    }

    /// 取消是否已请求。
    pub fn cancel_requested(&self) -> bool {
        self.cancel_token.is_cancelled()
    }

    /// 取出待执行工具调用(清空 pending)。
    pub fn take_pending_calls(&mut self) -> Vec<PendingToolCall> {
        std::mem::take(&mut self.pending_tool_calls)
    }

    /// 丢弃不完整的工具调用(MaxTokens 截断,T1-03)。
    pub fn discard_incomplete_calls(&mut self) {
        self.pending_tool_calls.retain(|c| c.is_complete);
    }

    /// 注入观察消息(续写/工具错误说明等)。
    pub fn inject_observation(&mut self, text: &str) {
        self.observations.push(ChatMessage::User {
            content: text.to_string(),
        });
    }

    /// 记录工具连续失败。返回该工具是否已达 3 次。
    pub fn record_tool_failure(&mut self, tool_name: &str) -> bool {
        let count = self
            .consecutive_failures
            .entry(tool_name.to_string())
            .or_insert(0);
        *count += 1;
        *count >= 3
    }

    /// 重置工具连续失败计数(成功时)。
    pub fn reset_tool_failure(&mut self, tool_name: &str) {
        self.consecutive_failures.remove(tool_name);
    }

    /// 累计用量。
    pub fn accumulate_usage(&mut self, usage: &StepUsage) {
        let add = |a: Option<u64>, b: Option<u64>| match (a, b) {
            (Some(x), Some(y)) => Some(x + y),
            (Some(x), None) => Some(x),
            (None, Some(y)) => Some(y),
            (None, None) => None,
        };
        self.total_usage.input_tokens = add(self.total_usage.input_tokens, usage.input_tokens);
        self.total_usage.output_tokens = add(self.total_usage.output_tokens, usage.output_tokens);
        self.total_usage.reasoning_tokens =
            add(self.total_usage.reasoning_tokens, usage.reasoning_tokens);
        self.total_usage.cached_input_tokens = add(
            self.total_usage.cached_input_tokens,
            usage.cached_input_tokens,
        );
        self.total_usage.cost_micro_usd =
            add(self.total_usage.cost_micro_usd, usage.cost_micro_usd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loop_engine::stream_consumer::StreamToolCall;

    fn rc() -> RunContext {
        RunContext::new(
            "run_1".to_string(),
            "ses_1".to_string(),
            "profile_1".to_string(),
            Budget::new(20, 200_000, 1_000_000, None, 0.1),
            CancellationToken::new(),
            "你好".to_string(),
        )
    }

    #[test]
    fn new_initializes_fields() {
        let rc = rc();
        assert_eq!(rc.run_id, "run_1");
        assert_eq!(rc.user_message, "你好");
        assert_eq!(rc.turn, 0);
        assert!(rc.is_new_user_message);
        assert!(rc.assembled.is_none());
        assert!(rc.final_text.is_none());
    }

    #[test]
    fn take_pending_calls_clears() {
        let mut rc = rc();
        rc.pending_tool_calls.push(PendingToolCall {
            call_id: "c1".to_string(),
            name: "tool".to_string(),
            arguments: serde_json::Value::Null,
            is_complete: true,
        });
        let calls = rc.take_pending_calls();
        assert_eq!(calls.len(), 1);
        assert!(rc.pending_tool_calls.is_empty());
    }

    #[test]
    fn discard_incomplete_calls() {
        let mut rc = rc();
        rc.pending_tool_calls.push(PendingToolCall {
            call_id: "c1".to_string(),
            name: "complete".to_string(),
            arguments: serde_json::json!({"a": 1}),
            is_complete: true,
        });
        rc.pending_tool_calls.push(PendingToolCall {
            call_id: "c2".to_string(),
            name: "truncated".to_string(),
            arguments: serde_json::Value::Null,
            is_complete: false,
        });
        rc.discard_incomplete_calls();
        assert_eq!(rc.pending_tool_calls.len(), 1);
        assert_eq!(rc.pending_tool_calls[0].call_id, "c1");
    }

    #[test]
    fn record_tool_failure_threshold() {
        let mut rc = rc();
        assert!(!rc.record_tool_failure("tool_a"));
        assert!(!rc.record_tool_failure("tool_a"));
        assert!(rc.record_tool_failure("tool_a")); // 第 3 次
                                                   // 不同工具独立计数
        assert!(!rc.record_tool_failure("tool_b"));
    }

    #[test]
    fn reset_tool_failure() {
        let mut rc = rc();
        rc.record_tool_failure("tool_a");
        rc.record_tool_failure("tool_a");
        rc.reset_tool_failure("tool_a");
        assert!(!rc.record_tool_failure("tool_a")); // 重置后重新计数
    }

    #[test]
    fn accumulate_usage_option_combinations() {
        let mut rc = rc();
        // Some + Some
        rc.accumulate_usage(&StepUsage {
            input_tokens: Some(100),
            output_tokens: Some(50),
            ..Default::default()
        });
        assert_eq!(rc.total_usage.input_tokens, Some(100));
        assert_eq!(rc.total_usage.output_tokens, Some(50));

        // Some + None → 保持 Some
        rc.accumulate_usage(&StepUsage {
            input_tokens: None,
            output_tokens: Some(30),
            ..Default::default()
        });
        assert_eq!(rc.total_usage.input_tokens, Some(100));
        assert_eq!(rc.total_usage.output_tokens, Some(80));

        // None + None → 保持 None
        assert_eq!(rc.total_usage.reasoning_tokens, None);
    }

    #[test]
    fn cancel_requested() {
        let rc = rc();
        assert!(!rc.cancel_requested());
        rc.cancel_token.cancel();
        assert!(rc.cancel_requested());
    }

    #[test]
    fn pending_tool_call_from_stream() {
        let stc = StreamToolCall {
            call: model_gateway::ToolCall {
                id: "call_1".to_string(),
                name: "search".to_string(),
                arguments: serde_json::json!({"q": "test"}),
            },
            is_complete: true,
        };
        let ptc = PendingToolCall::from_stream(stc);
        assert_eq!(ptc.call_id, "call_1");
        assert!(ptc.is_complete);

        let stc_incomplete = StreamToolCall {
            call: model_gateway::ToolCall {
                id: "call_2".to_string(),
                name: "truncated".to_string(),
                arguments: serde_json::Value::Null,
            },
            is_complete: false,
        };
        let ptc2 = PendingToolCall::from_stream(stc_incomplete);
        assert!(!ptc2.is_complete);
    }
}
