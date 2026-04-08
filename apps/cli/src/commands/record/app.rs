use std::path::PathBuf;
use std::time::Duration;

use ratatui::text::{Line, Span};

use super::{AudioMode, ProgressUpdate};
use crate::tui::waveform::{LiveWaveform, LiveWaveformState, WaveformMode};

const WAVEFORM_WIDTH: usize = 16;

pub(crate) struct App {
    pub(crate) output: PathBuf,
    pub(crate) sample_rate: u32,
    pub(crate) channels: u16,
    audio: AudioMode,
    elapsed: Duration,
    audio_secs: f64,
    waveform: LiveWaveformState,
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
            waveform: LiveWaveformState::new(WAVEFORM_WIDTH),
        }
    }

    pub(crate) fn audio_label(&self) -> &'static str {
        match self.audio {
            AudioMode::Input => "mic",
            AudioMode::Output => "system",
            AudioMode::Dual => "dual",
        }
    }

    fn waveform_mode(&self) -> WaveformMode {
        match self.audio {
            AudioMode::Dual => WaveformMode::Dual,
            AudioMode::Input | AudioMode::Output => WaveformMode::Mono,
        }
    }

    pub(crate) fn update(&mut self, progress: &ProgressUpdate) {
        self.elapsed = progress.elapsed;
        self.audio_secs = progress.audio_secs;
        self.waveform
            .push(progress.left_level, progress.right_level);
    }

    pub(crate) fn finish(&mut self, elapsed: Duration, audio_secs: f64) {
        self.elapsed = elapsed;
        self.audio_secs = audio_secs;
    }

    pub(crate) fn lines(&self) -> Vec<Line<'static>> {
        let file_name = self
            .output
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.output.display().to_string());

        let line0 = Line::from(format!(
            "recording {}  {}",
            self.audio_label(),
            format_elapsed(self.elapsed)
        ));
        let line1 = Line::from(format!("{} Hz  {} ch", self.sample_rate, self.channels));

        let mut spans = vec![Span::raw(format!("{}  ", file_name))];
        spans.extend(LiveWaveform::spans(
            &self.waveform,
            self.waveform_mode(),
            WAVEFORM_WIDTH,
        ));
        let line2 = Line::from(spans);

        vec![line0, line1, line2]
    }

    pub(crate) fn completion_lines(&self) -> Vec<Line<'static>> {
        let short = short_output_path(&self.output);
        let session_dir = session_dir_name(&self.output);

        vec![
            Line::from(format!("saved  {:.1}s  {}", self.audio_secs, short)),
            Line::from(format!("char play {session_dir}")),
            Line::from(format!("char transcribe {session_dir}")),
        ]
    }

    pub(crate) fn summary_line(&self) -> String {
        let session_dir = session_dir_name(&self.output);

        format!(
            "{:.1}s  {}\n\nchar play {session_dir}\nchar transcribe {session_dir}",
            self.audio_secs,
            self.output.display(),
        )
    }
}

fn short_output_path(path: &std::path::Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rel) = path.strip_prefix(&home) {
            return format!("~/{}", rel.display());
        }
    }
    path.display().to_string()
}

fn session_dir_name(output: &std::path::Path) -> String {
    output
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn format_elapsed(duration: Duration) -> String {
    let secs = duration.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::waveform::{MIC_COLOR, SYS_COLOR};

    const BLOCKS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

    #[test]
    fn dual_mode_overlaid_waveform() {
        let mut app = App::new(AudioMode::Dual, PathBuf::from("out.wav"), 16_000, 2);
        for _ in 0..3 {
            app.update(&ProgressUpdate {
                elapsed: Duration::from_secs(5),
                sample_count: 80_000,
                audio_secs: 5.0,
                left_level: 0.75,
                right_level: 0.25,
                render_ui: true,
                emit_event: true,
            });
        }

        let lines = app.lines();
        let text: String = lines[2].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("mic"));
        assert!(text.contains("sys"));

        let has_block = lines[2]
            .spans
            .iter()
            .any(|s| s.content.chars().any(|c| BLOCKS[1..].contains(&c)));
        assert!(has_block);
    }

    #[test]
    fn mic_dominant_gets_mic_color() {
        let mut app = App::new(AudioMode::Dual, PathBuf::from("out.wav"), 16_000, 2);
        app.update(&ProgressUpdate {
            elapsed: Duration::from_secs(1),
            sample_count: 16_000,
            audio_secs: 1.0,
            left_level: 0.5,
            right_level: 0.01,
            render_ui: true,
            emit_event: true,
        });
        let lines = app.lines();
        let block_span = lines[2]
            .spans
            .iter()
            .find(|s| s.content.chars().any(|c| BLOCKS[1..].contains(&c)));
        assert!(block_span.is_some());
        assert_eq!(block_span.unwrap().style.fg, Some(MIC_COLOR));
    }

    #[test]
    fn sys_dominant_gets_sys_color() {
        let mut app = App::new(AudioMode::Dual, PathBuf::from("out.wav"), 16_000, 2);
        app.update(&ProgressUpdate {
            elapsed: Duration::from_secs(1),
            sample_count: 16_000,
            audio_secs: 1.0,
            left_level: 0.01,
            right_level: 0.5,
            render_ui: true,
            emit_event: true,
        });
        let lines = app.lines();
        let block_span = lines[2]
            .spans
            .iter()
            .find(|s| s.content.chars().any(|c| BLOCKS[1..].contains(&c)));
        assert!(block_span.is_some());
        assert_eq!(block_span.unwrap().style.fg, Some(SYS_COLOR));
    }
}
