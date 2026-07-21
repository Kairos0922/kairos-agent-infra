//! 事件发射与脱敏。
//!
//! 本阶段 loop 直发事件 + EventEmitter 内脱敏(D4 决策);
//! hitl 统一收口随审批流落地。

mod emitter;
mod sanitizer;

pub use emitter::EventEmitter;
pub use sanitizer::{DefaultSanitizer, EventSanitizer};
