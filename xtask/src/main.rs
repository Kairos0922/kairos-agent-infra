//! xtask:项目开发任务运行器。通过 `cargo xtask <task>` 调用。
//!
//! 任务:
//! - `check-docs`:校验全仓 Markdown 的内部链接与标题锚点是否有效
//!   (替代原 `tools/check_doc_links.py`,消除 Python 运行时依赖)。
//! - `install-hooks`:设置 git `core.hooksPath` 指向版本化的 `.githooks/`,
//!   启用 pre-push 快反馈钩子(fmt + clippy)。
//!
//! 退出码:成功 0;失败 1 并打印原因。

use std::process::ExitCode;

mod check_docs;
mod install_hooks;

fn main() -> ExitCode {
    let task = std::env::args().nth(1);
    match task.as_deref() {
        Some("check-docs") => check_docs::run(),
        Some("install-hooks") => install_hooks::run(),
        Some(other) => {
            eprintln!("未知任务:{other}");
            print_usage();
            ExitCode::FAILURE
        }
        None => {
            print_usage();
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    eprintln!("用法:cargo xtask <task>");
    eprintln!("  check-docs      校验 Markdown 内部链接与标题锚点");
    eprintln!("  install-hooks   设置 git core.hooksPath = .githooks(启用 pre-push)");
}
