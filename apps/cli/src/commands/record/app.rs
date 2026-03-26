use std::path::PathBuf;
use std::time::Duration;

use super::{AudioMode, ProgressUpdate};

pub(crate) struct App {
    pub(crate) output: PathBuf,
    pub(crate) sample_rate: u32,
    pub(crate) channels: u16,
    audio: AudioMode,
    elapsed: Duration,
    audio_secs: f64,
    left_level: f32,
    right_level: f32,
}

impl App {
    pub(crate) fn new(audio: AudioMode, output: PathBuf, sample_rate: u32, channels: u16) -> Self {
        Self {
            output,
            sample_rate,
            channels,
            audio,
            elapsed: Duration::ZERO,
            audio_secs: 0.0,
            left_level: 0.0,
            right_level: 0.0,
        }
    }

    pub(crate) fn audio_label(&self) -> &'static str {
        match self.audio {
            AudioMode::Input => "mic",
            AudioMode::Output => "system",
            AudioMode::Dual => "dual",
        }
    }

    pub(crate) fn update(&mut self, progress: &ProgressUpdate) {
        self.elapsed = progress.elapsed;
        self.audio_secs = progress.audio_secs;
        self.left_level = progress.left_level;
        self.right_level = progress.right_level;
    }

    pub(crate) fn finish(&mut self, elapsed: Duration, audio_secs: f64) {
        self.elapsed = elapsed;
        self.audio_secs = audio_secs;
    }

    pub(crate) fn lines(&self) -> [String; 3] {
        [
            format!(
                "recording {}  {}",
                self.audio_label(),
                format_elapsed(self.elapsed)
            ),
            format!(
                "{} Hz  {} ch  {:.1}s audio",
                self.sample_rate, self.channels, self.audio_secs
            ),
            match self.audio {
                AudioMode::Dual => format!(
                    "{}  mic {}  sys {}",
                    self.output.display(),
                    meter(self.left_level),
                    meter(self.right_level)
                ),
                _ => format!("{}  lvl {}", self.output.display(), meter(self.left_level)),
            },
        ]
    }

    pub(crate) fn summary_line(&self) -> String {
        format!(
            "{:.1}s audio, {:.1}s elapsed -> {}",
            self.audio_secs,
            self.elapsed.as_secs_f64(),
            self.output.display()
        )
    }
}

fn format_elapsed(duration: Duration) -> String {
    let secs = duration.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

fn meter(level: f32) -> String {
    const BARS: usize = 8;
    let filled = ((level.clamp(0.0, 1.0) * BARS as f32).round() as usize).min(BARS);
    let mut out = String::with_capacity(BARS);
    for idx in 0..BARS {
        out.push(if idx < filled { '|' } else { '.' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dual_mode_lines_include_both_meters() {
        let mut app = App::new(AudioMode::Dual, PathBuf::from("out.wav"), 16_000, 2);
        app.update(&ProgressUpdate {
            elapsed: Duration::from_secs(5),
            sample_count: 80_000,
            audio_secs: 5.0,
            left_level: 0.75,
            right_level: 0.25,
            render_ui: true,
            emit_event: true,
        });

        let lines = app.lines();
        assert!(lines[2].contains("mic"));
        assert!(lines[2].contains("sys"));
    }
}
