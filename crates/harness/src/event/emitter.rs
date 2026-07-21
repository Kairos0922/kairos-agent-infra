//! EventEmitter:单点 emit(取号 + 脱敏 + 包信封 + 发送)。
//!
//! 并发工具 task 只把事件载荷经内部 channel 回传 run 主 task,
//! 由主 task 统一取号发送——seq 严格单调、与状态机事件天然有序(T2-12)。
//! 发送失败(channel 满/关闭)→ try_send 丢弃 + 日志,不影响 run 存活(T2-14)。

use protocol::{AgentEvent, EventEnvelope};
use tokio::sync::mpsc;

/// 事件发射器(per-run 实例,run 主 task 独占)。
pub struct EventEmitter {
    tx: mpsc::Sender<EventEnvelope>,
    run_id: String,
    session_id: String,
    agent_path: String,
    /// run 内单调递增,从 1 开始。主 task 独占,无需原子操作。
    seq: u64,
    sanitizer: Box<dyn super::sanitizer::EventSanitizer>,
}

impl EventEmitter {
    /// 构造事件发射器。
    pub fn new(
        tx: mpsc::Sender<EventEnvelope>,
        run_id: impl Into<String>,
        session_id: impl Into<String>,
        agent_path: impl Into<String>,
        sanitizer: Box<dyn super::sanitizer::EventSanitizer>,
    ) -> Self {
        Self {
            tx,
            run_id: run_id.into(),
            session_id: session_id.into(),
            agent_path: agent_path.into(),
            seq: 0,
            sanitizer,
        }
    }

    /// 发射事件:取号 + 包信封 + 发送。
    ///
    /// 发送失败(channel 满/关闭)时丢弃事件并记日志,不影响 run 存活。
    /// 客户端重连走 Step 回放补齐(observability.md §3)。
    pub fn emit(&mut self, event: AgentEvent) {
        self.seq += 1;
        let envelope = EventEnvelope::new(
            self.seq,
            &self.run_id,
            &self.session_id,
            &self.agent_path,
            event,
        );
        if let Err(e) = self.tx.try_send(envelope) {
            tracing::warn!(
                run_id = %self.run_id,
                seq = self.seq,
                error = %e,
                "事件发送失败(客户端断连或 channel 满),丢弃"
            );
        }
    }

    /// 脱敏工具参数摘要。
    pub fn summarize_args(&self, tool_name: &str, args: &serde_json::Value) -> String {
        self.sanitizer.summarize_args(tool_name, args)
    }

    /// 脱敏工具结果摘要。
    pub fn summarize_result(&self, tool_name: &str, result: &str) -> String {
        self.sanitizer.summarize_result(tool_name, result)
    }

    /// 当前 seq(供恢复场景续接)。
    pub fn current_seq(&self) -> u64 {
        self.seq
    }
}
