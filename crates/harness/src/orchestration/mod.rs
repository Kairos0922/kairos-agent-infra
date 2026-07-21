//! 工具调度(orchestration):多个调用的安排(并发/权限/审批)。
//!
//! 与 tools 模块的边界(tools.md §0):tools 负责"一次调用怎么正确执行",
//! orchestration 负责"这一轮的这些调用怎么安排"。

mod approval;
mod permission;

pub use approval::Approval;
pub use permission::PermissionPolicy;

use std::sync::Arc;
use std::time::Duration;

use foundation::{KairosError, TenantContext};
use tools::{CancelToken, ToolExecuteRequest, ToolExecutor, ToolResult, ToolSpec};

use crate::types::PendingToolCall;

/// 工具调度器:权限判定先于执行,整批先过 PermissionPolicy(T2-22)。
pub struct ToolOrchestrator {
    executor: Arc<dyn ToolExecutor>,
    /// run 启动时冻结的工具表快照(run 内冻结,tools.md §5)。
    tools: Vec<ToolSpec>,
    permission: PermissionPolicy,
    /// 每工具默认超时。
    per_tool_timeout: Duration,
    /// 审批超时(秒),来自 LoopPolicy.approval_timeout_secs(M3 修复)。
    approval_timeout_secs: u64,
}

/// 批量执行结果。
pub enum OrchestrationOutcome {
    /// 全部执行完毕。
    Completed { results: Vec<ToolResult> },
    /// 有调用需审批,run 应转 SUSPENDED(D1 释放模型)。
    NeedsApproval {
        approval: Approval,
        all_calls: Vec<PendingToolCall>,
    },
    /// 取消打中批处理(T2-07)。
    Cancelled { partial_results: Vec<ToolResult> },
}

impl ToolOrchestrator {
    /// 构造调度器。
    pub fn new(
        executor: Arc<dyn ToolExecutor>,
        tools: Vec<ToolSpec>,
        permission: PermissionPolicy,
        approval_timeout_secs: u64,
    ) -> Self {
        Self {
            executor,
            tools,
            permission,
            per_tool_timeout: Duration::from_secs(60),
            approval_timeout_secs,
        }
    }

    /// 覆写每工具超时。
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.per_tool_timeout = timeout;
        self
    }

    /// 获取冻结工具表(供 engine 转换为 ToolDefinition 注入模型,H9)。
    pub fn tool_specs(&self) -> &[ToolSpec] {
        &self.tools
    }

    /// 批量执行工具调用。
    ///
    /// 权限判定先于执行(T2-22):整批先过 PermissionPolicy,收集全部需审批项。
    /// 并发执行用 JoinSet::spawn 独立 task(panic 被捕获为 JoinError,T2-02)。
    pub async fn execute_batch(
        &self,
        ctx: &TenantContext,
        calls: Vec<PendingToolCall>,
        cancel: CancelToken,
        run_id: &str,
    ) -> Result<OrchestrationOutcome, KairosError> {
        // 1. 权限判定:整批先过
        let mut approved = Vec::new();
        let mut needs_approval = Vec::new();
        for call in &calls {
            if self.permission.requires_approval(call, &self.tools) {
                needs_approval.push(call.clone());
            } else {
                approved.push(call.clone());
            }
        }

        // 2. 有需审批 → 一次 SUSPENDED(批量审批,T2-22)
        if !needs_approval.is_empty() {
            let mut approval = Approval::new_batch(&needs_approval, self.approval_timeout_secs);
            approval.run_id = run_id.to_string();
            return Ok(OrchestrationOutcome::NeedsApproval {
                approval,
                all_calls: calls,
            });
        }

        // 3. 全部不需审批 → 并发执行
        let results = self.execute_approved(ctx, approved, cancel).await?;

        // 检测取消:如果有结果被取消,返回 Cancelled(M5 修复)
        if results
            .iter()
            .any(|r| r.status == tools::ToolStatus::Cancelled)
        {
            return Ok(OrchestrationOutcome::Cancelled {
                partial_results: results,
            });
        }

        Ok(OrchestrationOutcome::Completed { results })
    }

    /// 并发执行已批准的工具调用(JoinSet::spawn 隔离 panic)。
    ///
    /// 每个 task 内持有 call_id,结果通过 ToolResult.call_id 关联回调用(C4)。
    async fn execute_approved(
        &self,
        ctx: &TenantContext,
        calls: Vec<PendingToolCall>,
        cancel: CancelToken,
    ) -> Result<Vec<ToolResult>, KairosError> {
        let mut join_set = tokio::task::JoinSet::new();

        for call in calls {
            let executor = self.executor.clone();
            let child_token = cancel.child_token();
            let ctx = ctx.clone();
            let timeout = self.per_tool_timeout;
            let call_id = call.call_id.clone();

            let request = ToolExecuteRequest {
                call_id: call.call_id.clone(),
                name: call.name.clone(),
                arguments: call.arguments.clone(),
            };

            join_set.spawn(async move {
                let start = std::time::Instant::now();
                // select: 执行 vs 取消
                tokio::select! {
                    result = tokio::time::timeout(
                        timeout,
                        executor.execute(&ctx, request, child_token.clone()),
                    ) => {
                        let elapsed = start.elapsed();
                        match result {
                            Ok(Ok(tool_result)) => tool_result,
                            Ok(Err(e)) => ToolResult::error(
                                call_id,
                                format!("工具执行引擎错误: {e}"),
                                elapsed,
                            ),
                            Err(_) => ToolResult::timeout(call_id, elapsed),
                        }
                    }
                    _ = child_token.cancelled() => {
                        ToolResult::cancelled(call_id, start.elapsed())
                    }
                }
            });
        }

        let mut results = Vec::new();
        while let Some(join_result) = join_set.join_next().await {
            match join_result {
                Ok(tool_result) => results.push(tool_result),
                // panic 被捕获为 JoinError(T2-02)→ 工具级错误
                Err(join_err) if join_err.is_panic() => {
                    tracing::error!(error = %join_err, "工具执行 panic,按工具级错误处理");
                    results.push(ToolResult::error(
                        "unknown",
                        "工具内部错误(internal error)",
                        Duration::ZERO,
                    ));
                }
                Err(join_err) => {
                    return Err(KairosError::provider(
                        "orchestration",
                        format!("工具任务异常: {join_err}"),
                        false,
                    ));
                }
            }
        }

        Ok(results)
    }

    /// 按名称查找工具规格。
    pub fn find_tool(&self, name: &str) -> Option<&ToolSpec> {
        self.tools.iter().find(|t| t.name == name)
    }
}
