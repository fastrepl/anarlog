use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::ValueEnum;
use hypr_listener2_core::{BatchErrorCode, BatchEvent, BatchParams, BatchProvider};
use indicatif::{ProgressBar, ProgressStyle};
use tokio::sync::mpsc;

use crate::commands::OutputFormat;
use crate::commands::cactus_server::resolve_and_spawn_cactus;
use crate::error::{CliError, CliResult};

mod runtime;

use runtime::BatchEventRuntime;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Provider {
    Argmax,
    Deepgram,
    Soniox,
    Assemblyai,
    Fireworks,
    Openai,
    Gladia,
    Elevenlabs,
    Dashscope,
    Mistral,
    Am,
    Cactus,
}

impl From<Provider> for BatchProvider {
    fn from(value: Provider) -> Self {
        match value {
            Provider::Argmax => BatchProvider::Argmax,
            Provider::Deepgram => BatchProvider::Deepgram,
            Provider::Soniox => BatchProvider::Soniox,
            Provider::Assemblyai => BatchProvider::AssemblyAI,
            Provider::Fireworks => BatchProvider::Fireworks,
            Provider::Openai => BatchProvider::OpenAI,
            Provider::Gladia => BatchProvider::Gladia,
            Provider::Elevenlabs => BatchProvider::ElevenLabs,
            Provider::Dashscope => BatchProvider::DashScope,
            Provider::Mistral => BatchProvider::Mistral,
            Provider::Am => BatchProvider::Am,
            Provider::Cactus => BatchProvider::Cactus,
        }
    }
}

pub struct Args {
    pub input: PathBuf,
    pub provider: Provider,
    pub base_url: Option<String>,
    pub api_key: String,
    pub model: Option<String>,
    pub language: String,
    pub keywords: Vec<String>,
    pub output: Option<PathBuf>,
    pub format: OutputFormat,
    pub quiet: bool,
}

pub async fn run(args: Args) -> CliResult<()> {
    validate_input_path(&args.input)?;

    let languages = vec![
        args.language
            .parse::<hypr_language::Language>()
            .map_err(|e| {
                CliError::invalid_argument("--language", args.language.clone(), e.to_string())
            })?,
    ];

    let _server;
    let base_url = if matches!(args.provider, Provider::Cactus) {
        let (server, url) = resolve_and_spawn_cactus(args.model.as_deref()).await?;
        _server = Some(server);
        url
    } else {
        _server = None;
        args.base_url
            .ok_or_else(|| CliError::required_argument("--base-url (or CHAR_BASE_URL)"))?
    };

    let session_id = uuid::Uuid::new_v4().to_string();
    let (batch_tx, mut batch_rx) = mpsc::unbounded_channel::<BatchEvent>();
    let runtime = Arc::new(BatchEventRuntime { tx: batch_tx });

    let file_path = args.input.to_str().ok_or_else(|| {
        CliError::invalid_argument(
            "--input",
            args.input.display().to_string(),
            "path must be valid utf-8",
        )
    })?;

    let params = BatchParams {
        session_id,
        provider: args.provider.into(),
        file_path: file_path.to_string(),
        model: args.model,
        base_url,
        api_key: args.api_key,
        languages,
        keywords: args.keywords,
    };

    let quiet = args.quiet;
    let show_progress = !quiet && std::io::stderr().is_terminal();
    let format = args.format;
    let output = args.output;

    let progress = if show_progress {
        let bar = ProgressBar::new(100);
        bar.set_style(
            ProgressStyle::with_template("{spinner} {msg} [{bar:20}] {pos:>3}%")
                .unwrap()
                .progress_chars("█▓░"),
        );
        bar.set_message("Transcribing");
        bar.enable_steady_tick(std::time::Duration::from_millis(120));
        Some(bar)
    } else {
        None
    };

    let started = std::time::Instant::now();
    let batch_task =
        tokio::spawn(async move { hypr_listener2_core::run_batch(runtime, params).await });

    let mut last_progress_percent: i8 = -1;
    let mut response: Option<owhisper_interface::batch::Response> = None;
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
                response = Some(next);
            }
            BatchEvent::BatchFailed { code, error, .. } => {
                failure = Some((code, error));
            }
        }
    }

    let result = batch_task
        .await
        .map_err(|e| CliError::external_action_failed("batch transcription", e.to_string()))?;
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

    let response = response
        .or_else(|| batch_response_from_streams(streamed_segments))
        .ok_or_else(|| {
            CliError::operation_failed("batch transcription", "completed without a final response")
        })?;

    match format {
        OutputFormat::Json => {
            write_json_response(output.as_deref(), &response).await?;
        }
        OutputFormat::Text => {
            let transcript = extract_transcript(&response);
            write_text_response(output.as_deref(), transcript).await?;
        }
        OutputFormat::Pretty => {
            let pretty = format_pretty(&response);
            write_text_response(output.as_deref(), pretty).await?;
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

fn batch_response_from_streams(
    segments: Vec<owhisper_interface::stream::StreamResponse>,
) -> Option<owhisper_interface::batch::Response> {
    use owhisper_interface::batch;
    use owhisper_interface::stream::StreamResponse;

    if segments.is_empty() {
        return None;
    }

    let mut all_words: Vec<batch::Word> = Vec::new();
    let mut all_transcripts: Vec<String> = Vec::new();
    let mut total_confidence = 0.0;
    let mut max_end = 0.0_f64;
    let mut count = 0usize;

    for segment in segments {
        let StreamResponse::TranscriptResponse {
            channel,
            start,
            duration,
            ..
        } = segment
        else {
            continue;
        };

        let Some(alt) = channel.alternatives.into_iter().next() else {
            continue;
        };

        let text = alt.transcript.trim().to_string();
        if text.is_empty() {
            continue;
        }

        let words: Vec<batch::Word> = alt.words.into_iter().map(batch::Word::from).collect();
        all_words.extend(words);
        all_transcripts.push(text);
        total_confidence += alt.confidence;
        max_end = max_end.max(start + duration);
        count += 1;
    }

    if count == 0 {
        return None;
    }

    let transcript = all_transcripts.join(" ");
    let avg_confidence = total_confidence / count as f64;

    Some(batch::Response {
        metadata: serde_json::json!({ "duration": max_end }),
        results: batch::Results {
            channels: vec![batch::Channel {
                alternatives: vec![batch::Alternatives {
                    transcript,
                    confidence: avg_confidence,
                    words: all_words,
                }],
            }],
        },
    })
}

fn validate_input_path(path: &Path) -> CliResult<()> {
    if !path.exists() {
        return Err(CliError::not_found(
            format!("input file '{}'", path.display()),
            None,
        ));
    }

    if !path.is_file() {
        return Err(CliError::invalid_argument(
            "--input",
            path.display().to_string(),
            "expected a file path",
        ));
    }

    Ok(())
}

fn format_timestamp(secs: f64) -> String {
    let total_secs = secs as u64;
    let mins = total_secs / 60;
    let s = total_secs % 60;
    let frac = ((secs - secs.floor()) * 10.0).round() as u64;
    format!("{mins:02}:{s:02}.{frac}")
}

fn format_pretty(response: &owhisper_interface::batch::Response) -> String {
    use owhisper_interface::batch::Word;

    let words: Vec<&Word> = response
        .results
        .channels
        .iter()
        .filter_map(|c| c.alternatives.first())
        .flat_map(|alt| &alt.words)
        .collect();

    if words.is_empty() {
        return extract_transcript(response);
    }

    // Words from different VAD chunks will have gaps between them.
    // Use a small threshold to detect segment boundaries.
    let pause_threshold = 0.5;
    let mut segments: Vec<(f64, f64, Vec<&str>)> = Vec::new();

    for word in &words {
        let text = word
            .punctuated_word
            .as_deref()
            .unwrap_or(word.word.as_str());

        let should_split = segments
            .last()
            .map(|(_, end, _)| word.start - *end > pause_threshold)
            .unwrap_or(true);

        if should_split {
            segments.push((word.start, word.end, vec![text]));
        } else {
            let seg = segments.last_mut().unwrap();
            seg.1 = word.end;
            seg.2.push(text);
        }
    }

    let term_width = textwrap::termwidth();

    segments
        .iter()
        .map(|(start, end, words)| {
            let prefix = format!(
                "\x1b[2m[{} \u{2192} {}]\x1b[0m  ",
                format_timestamp(*start),
                format_timestamp(*end),
            );
            // "[00:00.0 → 00:00.0]  " = 22 visible chars
            let prefix_visible_len = 22;
            let indent = " ".repeat(prefix_visible_len);
            let text = words.join(" ");

            let opts = textwrap::Options::new(term_width)
                .initial_indent(&prefix)
                .subsequent_indent(&indent);
            textwrap::fill(&text, opts)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn extract_transcript(response: &owhisper_interface::batch::Response) -> String {
    response
        .results
        .channels
        .iter()
        .filter_map(|c| c.alternatives.first())
        .map(|alt| alt.transcript.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

async fn write_text_response(output: Option<&Path>, transcript: String) -> CliResult<()> {
    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                CliError::operation_failed("create output directory", e.to_string())
            })?;
        }

        tokio::fs::write(path, transcript + "\n")
            .await
            .map_err(|e| CliError::operation_failed("write output", e.to_string()))?;
        return Ok(());
    }

    println!("{transcript}");
    Ok(())
}

async fn write_json_response(
    output: Option<&Path>,
    response: &owhisper_interface::batch::Response,
) -> CliResult<()> {
    let bytes = if std::io::stdout().is_terminal() {
        serde_json::to_vec_pretty(response)
    } else {
        serde_json::to_vec(response)
    }
    .map_err(|e| CliError::operation_failed("serialize response", e.to_string()))?;

    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                CliError::operation_failed("create output directory", e.to_string())
            })?;
        }

        tokio::fs::write(path, bytes)
            .await
            .map_err(|e| CliError::operation_failed("write output", e.to_string()))?;
        return Ok(());
    }

    std::io::stdout()
        .write_all(&bytes)
        .map_err(|e| CliError::operation_failed("write output", e.to_string()))?;
    std::io::stdout()
        .write_all(b"\n")
        .map_err(|e| CliError::operation_failed("write output", e.to_string()))?;
    Ok(())
}
