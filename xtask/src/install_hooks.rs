//! install-hooks:把仓库版本化的 `.githooks/` 目录设为 git 钩子路径。
//!
//! git 钩子不随仓库版本化(存放在 `.git/hooks`),故用 `core.hooksPath` 指向
//! 仓库内 `.githooks/`——一次设置,pre-push(fmt + clippy 快反馈)即生效。
//! 新 clone 后跑一次 `cargo xtask install-hooks` 即可。

use std::process::{Command, ExitCode};

pub fn run() -> ExitCode {
    match Command::new("git")
        .args(["config", "core.hooksPath", ".githooks"])
        .status()
    {
        Ok(status) if status.success() => {
            println!("已设置 git core.hooksPath = .githooks(.githooks/pre-push 已生效)");
            ExitCode::SUCCESS
        }
        Ok(status) => {
            eprintln!("git config 执行失败:{status}");
            ExitCode::FAILURE
        }
        Err(err) => {
            eprintln!("无法执行 git(是否已安装并在 PATH 中?):{err}");
            ExitCode::FAILURE
        }
    }
}
