//! Context Engine:分区组装 prompt。
//!
//! 7 个分区按稳定性递减排列(P1 persona → P7 task),最大化 provider 侧
//! prompt cache 命中。配额基数 = context_window − reserved_output(T1-05)。
//! 空分区配额释放给 P6/P7(T2-15)。

pub mod assembly;
pub mod digest;
pub mod history;
pub mod partition;
pub mod retrievers;
pub mod token_counter;

pub use assembly::{AssembledContext, ContextEngine, SystemSection};
pub use partition::{Partition, PartitionConfig};
pub use token_counter::TokenCounter;
