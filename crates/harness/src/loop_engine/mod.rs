//! Loop Engine:显式状态机驱动 agent 主循环。
//!
//! 8 个状态全部实现。状态转移是唯一控制流,handler 不向外抛引擎错误(终态必达)。
//! 每个 run 恰好一个 run_finished/run_error(T1-02)。

pub mod budget;
pub mod engine;
pub mod run_context;
pub mod state;
pub mod step_builder;
pub mod stream_consumer;

pub use budget::Budget;
pub use engine::LoopEngine;
pub use run_context::RunContext;
pub use state::LoopState;
