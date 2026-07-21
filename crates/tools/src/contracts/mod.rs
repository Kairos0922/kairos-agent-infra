//! 工具模块契约:ToolSpec / ToolRegistry / ToolExecutor 及关联 DTO。
//!
//! 契约位于模块 crate 内(ADR 0011/0014):领域逻辑只依赖这些 trait,
//! 具体实现由组装根 factory 注入;providers mod 私有,harness 不可见。

mod executor;
mod registry;
mod spec;

pub use executor::{CancelToken, ToolExecuteRequest, ToolExecutor, ToolResult, ToolStatus};
pub use registry::ToolRegistry;
pub use spec::{DangerLevel, ToolSource, ToolSpec};
