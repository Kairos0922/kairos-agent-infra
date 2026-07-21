//! 可观测模块契约:StepSink(写入)与 TraceQuery(查询)。

mod query;
mod sink;

pub use query::TraceQuery;
pub use sink::StepSink;
