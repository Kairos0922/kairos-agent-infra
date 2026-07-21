//! 工具模块(tools,L1):工具的定义(Spec)、注册(Registry)、单次调用的执行(Executor)。
//!
//! **职责边界**(见 docs/modules/tools.md §0):
//! - 本模块负责"一次调用怎么正确执行"(校验/超时/取消/异常封装)。
//! - 多个调用的调度(并发/排序)、权限判定与审批路由归 harness/orchestration。
//!
//! `ToolCall`/`ToolDefinition` 归 `model_gateway`(模型协议 DTO,已在
//! `model_gateway::contracts::chat` 定义)。本 crate 只定义 `ToolSpec`(工具固有属性)、
//! `ToolExecuteRequest`(执行入参)与 `ToolResult`(执行产出)。harness 的
//! ToolOrchestrator 负责 `model_gateway::ToolCall` → `ToolExecuteRequest` 的转换。
//!
//! `providers` 为私有 mod(待 builtin/MCP/SkillScript 实现时补),上层 crate 不可见
//! (六层契约三,ADR 0014/0021)。

pub mod contracts;

pub use contracts::{
    CancelToken, DangerLevel, ToolExecuteRequest, ToolExecutor, ToolRegistry, ToolResult,
    ToolSource, ToolSpec, ToolStatus,
};
