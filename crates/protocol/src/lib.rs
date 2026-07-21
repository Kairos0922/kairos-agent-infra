//! protocol:agent-events + 控制 API 的类型定义。
//!
//! Runtime(Rust)与 UI(TS)之间的稳定跨语言边界(ADR 0021)。事件 type 与 wire 字段
//! 用 snake_case,与 TS 侧 `packages/protocol-ts` 对齐。
//!
//! **Schema 事实源**:`events::AgentEvent` 枚举(ADR 修订 agent-events.md §4,
//! 归 protocol crate 与 crate 定位一致)。JSON Schema 由 CI 经 schemars 导出。

pub mod events;

pub use events::{AgentEvent, BudgetInfo, EventEnvelope, UsageInfo, PROTOCOL_VERSION};
