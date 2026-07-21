//! Agent 事件协议(agent-events)的 Rust 类型定义。
//!
//! 本协议是 L4(server)与 L5(客户端:CLI/行业APP)之间的唯一耦合面。
//! 事件 type 与 wire 字段用 snake_case,与 TS 侧 `packages/protocol-ts` 对齐。
//!
//! **Schema 事实源**:本文件的 `AgentEvent` 枚举(ADR 修订 agent-events.md §4)。
//! JSON Schema 由 CI 经 schemars 自动导出,与文档不一致视为 CI 失败。
//!
//! **v1.0 冻结**:15 种事件变体全部定义;本阶段不发射的 4 种(SubagentSpawned/
//! SubagentFinished/ThinkingDelta/MemoryWritten)为保留位,结构体字段按 §3 定义,
//! 发射逻辑留空。YAGNI 的边界在"不发射"而非"不定义"。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 协议版本号。
pub const PROTOCOL_VERSION: &str = "kairos-events/1.0";

/// 事件信封(所有事件统一,见 agent-events.md §2)。
///
/// `payload` 为**嵌套对象**(非 flatten),与协议文档 §2 的 wire format 一致:
/// ```json
/// {
///   "protocol": "kairos-events/1.0",
///   "seq": 42,
///   "type": "text_delta",
///   "payload": { "delta": "hello" }
/// }
/// ```
///
/// **只 Serialize 不 Deserialize**(H1 修复):untagged AgentEvent 反序列化有歧义
/// (Heartbeat{} 匹配任意 JSON 对象),Rust 侧消费事件应按 `event_type` 字段手动分发。
/// TS 客户端按 type 字段解析,不依赖 Rust 反序列化。
#[derive(Debug, Clone, Serialize)]
pub struct EventEnvelope {
    /// 协议版本。
    pub protocol: String,
    /// run 内单调递增,从 1 开始(见 agent-events.md §2)。
    pub seq: u64,
    /// 所属 run 标识。
    pub run_id: String,
    /// 所属 session 标识。
    pub session_id: String,
    /// agent 路径:"root" 或 "root/0/1"(sub-agent 派生树位置)。
    pub agent_path: String,
    /// 事件类型(wire 值,snake_case)。由 `AgentEvent::type_tag()` 映射。
    #[serde(rename = "type")]
    pub event_type: String,
    /// 事件时间戳。
    pub ts: DateTime<Utc>,
    /// 事件载荷(按 type 定义)。
    pub payload: AgentEvent,
}

impl EventEnvelope {
    /// 构造信封:自动填充 protocol 版本、type_tag 与时间戳。
    pub fn new(
        seq: u64,
        run_id: impl Into<String>,
        session_id: impl Into<String>,
        agent_path: impl Into<String>,
        payload: AgentEvent,
    ) -> Self {
        let event_type = payload.type_tag().to_string();
        Self {
            protocol: PROTOCOL_VERSION.to_string(),
            seq,
            run_id: run_id.into(),
            session_id: session_id.into(),
            agent_path: agent_path.into(),
            event_type,
            ts: Utc::now(),
            payload,
        }
    }
}

/// v1.0 冻结事件全集(15 种,见 agent-events.md §3)。
///
/// **untagged**:payload 序列化为纯内容对象(不含变体名包装),type 由
/// `EventEnvelope.event_type` 承载,与协议 §2 的 wire format 一致。
/// 反序列化歧义(如 TextDelta vs ThinkingDelta 都只有 delta 字段)由信封的
/// type 字段消歧——直接反序列化 AgentEvent 不是真实使用场景。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AgentEvent {
    // ── 生命周期 ──
    /// run 开始。客户端收到的第一个事件。
    RunStarted {
        profile_ref: String,
        budget: BudgetInfo,
    },

    /// run 结束(正常/预算耗尽/取消)。
    RunFinished {
        status: String,
        usage: UsageInfo,
        turns: u32,
        final_text: Option<String>,
    },

    /// run 失败。message 不含堆栈与内部路径(agent-events.md §5)。
    RunError {
        code: String,
        message: String,
        retryable: bool,
    },

    /// 心跳(空闲 15s 发一次)。
    Heartbeat {},

    // ── 循环与文本 ──
    /// 新一轮开始。
    StepStarted { turn: u32 },

    /// 助手回复增量(流式)。
    TextDelta { delta: String },

    /// 一轮完成。
    StepCompleted {
        turn: u32,
        usage: UsageInfo,
        stop_reason: String,
    },

    // ── 工具 ──
    /// 工具调用开始。args_summary 经脱敏(非完整参数)。
    ToolCallStarted {
        call_id: String,
        tool_name: String,
        args_summary: String,
    },

    /// 工具调用结果。result_summary 经脱敏。
    ToolCallResult {
        call_id: String,
        status: String,
        result_summary: String,
    },

    // ── 审批(HITL) ──
    /// 需要审批。args_summary 经脱敏。
    ApprovalRequired {
        approval_id: String,
        tool_name: String,
        reason: String,
        args_summary: String,
        expires_at: DateTime<Utc>,
    },

    /// 审批已解决。
    ApprovalResolved {
        approval_id: String,
        decision: String,
        by: String,
    },

    // ── Sub-agent(v1.0 保留位,本阶段不发射) ──
    /// 子 agent 派生。
    SubagentSpawned {
        child_path: String,
        profile_ref: String,
        task_summary: String,
        budget: BudgetInfo,
    },

    /// 子 agent 完成。
    SubagentFinished {
        child_path: String,
        status: String,
        usage: UsageInfo,
    },

    // ── 保留位(v1.0 定义但默认不发,开关在 Profile) ──
    /// 模型推理过程增量(教育教师场景默认关闭)。
    ThinkingDelta { delta: String },

    /// 记忆写入通知(kind + id,无明文;默认关闭)。
    MemoryWritten { kind: String, id: String },
}

impl AgentEvent {
    /// 事件的 wire type 标签(snake_case)。
    pub fn type_tag(&self) -> &'static str {
        match self {
            Self::RunStarted { .. } => "run_started",
            Self::RunFinished { .. } => "run_finished",
            Self::RunError { .. } => "run_error",
            Self::Heartbeat {} => "heartbeat",
            Self::StepStarted { .. } => "step_started",
            Self::TextDelta { .. } => "text_delta",
            Self::StepCompleted { .. } => "step_completed",
            Self::ToolCallStarted { .. } => "tool_call_started",
            Self::ToolCallResult { .. } => "tool_call_result",
            Self::ApprovalRequired { .. } => "approval_required",
            Self::ApprovalResolved { .. } => "approval_resolved",
            Self::SubagentSpawned { .. } => "subagent_spawned",
            Self::SubagentFinished { .. } => "subagent_finished",
            Self::ThinkingDelta { .. } => "thinking_delta",
            Self::MemoryWritten { .. } => "memory_written",
        }
    }
}

/// 预算信息(run_started 载荷)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetInfo {
    pub max_turns: u32,
    pub max_tokens: u64,
    pub deadline: Option<DateTime<Utc>>,
}

/// token 用量信息(事件载荷中的 usage 字段)。
///
/// protocol 自包含 wire DTO,不引用 model_gateway/observability 的类型。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsageInfo {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub cost_micro_usd: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_wire_format() {
        let event = AgentEvent::TextDelta {
            delta: "hello".to_string(),
        };
        let envelope = EventEnvelope::new(1, "run_1", "ses_1", "root", event);
        let json = serde_json::to_value(&envelope).unwrap();

        // 验证 §2 信封格式:payload 为嵌套对象
        assert_eq!(json["protocol"], "kairos-events/1.0");
        assert_eq!(json["seq"], 1);
        assert_eq!(json["type"], "text_delta");
        assert_eq!(json["payload"]["delta"], "hello");
        // payload 是嵌套对象,不是平铺
        assert!(json["payload"].is_object());
        assert!(json.get("delta").is_none());
    }

    #[test]
    fn all_type_tags_are_snake_case() {
        let events = vec![
            AgentEvent::RunStarted {
                profile_ref: "p".into(),
                budget: BudgetInfo {
                    max_turns: 20,
                    max_tokens: 200000,
                    deadline: None,
                },
            },
            AgentEvent::RunFinished {
                status: "completed".into(),
                usage: UsageInfo::default(),
                turns: 1,
                final_text: None,
            },
            AgentEvent::RunError {
                code: "e".into(),
                message: "m".into(),
                retryable: false,
            },
            AgentEvent::Heartbeat {},
            AgentEvent::StepStarted { turn: 1 },
            AgentEvent::TextDelta { delta: "d".into() },
            AgentEvent::StepCompleted {
                turn: 1,
                usage: UsageInfo::default(),
                stop_reason: "end_turn".into(),
            },
            AgentEvent::ToolCallStarted {
                call_id: "c".into(),
                tool_name: "t".into(),
                args_summary: "a".into(),
            },
            AgentEvent::ToolCallResult {
                call_id: "c".into(),
                status: "ok".into(),
                result_summary: "r".into(),
            },
            AgentEvent::ApprovalRequired {
                approval_id: "a".into(),
                tool_name: "t".into(),
                reason: "r".into(),
                args_summary: "a".into(),
                expires_at: Utc::now(),
            },
            AgentEvent::ApprovalResolved {
                approval_id: "a".into(),
                decision: "approved".into(),
                by: "user".into(),
            },
            AgentEvent::SubagentSpawned {
                child_path: "root/0".into(),
                profile_ref: "p".into(),
                task_summary: "t".into(),
                budget: BudgetInfo {
                    max_turns: 5,
                    max_tokens: 50000,
                    deadline: None,
                },
            },
            AgentEvent::SubagentFinished {
                child_path: "root/0".into(),
                status: "completed".into(),
                usage: UsageInfo::default(),
            },
            AgentEvent::ThinkingDelta { delta: "d".into() },
            AgentEvent::MemoryWritten {
                kind: "episodic".into(),
                id: "m1".into(),
            },
        ];

        for event in &events {
            let tag = event.type_tag();
            // 全小写 + 下划线
            assert!(
                tag.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "type_tag 应为 snake_case: {tag}"
            );
        }

        // 15 种变体全覆盖
        assert_eq!(events.len(), 15);
    }

    #[test]
    fn envelope_serialization() {
        let event = AgentEvent::RunFinished {
            status: "completed".into(),
            usage: UsageInfo {
                input_tokens: Some(100),
                output_tokens: Some(50),
                total_tokens: Some(150),
                cost_micro_usd: Some(500),
            },
            turns: 3,
            final_text: Some("done".into()),
        };
        let envelope = EventEnvelope::new(5, "run_1", "ses_1", "root", event);
        let json = serde_json::to_value(&envelope).unwrap();
        assert_eq!(json["seq"], 5);
        assert_eq!(json["type"], "run_finished");
        assert_eq!(json["payload"]["status"], "completed");
        assert_eq!(json["payload"]["turns"], 3);
        assert_eq!(json["payload"]["final_text"], "done");
    }

    #[test]
    fn usage_info_default_is_empty() {
        let u = UsageInfo::default();
        assert!(u.input_tokens.is_none());
        assert!(u.cost_micro_usd.is_none());
    }
}
