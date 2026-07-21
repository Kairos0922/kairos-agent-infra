//! Session 与 HITL:harness 层内契约。
//!
//! Session 是与一位 user 的持续对话容器,持有 history 与元数据。
//! Run 是 session 内一次任务执行(一次状态机生命周期),session : run = 1 : N。
//! Context Engine 的 P6(history)从 session 读;run 的 Step 归 observability;
//! session 只存对话层内容,不复制 trace(session-hitl.md §1)。

mod store;
mod types;

pub use store::SessionStore;
pub use types::{HistoryEntry, HistorySpan, Page, Session, SessionFilter, SessionMeta};
