//! harness(L2):Agent 运行时骨架,唯一允许编排多个 L1 模块的层。
//!
//! 只依赖各模块 contracts(公开 trait),禁触任何 providers——
//! 由 crate 依赖边界 + 私有可见性强制(ADR 0014/0021)。详见 docs/harness/。
//!
//! 核心组件:
//! - `loop_engine`:显式状态机驱动 agent 主循环(ASSEMBLE→MODEL_CALL→ROUTE→EXECUTE→OBSERVE)
//! - `context`:分区组装 prompt(P1–P7,按稳定性递减排列,最大化 prompt cache 命中)
//! - `orchestration`:工具调度(并发/权限/审批)
//! - `session`:SessionStore 契约(harness 层内)
//! - `event`:EventEmitter(单点 emit + 脱敏)
//!
//! H10 修复:公开 API 涉及的 L1 类型做 re-export,server 层无需直接依赖 L1 crate。

pub mod context;
pub mod event;
pub mod loop_engine;
pub mod orchestration;
pub mod policy;
pub mod session;
pub mod types;

// H10 修复:re-export 公开 API 涉及的 L1 类型,server 只需依赖 harness。
pub use model_gateway::ModelTier;
pub use observability::{RunStatus, StepUsage};
