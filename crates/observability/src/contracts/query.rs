//! TraceQuery:Step/Run 查询契约(回放/eval/控制台)。

use async_trait::async_trait;
use foundation::{KairosError, TenantContext};

use crate::step::{Page, PageRequest, RunFilter, RunRecord, Step};

/// Run/Step 查询接口。消费方:SSE 断线补发与 run 回放、eval 回放与 distill 管线。
///
/// 全部方法带 `ctx`,租户内可见;跨租户的运维查询走 server 管理面单独端点。
#[async_trait]
pub trait TraceQuery: Send + Sync {
    /// 获取 run 级汇总。
    async fn get_run(&self, ctx: &TenantContext, run_id: &str) -> Result<RunRecord, KairosError>;

    /// 分页列出 run(按 user/profile/status/时间窗过滤)。
    async fn list_runs(
        &self,
        ctx: &TenantContext,
        filter: RunFilter,
        page: PageRequest,
    ) -> Result<Page<RunRecord>, KairosError>;

    /// 获取一个 run 的全部 Step(回放/eval)。
    async fn get_steps(&self, ctx: &TenantContext, run_id: &str) -> Result<Vec<Step>, KairosError>;
}
