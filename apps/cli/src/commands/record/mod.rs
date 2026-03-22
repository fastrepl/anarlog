mod app;
mod runtime;
mod ui;

use std::fs;
use std::io::{BufWriter, IsTerminal, Write};
use std::path::PathBuf;

use hypr_audio::AudioProvider;
use hypr_audio_actual::ActualAudio;
use hypr_audio_utils::chunk_size_for_stt;
use serde::Serialize;

use crate::error::{CliError, CliResult};

pub(crate) use app::App;
pub(crate) use runtime::{CaptureResult, ProgressUpdate};
pub(crate) use ui::RecordViewport;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum AudioMode {
    Input,
    Output,
    Dual,
}

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum EventFormat {
    Jsonl,
}

#[derive(clap::Args)]
pub struct Args {
    #[arg(long, value_enum, default_value = "input")]
    pub audio: AudioMode,
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: Option<PathBuf>,
    #[arg(long, default_value = "16000")]
    pub sample_rate: u32,
    #[arg(long, value_enum)]
    pub events: Option<EventFormat>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RecordEvent {
    Started {
        audio: String,
        sample_rate: u32,
        channels: u16,
        output: String,
        started_at: String,
    },
    Progress {
        elapsed_ms: u64,
        audio_secs: f64,
        sample_count: u64,
        level_left: f32,
        level_right: f32,
    },
    Stopped {
        reason: String,
        elapsed_ms: u64,
        audio_secs: f64,
        output: String,
    },
    Failed {
        stage: String,
        message: String,
    },
}

struct EventWriter<W: Write> {
    writer: W,
}

impl<W: Write> EventWriter<W> {
    fn new(writer: W) -> Self {
        Self { writer }
    }

    fn emit(&mut self, event: &RecordEvent) -> CliResult<()> {
        serde_json::to_writer(&mut self.writer, event)
            .map_err(|e| CliError::operation_failed("serialize record event", e.to_string()))?;
        self.writer
            .write_all(b"\n")
            .map_err(|e| CliError::operation_failed("write record event", e.to_string()))?;
        self.writer
            .flush()
            .map_err(|e| CliError::operation_failed("flush record event", e.to_string()))
    }
}

pub async fn run(args: Args, quiet: bool) -> CliResult<()> {
    run_with_audio(args, quiet, &ActualAudio).await
}

async fn run_with_audio<A: AudioProvider>(args: Args, quiet: bool, audio: &A) -> CliResult<()> {
    let output_path = args.output.unwrap_or_else(|| {
        let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let suffix = match args.audio {
            AudioMode::Input => "input",
            AudioMode::Output => "output",
            AudioMode::Dual => "dual",
        };
        PathBuf::from(format!("recording_{ts}_{suffix}.wav"))
    });

    let channels: u16 = match args.audio {
        AudioMode::Input | AudioMode::Output => 1,
        AudioMode::Dual => 2,
    };
    let chunk_size = chunk_size_for_stt(args.sample_rate);

    let mut app = App::new(args.audio, output_path.clone(), args.sample_rate, channels);

    let mut event_writer = match args.events {
        Some(EventFormat::Jsonl) => Some(EventWriter::new(BufWriter::new(std::io::stdout()))),
        None => None,
    };
    if let Some(writer) = event_writer.as_mut() {
        writer.emit(&RecordEvent::Started {
            audio: app.audio_label().to_string(),
            sample_rate: args.sample_rate,
            channels,
            output: output_path.display().to_string(),
            started_at: chrono::Utc::now().to_rfc3339(),
        })?;
    }

    let stderr_is_tty = std::io::stderr().is_terminal();
    let mut viewport = if quiet || !stderr_is_tty {
        None
    } else {
        Some(
            RecordViewport::stderr()
                .map_err(|e| CliError::operation_failed("init record viewport", e.to_string()))?,
        )
    };
    if let Some(view) = viewport.as_mut() {
        view.render(&app.lines())
            .map_err(|e| CliError::operation_failed("render record viewport", e.to_string()))?;
    } else if !quiet {
        eprintln!(
            "recording {} -> {}",
            app.audio_label(),
            output_path.display()
        );
    }

    let capture = runtime::capture(
        audio,
        args.audio,
        args.sample_rate,
        chunk_size,
        |progress| {
            app.update(&progress);
            if progress.emit_event {
                if let Some(writer) = event_writer.as_mut() {
                    writer.emit(&RecordEvent::Progress {
                        elapsed_ms: progress.elapsed.as_millis() as u64,
                        audio_secs: progress.audio_secs,
                        sample_count: progress.sample_count,
                        level_left: progress.left_level,
                        level_right: progress.right_level,
                    })?;
                }
            }
            if progress.render_ui {
                if let Some(view) = viewport.as_mut() {
                    view.render(&app.lines()).map_err(|e| {
                        CliError::operation_failed("render record viewport", e.to_string())
                    })?;
                }
            }
            Ok(())
        },
    )
    .await;

    match capture {
        Ok(result) => {
            finish_success(
                result,
                &mut app,
                viewport.as_mut(),
                event_writer.as_mut(),
                quiet,
            )
            .await
        }
        Err(error) => {
            if let Some(view) = viewport.as_mut() {
                view.clear().ok();
            }
            if let Some(writer) = event_writer.as_mut() {
                writer.emit(&RecordEvent::Failed {
                    stage: "capture".to_string(),
                    message: error.to_string(),
                })?;
            }
            Err(error)
        }
    }
}

async fn finish_success<W2: Write>(
    result: CaptureResult,
    app: &mut App,
    viewport: Option<&mut RecordViewport>,
    event_writer: Option<&mut EventWriter<W2>>,
    quiet: bool,
) -> CliResult<()> {
    ensure_parent_dirs(&app.output)?;

    let spec = hound::WavSpec {
        channels: app.channels,
        sample_rate: app.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(&app.output, spec)
        .map_err(|e| CliError::operation_failed("create wav file", e.to_string()))?;
    for &sample in &result.samples {
        writer
            .write_sample(sample)
            .map_err(|e| CliError::operation_failed("write wav sample", e.to_string()))?;
    }
    writer
        .finalize()
        .map_err(|e| CliError::operation_failed("finalize wav", e.to_string()))?;

    app.finish(result.elapsed, result.audio_secs);

    if let Some(view) = viewport {
        view.clear()
            .map_err(|e| CliError::operation_failed("clear record viewport", e.to_string()))?;
    }

    if let Some(writer) = event_writer {
        writer.emit(&RecordEvent::Stopped {
            reason: result.stop_reason.as_str().to_string(),
            elapsed_ms: result.elapsed.as_millis() as u64,
            audio_secs: result.audio_secs,
            output: app.output.display().to_string(),
        })?;
    }

    if !quiet {
        eprintln!("{}", app.summary_line());
    }

    Ok(())
}

fn ensure_parent_dirs(path: &std::path::Path) -> CliResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| CliError::operation_failed("create output directory", e.to_string()))?;
    }
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
            .emit(&RecordEvent::Started {
                audio: "mic".to_string(),
                sample_rate: 16_000,
                channels: 1,
                output: "foo.wav".to_string(),
                started_at: "2026-03-21T00:00:00Z".to_string(),
            })
            .unwrap();

        let output = String::from_utf8(bytes.into_inner()).unwrap();
        assert!(output.ends_with('\n'));
        assert!(output.contains("\"type\":\"started\""));
        assert!(output.contains("\"output\":\"foo.wav\""));
    }
}
