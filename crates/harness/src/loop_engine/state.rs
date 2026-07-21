//! LoopState:状态机的 8 个状态及转移逻辑。
//!
//! 状态转移是唯一控制流,不允许状态处理函数内部隐式跳转。
//! handler 返回 Result<LoopState>:Err 由 run() 捕获 → 强制 Finished(failed)(T1-02)。

use observability::RunStatus;

use crate::types::WrapUpReason;

/// 状态机的全部状态。
#[derive(Debug, Clone)]
pub enum LoopState {
    /// 组装分区 prompt。入口检查预算。
    Assemble,
    /// 经 ModelRouter 调模型,流式转发 text_delta。
    ModelCall,
    /// 解析模型输出:工具调用 / 最终回答 / 异常(T1-03 穷举)。
    Route,
    /// 经 Orchestration 执行工具(并发/超时/权限)。
    Execute,
    /// 等待审批回执。run() 返回 Suspended,释放进程资源(D1)。
    Suspended { approval_id: String },
    /// 工具结果规整为观察;写 Step(checkpoint)。
    Observe,
    /// 优雅收尾:注入收尾指令,限一轮,动用 reserve。
    WrapUp { reason: WrapUpReason },
    /// 终态。
    Finished { status: RunStatus },
}

impl LoopState {
    /// 是否为终态。
    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Finished { .. })
    }

    /// 是否为 WRAP_UP(取消守卫用,T2-27)。
    pub fn is_wrap_up(&self) -> bool {
        matches!(self, Self::WrapUp { .. })
    }
}

/// ROUTE 的穷举结果(T1-03)。
#[derive(Debug)]
pub enum RouteDecision {
    /// 有完整工具调用 → 执行。
    Execute,
    /// 最终回答 → 完成。
    Finished,
    /// 工具调用被截断 → 丢弃 + 说明 → Observe。
    TruncatedToolCalls,
    /// 纯文本截断 → 续写(有上限)。
    ContinueText,
    /// 续写超限 → WRAP_UP。
    ContinuationLimitReached,
    /// 内容过滤 → 失败。
    ContentFilter,
    /// 取消 → WRAP_UP。
    Cancelled,
    /// 未覆盖的组合 → fail-loud。
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finished_is_terminal() {
        assert!(LoopState::Finished {
            status: RunStatus::Completed
        }
        .is_finished());
        assert!(!LoopState::Assemble.is_finished());
        assert!(!LoopState::ModelCall.is_finished());
    }

    #[test]
    fn wrap_up_detection() {
        assert!(LoopState::WrapUp {
            reason: WrapUpReason::BudgetExhausted
        }
        .is_wrap_up());
        assert!(!LoopState::Finished {
            status: RunStatus::Completed
        }
        .is_wrap_up());
    }

    #[test]
    fn all_states_constructible() {
        let states = [
            LoopState::Assemble,
            LoopState::ModelCall,
            LoopState::Route,
            LoopState::Execute,
            LoopState::Suspended {
                approval_id: "apr_1".to_string(),
            },
            LoopState::Observe,
            LoopState::WrapUp {
                reason: WrapUpReason::Cancelled,
            },
            LoopState::Finished {
                status: RunStatus::Failed,
            },
        ];
        assert_eq!(states.len(), 8);
    }
}
