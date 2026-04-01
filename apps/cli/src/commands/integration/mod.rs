pub mod claude;
pub mod codex;
pub mod opencode;

use serde::Serialize;

use crate::error::{CliError, CliResult};

pub(super) fn read_stdin() -> CliResult<String> {
    std::io::read_to_string(std::io::stdin())
        .map_err(|e| CliError::operation_failed("read stdin", e.to_string()))
}

pub(super) fn read_stdin_json() -> CliResult<serde_json::Value> {
    let input = read_stdin()?;

    serde_json::from_str(&input)
        .map_err(|e| CliError::invalid_argument("stdin", input, e.to_string()))
}

pub(super) fn print_pretty_json(event: &impl Serialize) -> CliResult<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(event)
            .map_err(|e| CliError::operation_failed("serialize", e.to_string()))?
    );
    Ok(())
}
