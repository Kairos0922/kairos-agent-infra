//! StepBuilder:从 RunContext 当前轮数据构建不可变 Step。

use chrono::Utc;
use observability::{BudgetSnapshot, ContextDigest, ModelCallRecord, Step, ToolCallRecord};

/// Step 构建器。
pub struct StepBuilder {
    run_id: String,
    agent_path: String,
    turn: u32,
}

impl StepBuilder {
    /// 构造构建器。
    pub fn new(run_id: String, agent_path: String, turn: u32) -> Self {
        Self {
            run_id,
            agent_path,
            turn,
        }
    }

    /// 构建不可变 Step。
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        self,
        digest: ContextDigest,
        model_call: ModelCallRecord,
        tool_calls: Vec<ToolCallRecord>,
        stop_reason: String,
        budget_snapshot: BudgetSnapshot,
        started_at: chrono::DateTime<Utc>,
    ) -> Step {
        Step {
            run_id: self.run_id,
            agent_path: self.agent_path,
            turn: self.turn,
            context_digest: digest,
            model_call,
            tool_calls,
            stop_reason,
            budget_snapshot,
            started_at,
            ended_at: Utc::now(),
        }
    }
}
