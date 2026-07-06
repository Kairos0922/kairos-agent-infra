//! 结构化日志:基于 `tracing` 的统一初始化。
//!
//! 全项目用 `tracing` 宏记结构化字段:
//!
//! ```no_run
//! use tracing::info;
//! info!(kind = "semantic", method = "hybrid", n_candidates = 42, latency_ms = 18.3, "recall");
//! ```
//!
//! 安全红线(合规 strict 档,见 docs/project/architecture.md §3):
//! - **绝不记录记忆/知识内容明文与密钥**,只记元数据(数量、耗时、kind)。
//! - 涉及内容只记 id 或哈希前缀。
//!
//! 本模块保持薄:装配一个输出单行 JSON 到 stderr 的 subscriber;不引额外结构化日志依赖。

use tracing_subscriber::{fmt, EnvFilter};

/// 初始化全局日志 subscriber:单行 JSON 输出到 stderr,级别由 `level` 控制。
///
/// 幂等安全:重复调用不会 panic(内部用 `try_init`,已初始化则静默跳过),便于测试与多入口。
///
/// # 参数
/// - `level`:日志级别名(来自 `KairosSettings.log_level`),如 "INFO" / "DEBUG"。
///   兼容 `RUST_LOG` 环境变量覆盖(EnvFilter 优先读环境)。
pub fn init_logging(level: &str) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level.to_lowercase()));
    let _ = fmt()
        .json()
        .with_writer(std::io::stderr)
        .with_current_span(false)
        .with_env_filter(filter)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_is_idempotent() {
        // 重复调用不 panic(try_init 已初始化则跳过)。
        init_logging("info");
        init_logging("debug");
    }
}
