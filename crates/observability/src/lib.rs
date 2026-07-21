//! 可观测模块(observability,L1):Step 的持久化与查询。
//!
//! 三个消费方:① loop(写入,checkpoint 语义)② SSE 断线补发与 run 回放
//! ③ eval 回放与 distill 管线(读取)。
//!
//! **不归本模块**:UsageRecord(归 model_gateway)、应用日志(归 foundation/logging)、
//! 指标看板(暂缓)。
//!
//! Step 及关联 DTO 全部自包含:不 import 其他 L1 模块类型(模块间零依赖)。
//! ModelCallRecord 的 tier/deployment/model 为 String,harness 构建 Step 时
//! 从 model_gateway 类型转换(加往返单测固化)。
//!
//! `providers` 为私有 mod(待 SQLite/PostgreSQL 实现时补),上层 crate 不可见。

pub mod contracts;
pub mod step;

pub use contracts::{StepSink, TraceQuery};
pub use step::{
    BudgetSnapshot, ContextDigest, ModelCallRecord, Page, PageRequest, RetrievalQueryRecord,
    RunFilter, RunRecord, RunStatus, Step, StepUsage, ToolCallRecord,
};
