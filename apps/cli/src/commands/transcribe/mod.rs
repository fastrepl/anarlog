mod output;
mod response;

use std::sync::Arc;

use hypr_listener2_core::{BatchErrorCode, BatchEvent};
use tokio::sync::mpsc;

use crate::cli::OutputFormat;
use crate::stt::SttProvider;

#[derive(clap::Args)]
pub struct Args {
    #[arg(long, value_name = "FILE", visible_alias = "file")]
    pub input: clio::InputPath,
    #[arg(short = 'p', long, value_enum)]
    pub provider: SttProvider,
    #[arg(long = "keyword", short = 'k', value_name = "KEYWORD")]
    pub keywords: Vec<String>,
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: Option<std::path::PathBuf>,
    #[arg(short = 'f', long, value_enum, default_value = "pretty")]
    pub format: OutputFormat,
}

use crate::error::{CliError, CliResult};
use crate::stt::{ChannelBatchRuntime, SttOverrides, resolve_config};

pub struct BatchResult {
    pub response: owhisper_interface::batch::Response,
    pub file_name: String,
    pub elapsed: std::time::Duration,
}

pub async fn run_batch(input: &clio::InputPath, stt: SttOverrides) -> CliResult<BatchResult> {
    let resolved = resolve_config(None, stt).await?;
    let _ = &resolved.server;

    let file_path = input.path().to_str().ok_or_else(|| {
        CliError::invalid_argument(
            "--input",
            input.path().display().to_string(),
            "path must be valid utf-8",
        )
    })?;

    let session_id = uuid::Uuid::new_v4().to_string();
    let file_name = input
        .path()
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| file_path.to_string());

    let (batch_tx, mut batch_rx) = mpsc::unbounded_channel::<BatchEvent>();
    let runtime = Arc::new(ChannelBatchRuntime { tx: batch_tx });

    let params = resolved.to_batch_params(session_id, file_path.to_string(), vec![]);

    let started = std::time::Instant::now();
    let batch_task =
        tokio::spawn(async move { hypr_listener2_core::run_batch(runtime, params).await });

    let mut batch_response: Option<owhisper_interface::batch::Response> = None;
    let mut streamed_segments: Vec<owhisper_interface::stream::StreamResponse> = Vec::new();
    let mut failure: Option<(BatchErrorCode, String)> = None;

    while let Some(event) = batch_rx.recv().await {
        match event {
            BatchEvent::BatchStarted { .. } => {}
            BatchEvent::BatchCompleted { .. } => {}
            BatchEvent::BatchResponseStreamed {
                response: streamed, ..
            } => {
                streamed_segments.push(streamed);
            }
            BatchEvent::BatchResponse { response: next, .. } => {
                batch_response = Some(next);
            }
            BatchEvent::BatchFailed { code, error, .. } => {
                failure = Some((code, error));
            }
        }
    }

    let result = batch_task
        .await
        .map_err(|e| CliError::operation_failed("batch transcription", e.to_string()))?;
    if let Err(error) = result {
        let message = if let Some((code, message)) = failure {
            format!("{code:?}: {message}")
        } else {
            error.to_string()
        };
        return Err(CliError::operation_failed("batch transcription", message));
    }

    let response = batch_response
        .or_else(|| response::batch_response_from_streams(streamed_segments))
        .ok_or_else(|| {
            CliError::operation_failed("batch transcription", "completed without a final response")
        })?;

    let elapsed = started.elapsed();
    Ok(BatchResult {
        response,
        file_name,
        elapsed,
    })
}

pub async fn run(args: Args, stt: SttOverrides) -> CliResult<()> {
    let format = args.format;
    let output_path = args.output.clone();

    let result = run_batch(&args.input, stt).await?;
    let response = &result.response;

    match format {
        OutputFormat::Json => {
            crate::output::write_json(output_path.as_deref(), &response).await?;
        }
        OutputFormat::Text => {
            let transcript = output::extract_transcript(&response);
            crate::output::write_text(output_path.as_deref(), transcript).await?;
        }
        OutputFormat::Pretty => {
            let pretty = output::format_pretty(&response);
            crate::output::write_text(output_path.as_deref(), pretty).await?;
        }
    }

    let elapsed = result.elapsed;
    let audio_duration = response
        .metadata
        .get("duration")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let mut parts = Vec::new();
    if audio_duration > 0.0 {
        parts.push(format!("{:.1}s audio", audio_duration));
    }
    parts.push(format!("in {:.1}s", elapsed.as_secs_f64()));
    if let Some(path) = &output_path {
        parts.push(format!("-> {}", path.display()));
    }
    use colored::Colorize;
    eprintln!("{}", parts.join(", ").dimmed());

    Ok(())
}
