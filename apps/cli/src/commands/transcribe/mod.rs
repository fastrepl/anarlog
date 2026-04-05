mod output;

use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tokio::sync::mpsc;

use hypr_listener2_core::{BatchErrorCode, BatchEvent};
use owhisper_interface::batch_stream::BatchStreamEvent;

use crate::OptTraceBuffer;
use crate::app::AppContext;
use crate::cli::OutputFormat;
use crate::error::{CliError, CliResult};
use crate::stt::{ChannelBatchRuntime, SttOverrides, resolve_config};

#[derive(clap::Args)]
pub struct Args {
    /// Timestamp (e.g. 20260327_143022) or path to an audio file
    #[arg(value_name = "INPUT")]
    pub target: Option<String>,
    #[arg(short = 'i', long, value_name = "FILE", visible_alias = "file")]
    pub input: Option<String>,
    #[arg(short = 'p', long, value_enum)]
    pub provider: crate::stt::SttProvider,
    #[arg(long = "keyword", short = 'k', value_name = "KEYWORD")]
    pub keywords: Vec<String>,
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: Option<std::path::PathBuf>,
    #[arg(short = 'f', long, value_enum, default_value = "pretty")]
    pub format: OutputFormat,
    #[arg(long, env = "CHAR_BASE", hide_env_values = true, value_name = "DIR")]
    pub base: Option<std::path::PathBuf>,
    #[arg(long, env = "CHAR_BASE_URL", hide_env_values = true, value_parser = crate::cli::parse_base_url)]
    pub base_url: Option<String>,
    #[arg(long, env = "CHAR_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,
    #[arg(short = 'm', long, env = "CHAR_MODEL", hide_env_values = true)]
    pub model: Option<String>,
    #[arg(
        short = 'l',
        long,
        env = "CHAR_LANGUAGE",
        hide_env_values = true,
        default_value = "en"
    )]
    pub language: String,
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
    _normalized_input_dir: Option<tempfile::TempDir>,
    _server: crate::stt::ServerGuard,
}

async fn start_batch(
    input: &Path,
    keywords: Vec<String>,
    stt: SttOverrides,
    audio_dest: Option<&Path>,
    on_normalize_progress: Option<&mut dyn FnMut(f64)>,
) -> CliResult<BatchHandle> {
    let resolved = resolve_config(None, stt).await?;
    let (temp_dir, normalized_input_path) =
        normalize_input_file(input, audio_dest, on_normalize_progress)?;
    let params = build_batch_params(&resolved, &normalized_input_path, keywords)?;

    let (batch_tx, batch_rx) = mpsc::unbounded_channel::<BatchEvent>();
    let runtime = Arc::new(ChannelBatchRuntime { tx: batch_tx });

    let started = std::time::Instant::now();
    let task = tokio::spawn(async move { hypr_listener2_core::run_batch(runtime, params).await });

    Ok(BatchHandle {
        rx: batch_rx,
        task,
        started,
        _normalized_input_dir: temp_dir,
        _server: resolved.server,
    })
}

fn normalize_input_file(
    input: &Path,
    dest: Option<&Path>,
    on_progress: Option<&mut dyn FnMut(f64)>,
) -> CliResult<(Option<tempfile::TempDir>, PathBuf)> {
    let (temp_dir, tmp_path, target_path) = if let Some(dest) = dest {
        let tmp_path = dest.with_extension("mp3.tmp");
        (None, tmp_path, dest.to_path_buf())
    } else {
        let td = tempfile::Builder::new()
            .prefix("char-transcribe-")
            .tempdir()
            .map_err(|e| {
                CliError::operation_failed("create normalization tempdir", e.to_string())
            })?;
        let tmp_path = td.path().join("input.mp3.tmp");
        let target_path = td.path().join("input.mp3");
        (Some(td), tmp_path, target_path)
    };

    hypr_audio_norm::normalize_file(input, &tmp_path, &target_path, None, on_progress)
        .map_err(|e| CliError::operation_failed("normalize audio input", e.to_string()))?;

    Ok((temp_dir, target_path))
}

fn build_batch_params(
    resolved: &crate::stt::ResolvedSttConfig,
    input: &std::path::Path,
    keywords: Vec<String>,
) -> CliResult<hypr_listener2_core::BatchParams> {
    let file_path = input.to_str().ok_or_else(|| {
        CliError::invalid_argument(
            "--input",
            input.display().to_string(),
            "path must be valid utf-8",
        )
    })?;

    Ok(resolved.to_batch_params(
        uuid::Uuid::new_v4().to_string(),
        file_path.to_string(),
        keywords,
    ))
}

fn input_timestamp(path: &Path) -> String {
    let system_time = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .unwrap_or_else(|_| std::time::SystemTime::now());
    format_timestamp(system_time)
}

fn format_timestamp(t: std::time::SystemTime) -> String {
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hour = time_of_day / 3600;
    let min = (time_of_day % 3600) / 60;
    let sec = time_of_day % 60;

    // days since epoch to y/m/d (civil calendar)
    let (y, m, d) = days_to_ymd(days as i64);
    format!("{y:04}{m:02}{d:02}_{hour:02}{min:02}{sec:02}")
}

fn days_to_ymd(days: i64) -> (i64, u32, u32) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn extract_stream_transcript(event: &BatchStreamEvent) -> Option<&str> {
    event.text()
}

struct CollectedBatch {
    response: owhisper_interface::batch::Response,
    elapsed: Duration,
}

struct OutputTarget {
    output_path: PathBuf,
    audio_dest: Option<PathBuf>,
}

fn finish_batch(
    task_result: Result<
        Result<hypr_listener2_core::BatchRunOutput, hypr_listener2_core::Error>,
        tokio::task::JoinError,
    >,
    failure: Option<(BatchErrorCode, String)>,
    started: std::time::Instant,
) -> CliResult<CollectedBatch> {
    let result = task_result
        .map_err(|e| CliError::operation_failed("batch transcription", e.to_string()))?;
    let output = if let Ok(output) = result {
        output
    } else {
        let error = result.err().unwrap();
        let message = if let Some((code, message)) = failure {
            format!("{code:?}: {message}")
        } else {
            error.to_string()
        };
        return Err(CliError::operation_failed("batch transcription", message));
    };

    Ok(CollectedBatch {
        response: output.response,
        elapsed: started.elapsed(),
    })
}

fn resolve_input(
    target: Option<&str>,
    input: Option<&str>,
    base: Option<&Path>,
) -> CliResult<PathBuf> {
    let raw = match (target, input) {
        (Some(t), _) => t,
        (None, Some(i)) => i,
        (None, None) => {
            return Err(CliError::invalid_argument(
                "INPUT",
                "(missing)",
                "provide an audio file or session timestamp",
            ));
        }
    };

    let as_path = PathBuf::from(raw);
    if as_path.is_file() {
        return Ok(as_path);
    }

    let base_dir = base.map(Path::to_path_buf).unwrap_or_else(|| {
        dirs::data_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("char")
    });

    let session_audio = base_dir.join(raw).join("audio.mp3");
    if session_audio.is_file() {
        return Ok(session_audio);
    }

    Err(CliError::not_found(
        format!("audio file for '{raw}'"),
        Some(format!(
            "Pass a file path or a session timestamp.\nLooked in: {}",
            session_audio.display()
        )),
    ))
}

fn resolve_output_target(
    input_path: &Path,
    output: Option<PathBuf>,
    base: Option<&Path>,
) -> CliResult<OutputTarget> {
    if let Some(output_path) = output {
        return Ok(OutputTarget {
            output_path,
            audio_dest: None,
        });
    }

    let ts = input_timestamp(input_path);
    let dir = super::resolve_session_dir(base, &ts)?;

    Ok(OutputTarget {
        output_path: dir.join("transcript.json"),
        audio_dest: Some(dir.join("audio.mp3")),
    })
}

fn saves_json(output_path: &Path) -> bool {
    output_path.file_name() == Some(std::ffi::OsStr::new("transcript.json"))
}

async fn save_pretty_output(
    output_path: Option<&Path>,
    pretty: &str,
    response: &owhisper_interface::batch::Response,
) -> CliResult<()> {
    let Some(output_path) = output_path else {
        return Ok(());
    };

    if saves_json(output_path) {
        crate::output::write_json(Some(output_path), response).await
    } else {
        crate::output::write_text(Some(output_path), pretty.to_string()).await
    }
}

// -- Entry point --

#[allow(clippy::unit_arg)]
pub async fn run(ctx: &AppContext, args: Args) -> CliResult<()> {
    let input_path = resolve_input(
        args.target.as_deref(),
        args.input.as_deref(),
        args.base.as_deref(),
    )?;
    let format = args.format;
    let output_target =
        resolve_output_target(&input_path, args.output.clone(), args.base.as_deref())?;
    let output_path = Some(output_target.output_path.clone());
    let input_display = input_path.display().to_string();
    let provider_display = format!("{:?}", args.provider).to_lowercase();
    let stt = ctx.stt_overrides(
        Some(args.provider),
        args.base_url.clone(),
        args.api_key.clone(),
        args.model.clone(),
        args.language.clone(),
    );

    match format {
        OutputFormat::Json => {
            run_json(
                &input_path,
                args,
                stt,
                output_path,
                output_target.audio_dest,
                input_display,
                provider_display,
            )
            .await
        }
        OutputFormat::Pretty => {
            run_pretty(
                &input_path,
                args,
                stt,
                output_path,
                output_target.audio_dest,
                ctx.trace_buffer(),
            )
            .await
        }
    }
}

// -- JSON mode --

async fn run_json(
    input_path: &Path,
    args: Args,
    stt: SttOverrides,
    output_path: Option<std::path::PathBuf>,
    audio_dest: Option<PathBuf>,
    input_display: String,
    provider_display: String,
) -> CliResult<()> {
    let mut writer = EventWriter::new(BufWriter::new(std::io::stdout()));

    writer.emit(&TranscribeEvent::Started {
        input: input_display,
        provider: provider_display,
    })?;

    let mut handle = match start_batch(
        input_path,
        args.keywords.clone(),
        stt,
        audio_dest.as_deref(),
        None,
    )
    .await
    {
        Ok(h) => h,
        Err(e) => {
            let _ = writer.emit(&TranscribeEvent::Failed {
                code: "error".to_string(),
                message: e.to_string(),
            });
            return Err(e);
        }
    };

    let mut failure: Option<(BatchErrorCode, String)> = None;

    while let Some(event) = handle.rx.recv().await {
        match event {
            BatchEvent::BatchStarted { .. } | BatchEvent::BatchCompleted { .. } => {}
            BatchEvent::BatchResponseStreamed {
                event: streamed, ..
            } => {
                let transcript = extract_stream_transcript(&streamed)
                    .unwrap_or("")
                    .to_string();
                writer.emit(&TranscribeEvent::Progress {
                    percentage: streamed.percentage(),
                    transcript,
                })?;
            }
            BatchEvent::BatchResponse { .. } => {}
            BatchEvent::BatchFailed { code, error, .. } => {
                failure = Some((code, error));
            }
        }
    }

    let result = match finish_batch(handle.task.await, failure, handle.started) {
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
    input_path: &Path,
    args: Args,
    stt: SttOverrides,
    output_path: Option<std::path::PathBuf>,
    audio_dest: Option<PathBuf>,
    _trace_buffer: OptTraceBuffer,
) -> CliResult<()> {
    let update_check = super::update_check::UpdateHandle::spawn();

    #[cfg(feature = "standalone")]
    let file_name = input_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| input_path.display().to_string());

    #[cfg(feature = "standalone")]
    let mut viewport = if _trace_buffer.is_some() {
        crate::tui::InlineViewport::stderr(5, _trace_buffer).ok()
    } else {
        None
    };

    #[cfg(feature = "standalone")]
    let mut normalize_spinner_idx = 0usize;
    #[cfg(feature = "standalone")]
    let mut normalize_progress = |percentage: f64| {
        normalize_spinner_idx = (normalize_spinner_idx + 1) % crate::tui::SPINNER.len();
        draw_normalization(
            &mut viewport,
            &file_name,
            normalize_spinner_idx,
            percentage.clamp(0.0, 1.0),
        );
    };

    #[cfg(feature = "standalone")]
    let mut handle = match start_batch(
        input_path,
        args.keywords.clone(),
        stt,
        audio_dest.as_deref(),
        Some(&mut normalize_progress),
    )
    .await
    {
        Ok(handle) => handle,
        Err(error) => {
            if let Some(ref mut vp) = viewport {
                let _ = vp.clear();
            }
            return Err(error);
        }
    };

    #[cfg(not(feature = "standalone"))]
    let mut handle = start_batch(
        input_path,
        args.keywords.clone(),
        stt,
        audio_dest.as_deref(),
        None,
    )
    .await?;

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
                        vp.poll_input();
                        let pct_str = format!("{:.0}%", last_pct * 100.0);
                        vp.draw_strings(&[
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
                event: streamed, ..
            } => {
                #[cfg(feature = "standalone")]
                {
                    last_pct = streamed.percentage();
                    if let Some(t) = extract_stream_transcript(&streamed)
                        && !t.is_empty()
                    {
                        last_transcript = t.to_string();
                    }
                }
                #[cfg(not(feature = "standalone"))]
                {
                    let _ = streamed;
                }
            }
            BatchEvent::BatchResponse { .. } => {}
            BatchEvent::BatchFailed { code, error, .. } => {
                failure = Some((code, error));
            }
        }
    }

    let result = finish_batch(handle.task.await, failure, handle.started)?;
    let response = &result.response;
    let pretty = output::format_pretty(response);

    crate::output::write_text(None, pretty.clone()).await?;
    save_pretty_output(output_path.as_deref(), &pretty, response).await?;

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

    #[cfg(feature = "standalone")]
    if let Some(ref mut vp) = viewport {
        let mut lines = vec![format!("saved  {}", parts.join("  "))];
        if let Some(path) = &output_path {
            lines.push(format!("{}", path.display()));
        }
        vp.draw_strings(&lines);
        vp.finish()
            .map_err(|e| CliError::operation_failed("finish viewport", e.to_string()))?;
    } else {
        if let Some(path) = &output_path {
            parts.push(format!("-> {}", path.display()));
        }
        use colored::Colorize;
        eprintln!("{}", parts.join(", ").dimmed());
    }

    #[cfg(not(feature = "standalone"))]
    {
        if let Some(path) = &output_path {
            parts.push(format!("-> {}", path.display()));
        }
        use colored::Colorize;
        eprintln!("{}", parts.join(", ").dimmed());
    }

    if let Some(version) = update_check.result().await {
        eprintln!("  update available ({version}): char update");
    }

    Ok(())
}

#[cfg(feature = "standalone")]
fn draw_normalization(
    viewport: &mut Option<crate::tui::InlineViewport>,
    file_name: &str,
    spinner_idx: usize,
    percentage: f64,
) {
    if let Some(vp) = viewport {
        vp.poll_input();
        let pct = (percentage * 100.0).round().clamp(0.0, 100.0) as u8;
        vp.draw_strings(&[
            format!(
                "{} Normalizing {}... {}%",
                crate::tui::SPINNER[spinner_idx],
                file_name,
                pct
            ),
            format_gauge(pct),
        ]);
    }
}

#[cfg(feature = "standalone")]
fn format_gauge(pct: u8) -> String {
    let width = 40;
    let filled = (pct as usize * width) / 100;
    let empty = width - filled;
    format!("  [{}{}]", "█".repeat(filled), "░".repeat(empty))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::path::Path;

    use owhisper_interface::batch;

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

    #[test]
    fn build_batch_params_preserves_keywords() {
        let resolved = crate::stt::ResolvedSttConfig {
            provider: hypr_listener2_core::BatchProvider::Deepgram,
            base_url: "https://example.com".to_string(),
            api_key: "secret".to_string(),
            model: "nova".to_string(),
            language: "en".parse().unwrap(),
            server: crate::stt::ServerGuard::default(),
        };

        let params = build_batch_params(
            &resolved,
            Path::new("/tmp/example.mp3"),
            vec!["roadmap".to_string(), "planning".to_string()],
        )
        .unwrap();

        assert_eq!(params.keywords, vec!["roadmap", "planning"]);
    }

    #[test]
    fn resolve_output_target_uses_json_session_artifact_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input.mp3");
        std::fs::write(&input, b"audio").unwrap();

        let target = resolve_output_target(&input, None, Some(dir.path())).unwrap();

        assert_eq!(target.output_path.file_name().unwrap(), "transcript.json");
        assert_eq!(
            target.audio_dest.as_ref().and_then(|path| path.file_name()),
            Some(std::ffi::OsStr::new("audio.mp3"))
        );
    }

    #[test]
    fn saves_json_only_for_transcript_json() {
        assert!(saves_json(Path::new("/tmp/transcript.json")));
        assert!(!saves_json(Path::new("/tmp/output.txt")));
    }

    #[tokio::test]
    async fn save_pretty_output_persists_json_response_for_transcript_json() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("transcript.json");
        let response = batch::Response {
            metadata: serde_json::json!({ "duration": 1.2 }),
            results: batch::Results {
                channels: vec![batch::Channel {
                    alternatives: vec![batch::Alternatives {
                        transcript: "hello world".to_string(),
                        confidence: 1.0,
                        words: vec![batch::Word {
                            word: "hello".to_string(),
                            start: 0.0,
                            end: 0.5,
                            confidence: 1.0,
                            channel: 0,
                            speaker: None,
                            punctuated_word: Some("hello".to_string()),
                        }],
                    }],
                }],
            },
        };

        save_pretty_output(Some(&out), "pretty text", &response)
            .await
            .unwrap();

        let saved = std::fs::read_to_string(out).unwrap();
        let parsed: batch::Response = serde_json::from_str(&saved).unwrap();

        assert_eq!(
            parsed.results.channels[0].alternatives[0].transcript,
            "hello world"
        );
        assert!(saved.starts_with('{'));
    }

    #[tokio::test]
    async fn save_pretty_output_writes_pretty_text_for_non_json_path() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("transcript.txt");
        let response = batch::Response {
            metadata: serde_json::json!({}),
            results: batch::Results {
                channels: Vec::new(),
            },
        };

        save_pretty_output(Some(&out), "pretty text", &response)
            .await
            .unwrap();

        assert_eq!(std::fs::read_to_string(out).unwrap(), "pretty text\n");
    }
}
