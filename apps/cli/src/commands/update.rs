use std::process::Command;

use crate::error::{CliError, CliResult};

pub fn run() -> CliResult<()> {
    let status = Command::new("npm")
        .args(["i", "-g", "char@latest"])
        .status()
        .map_err(|e| CliError::operation_failed("run npm", e.to_string()))?;

    if !status.success() {
        return Err(CliError::operation_failed(
            "update char",
            "npm install failed",
        ));
    }

    Ok(())
}
