//! xtask:项目开发任务运行器。通过 `cargo xtask <task>` 调用。
//!
//! 任务:
//! - `check-docs`:校验全仓 Markdown 的内部链接与标题锚点是否有效
//!   (替代原 `tools/check_doc_links.py`,消除 Python 运行时依赖)。
//!
//! 退出码:全部有效 0;存在断链/失效锚点 1 并逐条打印。

use std::process::ExitCode;

mod check_docs;

fn main() -> ExitCode {
    let task = std::env::args().nth(1);
    match task.as_deref() {
        Some("check-docs") => check_docs::run(),
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
    eprintln!("  check-docs   校验 Markdown 内部链接与标题锚点");
}
