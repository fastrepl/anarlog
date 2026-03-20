mod backend;
mod tools;

pub use backend::{Backend, StreamEvent};

use crate::error::{CliError, CliResult};
use crate::llm::ResolvedLlmConfig;

pub async fn run_prompt(
    config: ResolvedLlmConfig,
    system_message: Option<String>,
    prompt: &str,
) -> CliResult<()> {
    use std::io::Write;

    let text = if prompt == "-" {
        std::io::read_to_string(std::io::stdin())
            .map_err(|e| CliError::operation_failed("read stdin", e.to_string()))?
    } else {
        prompt.to_string()
    };
    let backend = Backend::new(config, system_message)?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    backend
        .stream_text(text, Vec::new(), 5, |event| {
            if let StreamEvent::TextChunk(chunk) = event {
                out.write_all(chunk.as_bytes())
                    .map_err(|e| CliError::operation_failed("write stdout", e.to_string()))?;
                out.flush()
                    .map_err(|e| CliError::operation_failed("flush stdout", e.to_string()))?;
            }
            Ok(())
        })
        .await?;

    writeln!(out).map_err(|e| CliError::operation_failed("write stdout", e.to_string()))?;

    Ok(())
}
