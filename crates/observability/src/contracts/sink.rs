//! StepSink:Step 写入契约(checkpoint 语义)。

use async_trait::async_trait;
use foundation::{KairosError, TenantContext};

use crate::step::Step;

/// Step 写入接口。loop 的 checkpoint 依赖:append 成功才进下一轮。
///
/// 幂等:同 `(run_id, agent_path, turn)` 重复写入覆盖而非报错
/// (恢复场景会重放最后一轮)。
#[async_trait]
pub trait StepSink: Send + Sync {
    /// 追加一条 Step。写入成功才允许 loop 进入下一轮(checkpoint 语义优先于吞吐)。
    async fn append(&self, ctx: &TenantContext, step: Step) -> Result<(), KairosError>;
}
