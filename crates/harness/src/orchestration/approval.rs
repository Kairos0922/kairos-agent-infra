//! 审批(Approval):SUSPENDED 状态的持久化与恢复。
//!
//! D1 决策(释放模型):SUSPENDED 持久化后 run() 返回,server 调 resume()。
//! Approval 必须持久化工具调用服务端全文(T1-01),非仅摘要。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::PendingToolCall;

/// 审批记录(持久化到 SessionStore 同库)。
///
/// 完整工具参数为服务端执行依据;args_summary 仅为客户端展示视图,两者并存(T1-01)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approval {
    /// 审批标识。
    pub approval_id: String,
    /// 所属 run。
    pub run_id: String,
    /// 待审批的工具调用(服务端全文,T1-01)。
    pub pending_calls: Vec<PendingToolCall>,
    /// 发起时间(超时时钟基准,resume 时按此重算剩余)。
    pub requested_at: DateTime<Utc>,
    /// 超时时间。
    pub expires_at: DateTime<Utc>,
    /// 工具名称(事件展示用)。
    pub tool_names: Vec<String>,
    /// 脱敏参数摘要(事件展示用,非执行依据)。
    pub args_summaries: Vec<String>,
}

impl Approval {
    /// 从多个待审批工具调用构造(批量审批,T2-22)。
    ///
    /// `timeout_secs` 来自 LoopPolicy.approval_timeout_secs(M3 修复:不再硬编码)。
    pub fn new_batch(calls: &[PendingToolCall], timeout_secs: u64) -> Self {
        Self {
            approval_id: format!("apr_{}", uuid::Uuid::new_v4().simple()),
            run_id: String::new(), // 由 LoopEngine 填充
            pending_calls: calls.to_vec(),
            requested_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::seconds(timeout_secs as i64),
            tool_names: calls.iter().map(|c| c.name.clone()).collect(),
            args_summaries: calls.iter().map(|_| String::new()).collect(),
        }
    }

    /// 是否已过期。
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// 剩余等待时间(resume 时重算,≤0 直接按 denied)。
    pub fn remaining_wait(&self) -> chrono::Duration {
        self.expires_at - Utc::now()
    }
}
