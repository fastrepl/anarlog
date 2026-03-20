use std::path::PathBuf;

use clap::CommandFactory;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ToolError(String);

fn char_exe() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("char"))
}

fn cli_help() -> String {
    let mut cmd = crate::cli::Cli::command();
    let mut buf = Vec::new();
    cmd.write_long_help(&mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

#[derive(Deserialize, JsonSchema)]
pub struct CharArgs {
    /// The command and arguments to pass to `char`, e.g. "humans list" or "humans add Alice --email alice@example.com"
    command: String,
}

#[derive(Serialize, Deserialize)]
pub struct Char;

impl Tool for Char {
    const NAME: &'static str = "char";
    type Error = ToolError;
    type Args = CharArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "char".to_string(),
            description: format!("Run a `char` CLI command.\n\n{}", cli_help()),
            parameters: serde_json::to_value(schema_for!(CharArgs)).unwrap(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let parts: Vec<&str> = args.command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(ToolError("empty command".to_string()));
        }

        let output = tokio::process::Command::new(char_exe())
            .args(&parts)
            .output()
            .await
            .map_err(|e| ToolError(format!("failed to run char: {e}")))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            return Err(ToolError(format!(
                "char exited with {}: {}",
                output.status,
                if stderr.is_empty() { &stdout } else { &stderr }
            )));
        }

        Ok(stdout)
    }
}
