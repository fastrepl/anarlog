use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Duration;

use ratatui::text::Line;
use rodio::source::Source;

use crate::app::AppContext;
use crate::error::{CliError, CliResult};
use crate::tui::waveform::{
    MIC_COLOR, PlaybackWaveform, SYS_COLOR, compute_peaks, deinterleave_stereo,
};
use crate::tui::{InlineViewport, InputAction};

const WAVEFORM_WIDTH: usize = 46;
const SEEK_STEP: Duration = Duration::from_secs(5);

#[derive(clap::Args)]
pub struct Args {
    /// Timestamp (e.g. 20260327_143022) or path to an audio file
    pub target: String,

    /// Base directory for session lookup
    #[arg(long, env = "CHAR_BASE", hide_env_values = true, value_name = "DIR")]
    pub base: Option<PathBuf>,
}

fn resolve_audio_path(target: &str, base: Option<&std::path::Path>) -> CliResult<PathBuf> {
    let as_path = PathBuf::from(target);
    if as_path.is_file() {
        return Ok(as_path);
    }

    let base = base.map(PathBuf::from).unwrap_or_else(|| {
        dirs::data_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("char")
    });

    let session_audio = base.join(target).join("audio.mp3");
    if session_audio.is_file() {
        return Ok(session_audio);
    }

    Err(CliError::not_found(
        format!("audio file for '{target}'"),
        Some(format!(
            "Pass a file path or a session timestamp.\nLooked in: {}",
            session_audio.display()
        )),
    ))
}

struct PlayState {
    file_name: String,
    total: Duration,
    left_peaks: Vec<f32>,
    right_peaks: Option<Vec<f32>>,
    pos: Duration,
    paused: bool,
}

impl PlayState {
    fn fraction(&self) -> f64 {
        if self.total.as_secs_f64() > 0.0 {
            (self.pos.as_secs_f64() / self.total.as_secs_f64()).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn waveform_lines(&self, fraction: f64) -> Vec<Line<'static>> {
        match &self.right_peaks {
            Some(right) => PlaybackWaveform::lines_dual(
                &self.left_peaks,
                right,
                fraction,
                MIC_COLOR,
                SYS_COLOR,
                WAVEFORM_WIDTH,
                2,
            ),
            None => {
                PlaybackWaveform::lines(&self.left_peaks, fraction, MIC_COLOR, WAVEFORM_WIDTH, 2)
            }
        }
    }

    fn lines(&self) -> Vec<Line<'static>> {
        let mut lines = self.waveform_lines(self.fraction());

        let status = if self.paused { "paused " } else { "playing" };
        lines.push(Line::from(format!(
            "{}  {} / {}  {}",
            status,
            format_duration(self.pos),
            format_duration(self.total),
            self.file_name,
        )));

        lines
    }

    fn completion_lines(&self) -> Vec<Line<'static>> {
        let mut lines = self.waveform_lines(1.0);
        lines.push(Line::from(format!(
            "played  {}  {}",
            format_duration(self.total),
            self.file_name,
        )));
        lines
    }
}

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

pub async fn run(ctx: &AppContext, args: Args) -> CliResult<()> {
    use rodio::{Decoder, Player, stream::DeviceSinkBuilder};

    let path = resolve_audio_path(&args.target, args.base.as_deref())?;

    let bytes = std::fs::read(&path)
        .map_err(|e| CliError::operation_failed("read audio file", e.to_string()))?;

    // Decode once to analyze peaks and compute duration.
    let analyze = Decoder::try_from(std::io::Cursor::new(bytes.clone()))
        .map_err(|e| CliError::operation_failed("decode audio file", e.to_string()))?;
    let sample_rate = analyze.sample_rate().get() as f64;
    let num_channels = analyze.channels().get() as usize;
    let samples: Vec<f32> = analyze.collect();
    let duration =
        Duration::from_secs_f64(samples.len() as f64 / (sample_rate * num_channels as f64));

    let (left_peaks, right_peaks) = if num_channels >= 2 {
        let (left, right) = deinterleave_stereo(&samples);
        (
            compute_peaks(&left, WAVEFORM_WIDTH),
            Some(compute_peaks(&right, WAVEFORM_WIDTH)),
        )
    } else {
        (compute_peaks(&samples, WAVEFORM_WIDTH), None)
    };

    // Decode again for playback.
    let source = Decoder::try_from(std::io::Cursor::new(bytes))
        .map_err(|e| CliError::operation_failed("decode audio file", e.to_string()))?;

    let mut stream = DeviceSinkBuilder::open_default_sink()
        .map_err(|e| CliError::operation_failed("open audio device", e.to_string()))?;
    stream.log_on_drop(false);

    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());

    let mut state = PlayState {
        file_name,
        total: duration,
        left_peaks,
        right_peaks,
        pos: Duration::ZERO,
        paused: false,
    };

    let quiet = ctx.quiet();
    let stderr_is_tty = std::io::stderr().is_terminal();
    let mut viewport = if !quiet && stderr_is_tty {
        Some(
            InlineViewport::stderr_interactive(5, ctx.trace_buffer(), true)
                .map_err(|e| CliError::operation_failed("init play viewport", e.to_string()))?,
        )
    } else {
        None
    };

    if let Some(view) = viewport.as_mut() {
        view.draw(&state.lines());
    } else if !quiet {
        eprintln!("Playing {}", path.display());
    }

    let player = Player::connect_new(stream.mixer());
    player.append(source);

    let tick = Duration::from_millis(100);
    loop {
        let done = player.get_pos() >= duration || player.empty();

        if done {
            break;
        }

        tokio::select! {
            _ = tokio::time::sleep(tick) => {
                state.pos = player.get_pos();
                state.paused = player.is_paused();

                if let Some(view) = viewport.as_mut() {
                    for action in view.poll_input() {
                        match action {
                            InputAction::TogglePause => {
                                if player.is_paused() {
                                    player.play();
                                } else {
                                    player.pause();
                                }
                            }
                            InputAction::SeekForward => {
                                let target =
                                    (player.get_pos() + SEEK_STEP).min(state.total);
                                let _ = player.try_seek(target);
                            }
                            InputAction::SeekBackward => {
                                let target = player.get_pos().saturating_sub(SEEK_STEP);
                                let _ = player.try_seek(target);
                            }
                            _ => {}
                        }
                    }
                    state.pos = player.get_pos();
                    state.paused = player.is_paused();
                    view.draw(&state.lines());
                }
            }
            _ = tokio::signal::ctrl_c() => {
                break;
            }
        }
    }

    if let Some(view) = viewport.as_mut() {
        state.pos = player.get_pos();
        view.draw(&state.completion_lines());
        view.finish()
            .map_err(|e| CliError::operation_failed("finish play viewport", e.to_string()))?;
    }

    Ok(())
}
