mod output;
mod response;
pub(crate) mod runtime;

use std::io::IsTerminal;
use std::sync::Arc;

use hypr_listener2_core::{BatchErrorCode, BatchEvent};
use tokio::sync::mpsc;

pub use crate::cli::BatchArgs;
use crate::commands::{OutputFormat, SttGlobalArgs};
use crate::config::stt::resolve_config;
use crate::error::{CliError, CliResult};

use self::runtime::ChannelBatchRuntime;

pub async fn run(args: BatchArgs, stt: SttGlobalArgs, quiet: bool) -> CliResult<()> {
    let resolved = resolve_config(
        stt.provider,
        stt.base_url,
        stt.api_key,
        stt.model,
        stt.language,
    )
    .await?;
    let _ = resolved.server.as_ref();

    let file_path = args.input.path().to_str().ok_or_else(|| {
        CliError::invalid_argument(
            "--input",
            args.input.path().display().to_string(),
            "path must be valid utf-8",
        )
    })?;

    let session_id = uuid::Uuid::new_v4().to_string();
    let (batch_tx, mut batch_rx) = mpsc::unbounded_channel::<BatchEvent>();
    let runtime = Arc::new(ChannelBatchRuntime { tx: batch_tx });

    let params = resolved.to_batch_params(session_id, file_path.to_string(), args.keywords);

    let show_progress = !quiet && std::io::stderr().is_terminal();
    let format = args.format;
    let output = args.output;

    let progress = if show_progress {
        crate::output::create_progress_bar(
            "Transcribing",
            "{spinner} {msg} [{bar:20}] {pos:>3}%",
            "█▓░",
        )
    } else {
        None
    };

    let started = std::time::Instant::now();
    let batch_task =
        tokio::spawn(async move { hypr_listener2_core::run_batch(runtime, params).await });

    let mut last_progress_percent: i8 = -1;
    let mut batch_response: Option<owhisper_interface::batch::Response> = None;
    let mut streamed_segments: Vec<owhisper_interface::stream::StreamResponse> = Vec::new();
    let mut failure: Option<(BatchErrorCode, String)> = None;

    while let Some(event) = batch_rx.recv().await {
        match event {
            BatchEvent::BatchStarted { .. } => {
                if let Some(progress) = &progress {
                    progress.set_position(0);
                }
            }
            BatchEvent::BatchCompleted { .. } => {
                if let Some(progress) = &progress {
                    progress.set_position(100);
                }
            }
            BatchEvent::BatchResponseStreamed {
                percentage,
                response: streamed,
                ..
            } => {
                streamed_segments.push(streamed);
                let Some(progress) = &progress else {
                    continue;
                };
                let percent = (percentage * 100.0).round().clamp(0.0, 100.0) as i8;
                if percent == last_progress_percent {
                    continue;
                }

                last_progress_percent = percent;
                progress.set_position(percent as u64);
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
        if let Some(progress) = progress {
            progress.abandon_with_message("Failed");
        }
        let message = if let Some((code, message)) = failure {
            format!("{code:?}: {message}")
        } else {
            error.to_string()
        };
        return Err(CliError::operation_failed("batch transcription", message));
    }

    if let Some(progress) = progress {
        progress.set_position(100);
        progress.finish_and_clear();
    }

    let response = batch_response
        .or_else(|| response::batch_response_from_streams(streamed_segments))
        .ok_or_else(|| {
            CliError::operation_failed("batch transcription", "completed without a final response")
        })?;

    match format {
        OutputFormat::Json => {
            crate::output::write_json(output.as_deref(), &response).await?;
        }
        OutputFormat::Text => {
            let transcript = output::extract_transcript(&response);
            crate::output::write_text(output.as_deref(), transcript).await?;
        }
        OutputFormat::Pretty => {
            let pretty = output::format_pretty(&response);
            crate::output::write_text(output.as_deref(), pretty).await?;
        }
    }

    if !quiet {
        let elapsed = started.elapsed();
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
        if let Some(path) = &output {
            parts.push(format!("-> {}", path.display()));
        }
        eprintln!("\x1b[2m{}\x1b[0m", parts.join(", "));
    }

    Ok(())
}
