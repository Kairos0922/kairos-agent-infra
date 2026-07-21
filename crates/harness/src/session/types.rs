//! Session 相关类型。

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 会话元数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    /// 会话标识。
    pub session_id: String,
    /// 所属用户(认证派生,ADR 0023)。
    pub user_id: String,
    /// 创建时间。
    pub created_at: DateTime<Utc>,
    /// 主题摘要(可选,由模型生成)。
    pub topic_summary: Option<String>,
    /// 会话 scope(S16 演练增补):本会话的默认场景(如 {subject: 物理, class: 高二3班}),
    /// 供记忆检索/写回的 scope 推断兜底(context.md §2.1/§5.1)。
    /// 由内置工具 set_session_scope 写入(tools.md §2)。
    pub scope: Option<HashMap<String, String>>,
    /// 是否已归档。
    pub archived: bool,
}

/// 完整会话(元数据 + 对话历史)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// 元数据。
    pub meta: SessionMeta,
    /// 对话历史(P6 分区数据源)。
    pub history: Vec<HistoryEntry>,
}

/// 对话历史条目(一轮的对话层内容,不含 trace)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// 所属 run 标识。
    pub run_id: String,
    /// 轮次。
    pub turn: u32,
    /// 角色(user / assistant / tool / summary)。
    pub role: String,
    /// 内容文本。
    pub content: String,
    /// 工具调用 ID(role=tool 时)。
    pub tool_call_id: Option<String>,
    /// 工具名称(role=tool 时)。
    pub tool_name: Option<String>,
    /// 是否为压缩摘要段。
    pub is_summary: bool,
    /// 摘要覆盖的原始轮范围(is_summary=true 时)。
    pub summary_covers: Option<(u32, u32)>,
    /// 时间戳。
    pub created_at: DateTime<Utc>,
}

/// 历史替换范围(P6 压缩用)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistorySpan {
    /// 起始轮(含)。
    pub start_turn: u32,
    /// 结束轮(含)。
    pub end_turn: u32,
}

/// 会话过滤条件。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionFilter {
    /// 按用户过滤。
    pub user_id: Option<String>,
    /// 是否包含已归档。
    pub include_archived: bool,
}

/// 分页结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
}
