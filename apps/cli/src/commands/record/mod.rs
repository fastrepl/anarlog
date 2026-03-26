mod app;
mod runtime;

use std::fs;
use std::io::{BufWriter, IsTerminal, Write};
use std::path::PathBuf;

use hypr_audio::AudioProvider;
use hypr_audio_actual::ActualAudio;
use hypr_audio_utils::chunk_size_for_stt;
use serde::Serialize;

use crate::cli::OutputFormat;
use crate::error::{CliError, CliResult};
use crate::output::EventWriter;

pub(crate) use app::App;
pub(crate) use runtime::{CaptureResult, ProgressUpdate};

use crate::tui::InlineViewport;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum AudioMode {
    Input,
    Output,
    Dual,
}

#[derive(clap::Args)]
pub struct Args {
    #[arg(long, value_enum, default_value = "input")]
    pub audio: AudioMode,
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: Option<PathBuf>,
    #[arg(short = 'f', long, value_enum, default_value = "pretty")]
    pub format: OutputFormat,
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

pub async fn run(args: Args, quiet: bool) -> CliResult<()> {
    run_with_audio(args, quiet, &ActualAudio).await
}

async fn run_with_audio<A: AudioProvider>(args: Args, quiet: bool, audio: &A) -> CliResult<()> {
    let sample_rate = 16_000u32;
    let format = args.format;

    let output_path = args.output.unwrap_or_else(|| {
        let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let suffix = match args.audio {
            AudioMode::Input => "input",
            AudioMode::Output => "output",
            AudioMode::Dual => "dual",
        };
        PathBuf::from(format!("recording_{ts}_{suffix}.mp3"))
    });

    let channels: u16 = match args.audio {
        AudioMode::Input | AudioMode::Output => 1,
        AudioMode::Dual => 2,
    };
    let chunk_size = chunk_size_for_stt(sample_rate);

    let mut app = App::new(args.audio, output_path.clone(), sample_rate, channels);

    let mut event_writer = match format {
        OutputFormat::Json => Some(EventWriter::new(BufWriter::new(std::io::stdout()))),
        OutputFormat::Pretty => None,
    };
    if let Some(writer) = event_writer.as_mut() {
        writer.emit(&RecordEvent::Started {
            audio: app.audio_label().to_string(),
            sample_rate,
            channels,
            output: output_path.display().to_string(),
            started_at: chrono::Utc::now().to_rfc3339(),
        })?;
    }

    let stderr_is_tty = std::io::stderr().is_terminal();
    let mut viewport = match format {
        OutputFormat::Pretty if !quiet && stderr_is_tty => Some(
            InlineViewport::stderr(5, None)
                .map_err(|e| CliError::operation_failed("init record viewport", e.to_string()))?,
        ),
        _ => None,
    };
    if let Some(view) = viewport.as_mut() {
        view.draw(&app.lines());
    } else if !quiet && matches!(format, OutputFormat::Pretty) {
        eprintln!(
            "recording {} -> {}",
            app.audio_label(),
            output_path.display()
        );
    }

    let capture = runtime::capture(audio, args.audio, sample_rate, chunk_size, |progress| {
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
                view.draw(&app.lines());
            }
        }
        Ok(())
    })
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
    viewport: Option<&mut InlineViewport>,
    event_writer: Option<&mut EventWriter<W2>>,
    quiet: bool,
) -> CliResult<()> {
    ensure_parent_dirs(&app.output)?;

    let mut mp3_buf = Vec::new();
    match app.channels {
        1 => {
            let mut encoder = hypr_mp3::MonoStreamEncoder::new(app.sample_rate)
                .map_err(|e| CliError::operation_failed("create mp3 encoder", e.to_string()))?;
            encoder
                .encode_i16(&result.samples, &mut mp3_buf)
                .map_err(|e| CliError::operation_failed("encode mp3", e.to_string()))?;
            encoder
                .flush(&mut mp3_buf)
                .map_err(|e| CliError::operation_failed("flush mp3", e.to_string()))?;
        }
        2 => {
            let mut encoder = hypr_mp3::StereoStreamEncoder::new(app.sample_rate)
                .map_err(|e| CliError::operation_failed("create mp3 encoder", e.to_string()))?;
            let (left, right): (Vec<i16>, Vec<i16>) = result
                .samples
                .chunks(2)
                .map(|pair| (pair[0], pair.get(1).copied().unwrap_or(0)))
                .unzip();
            encoder
                .encode_i16(&left, &right, &mut mp3_buf)
                .map_err(|e| CliError::operation_failed("encode mp3", e.to_string()))?;
            encoder
                .flush(&mut mp3_buf)
                .map_err(|e| CliError::operation_failed("flush mp3", e.to_string()))?;
        }
        _ => {
            return Err(CliError::operation_failed(
                "encode mp3",
                format!("unsupported channel count: {}", app.channels),
            ));
        }
    }
    std::fs::write(&app.output, &mp3_buf)
        .map_err(|e| CliError::operation_failed("write mp3 file", e.to_string()))?;

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
