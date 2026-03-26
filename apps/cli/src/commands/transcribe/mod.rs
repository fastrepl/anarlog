mod output;
mod response;

use std::io::BufWriter;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tokio::sync::mpsc;

use hypr_listener2_core::{BatchErrorCode, BatchEvent};
use owhisper_interface::stream::StreamResponse;

use crate::OptTraceBuffer;
use crate::cli::OutputFormat;
use crate::error::{CliError, CliResult};
use crate::stt::{ChannelBatchRuntime, SttOverrides, resolve_config};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long, value_name = "FILE", visible_alias = "file")]
    pub input: clio::InputPath,
    #[arg(short = 'p', long, value_enum)]
    pub provider: crate::stt::SttProvider,
    #[arg(long = "keyword", short = 'k', value_name = "KEYWORD")]
    pub keywords: Vec<String>,
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: Option<std::path::PathBuf>,
    #[arg(short = 'f', long, value_enum, default_value = "pretty")]
    pub format: OutputFormat,
}

// -- JSONL event types (for --format json) --

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TranscribeEvent {
    Started {
        input: String,
        provider: String,
    },
    Progress {
        percentage: f64,
        transcript: String,
    },
    Completed {
        elapsed_ms: u64,
        audio_duration_secs: Option<f64>,
        response: owhisper_interface::batch::Response,
    },
    Failed {
        code: String,
        message: String,
    },
}

use crate::output::EventWriter;

// -- Batch handle --

struct BatchHandle {
    rx: mpsc::UnboundedReceiver<BatchEvent>,
    task: tokio::task::JoinHandle<
        Result<hypr_listener2_core::BatchRunOutput, hypr_listener2_core::Error>,
    >,
    started: std::time::Instant,
    _server: crate::stt::ServerGuard,
}

async fn start_batch(input: &clio::InputPath, stt: SttOverrides) -> CliResult<BatchHandle> {
    let resolved = resolve_config(None, stt).await?;

    let file_path = input.path().to_str().ok_or_else(|| {
        CliError::invalid_argument(
            "--input",
            input.path().display().to_string(),
            "path must be valid utf-8",
        )
    })?;

    let session_id = uuid::Uuid::new_v4().to_string();

    let (batch_tx, batch_rx) = mpsc::unbounded_channel::<BatchEvent>();
    let runtime = Arc::new(ChannelBatchRuntime { tx: batch_tx });
    let params = resolved.to_batch_params(session_id, file_path.to_string(), vec![]);

    let started = std::time::Instant::now();
    let task = tokio::spawn(async move { hypr_listener2_core::run_batch(runtime, params).await });

    Ok(BatchHandle {
        rx: batch_rx,
        task,
        started,
        _server: resolved.server,
    })
}

fn extract_stream_transcript(response: &StreamResponse) -> Option<&str> {
    match response {
        StreamResponse::TranscriptResponse { channel, .. } => {
            channel.alternatives.first().map(|a| a.transcript.as_str())
        }
        _ => None,
    }
}

struct CollectedBatch {
    response: owhisper_interface::batch::Response,
    elapsed: Duration,
}

fn finish_batch(
    task_result: Result<
        Result<hypr_listener2_core::BatchRunOutput, hypr_listener2_core::Error>,
        tokio::task::JoinError,
    >,
    batch_response: Option<owhisper_interface::batch::Response>,
    streamed_segments: Vec<StreamResponse>,
    failure: Option<(BatchErrorCode, String)>,
    started: std::time::Instant,
) -> CliResult<CollectedBatch> {
    let result = task_result
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

    Ok(CollectedBatch {
        response,
        elapsed: started.elapsed(),
    })
}

// -- Entry point --

pub async fn run(args: Args, stt: SttOverrides, trace_buffer: OptTraceBuffer) -> CliResult<()> {
    let format = args.format;
    let output_path = args.output.clone();
    let input_display = args.input.path().display().to_string();
    let provider_display = format!("{:?}", args.provider).to_lowercase();

    match format {
        OutputFormat::Json => {
            run_json(args, stt, output_path, input_display, provider_display).await
        }
        OutputFormat::Pretty => run_pretty(args, stt, output_path, trace_buffer).await,
    }
}

// -- JSON mode --

async fn run_json(
    args: Args,
    stt: SttOverrides,
    output_path: Option<std::path::PathBuf>,
    input_display: String,
    provider_display: String,
) -> CliResult<()> {
    let mut writer = EventWriter::new(BufWriter::new(std::io::stdout()));

    writer.emit(&TranscribeEvent::Started {
        input: input_display,
        provider: provider_display,
    })?;

    let mut handle = match start_batch(&args.input, stt).await {
        Ok(h) => h,
        Err(e) => {
            let _ = writer.emit(&TranscribeEvent::Failed {
                code: "error".to_string(),
                message: e.to_string(),
            });
            return Err(e);
        }
    };

    let mut batch_response: Option<owhisper_interface::batch::Response> = None;
    let mut streamed_segments: Vec<StreamResponse> = Vec::new();
    let mut failure: Option<(BatchErrorCode, String)> = None;

    while let Some(event) = handle.rx.recv().await {
        match event {
            BatchEvent::BatchStarted { .. } | BatchEvent::BatchCompleted { .. } => {}
            BatchEvent::BatchResponseStreamed {
                response: streamed,
                percentage,
                ..
            } => {
                let transcript = extract_stream_transcript(&streamed)
                    .unwrap_or("")
                    .to_string();
                writer.emit(&TranscribeEvent::Progress {
                    percentage,
                    transcript,
                })?;
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

    let result = match finish_batch(
        handle.task.await,
        batch_response,
        streamed_segments,
        failure,
        handle.started,
    ) {
        Ok(r) => r,
        Err(e) => {
            let _ = writer.emit(&TranscribeEvent::Failed {
                code: "error".to_string(),
                message: e.to_string(),
            });
            return Err(e);
        }
    };

    let audio_duration_secs = result
        .response
        .metadata
        .get("duration")
        .and_then(|v| v.as_f64());

    writer.emit(&TranscribeEvent::Completed {
        elapsed_ms: result.elapsed.as_millis() as u64,
        audio_duration_secs,
        response: result.response.clone(),
    })?;

    if let Some(path) = &output_path {
        crate::output::write_json(Some(path.as_path()), &result.response).await?;
    }

    Ok(())
}

// -- Pretty mode --

async fn run_pretty(
    args: Args,
    stt: SttOverrides,
    output_path: Option<std::path::PathBuf>,
    _trace_buffer: OptTraceBuffer,
) -> CliResult<()> {
    #[cfg(feature = "standalone")]
    let file_name = args
        .input
        .path()
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| args.input.path().display().to_string());

    let mut handle = start_batch(&args.input, stt).await?;

    #[cfg(feature = "standalone")]
    let mut viewport = if _trace_buffer.is_some() {
        crate::tui::InlineViewport::stderr(5, _trace_buffer).ok()
    } else {
        None
    };

    let mut batch_response: Option<owhisper_interface::batch::Response> = None;
    let mut streamed_segments: Vec<StreamResponse> = Vec::new();
    let mut failure: Option<(BatchErrorCode, String)> = None;

    #[cfg(feature = "standalone")]
    let (mut spinner_idx, mut last_pct, mut last_transcript, mut tick) = (
        0usize,
        0.0f64,
        String::new(),
        tokio::time::interval(Duration::from_millis(80)),
    );

    loop {
        #[cfg(feature = "standalone")]
        let event = if viewport.is_some() {
            tokio::select! {
                ev = handle.rx.recv() => ev,
                _ = tick.tick() => {
                    spinner_idx = (spinner_idx + 1) % crate::tui::SPINNER.len();
                    if let Some(ref mut vp) = viewport {
                        vp.poll_toggle();
                        let pct_str = format!("{:.0}%", last_pct * 100.0);
                        vp.draw(&[
                            format!("{} Transcribing {}... {}", crate::tui::SPINNER[spinner_idx], file_name, pct_str),
                            format!("  {}", crate::tui::truncate_line(&last_transcript, 76)),
                        ]);
                    }
                    continue;
                }
            }
        } else {
            handle.rx.recv().await
        };

        #[cfg(not(feature = "standalone"))]
        let event = handle.rx.recv().await;

        let Some(event) = event else { break };

        match event {
            BatchEvent::BatchStarted { .. } | BatchEvent::BatchCompleted { .. } => {}
            BatchEvent::BatchResponseStreamed {
                response: streamed,
                #[cfg(feature = "standalone")]
                percentage,
                ..
            } => {
                #[cfg(feature = "standalone")]
                {
                    last_pct = percentage;
                    if let Some(t) = extract_stream_transcript(&streamed) {
                        if !t.is_empty() {
                            last_transcript = t.to_string();
                        }
                    }
                }
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

    #[cfg(feature = "standalone")]
    if let Some(ref mut vp) = viewport {
        vp.clear()
            .map_err(|e| CliError::operation_failed("clear viewport", e.to_string()))?;
    }

    let result = finish_batch(
        handle.task.await,
        batch_response,
        streamed_segments,
        failure,
        handle.started,
    )?;
    let response = &result.response;

    let pretty = output::format_pretty(response);
    crate::output::write_text(output_path.as_deref(), pretty).await?;

    let audio_duration = response
        .metadata
        .get("duration")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let mut parts = Vec::new();
    if audio_duration > 0.0 {
        parts.push(format!("{:.1}s audio", audio_duration));
    }
    parts.push(format!("in {:.1}s", result.elapsed.as_secs_f64()));
    if let Some(path) = &output_path {
        parts.push(format!("-> {}", path.display()));
    }
    use colored::Colorize;
    eprintln!("{}", parts.join(", ").dimmed());

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn event_writer_serializes_jsonl_lines() {
        let mut bytes = Cursor::new(Vec::new());
        let mut writer = EventWriter::new(&mut bytes);

        writer
            .emit(&TranscribeEvent::Started {
                input: "test.wav".to_string(),
                provider: "deepgram".to_string(),
            })
            .unwrap();

        let output = String::from_utf8(bytes.into_inner()).unwrap();
        assert!(output.ends_with('\n'));
        assert!(output.contains("\"type\":\"started\""));
        assert!(output.contains("\"input\":\"test.wav\""));
    }
}
