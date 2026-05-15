use std::process::{Command, ExitCode};

use crate::paths::repo_root;

pub fn install() -> ExitCode {
    let status = Command::new("git")
        .args(["config", "core.hooksPath", ".githooks"])
        .current_dir(repo_root())
        .status();
    match status {
        Ok(status) if status.success() => {
            println!("Installed git hooks from .githooks.");
            ExitCode::SUCCESS
        }
        Ok(_) => {
            eprintln!("git config core.hooksPath .githooks failed");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("failed to run git config: {e}");
            ExitCode::FAILURE
        }
    }
}
