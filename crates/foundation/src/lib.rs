//! 底座(foundation,L0):所有上层共享的横切关注点。
//!
//! 只放从第一天起任何模块都需要的东西:配置、错误层级、租户上下文(tenancy)、
//! 日志、注册/装配机制。不含任何业务逻辑,保持"薄"。
//!
//! 边界约束(由 Cargo crate 依赖边界强制,ADR 0014/0021):foundation 不依赖任何上层
//! crate(memory / harness / assembly / server / cli)。

pub mod config;
pub mod errors;
pub mod factory;
pub mod logging;
pub mod tenancy;

pub use errors::{ErrorDetails, KairosError};
pub use tenancy::TenantContext;
