pub mod shell;

pub use super::tracing::TracingCapture;

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use super::app::{App, ChannelTranscript, TranscriptContent};
use super::audio::ChannelKind;
use crate::theme::Theme;

pub(crate) fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let width = frame.area().width.saturating_sub(4) as usize;
    let view = app.transcript_view(width);
    let lines = match view.content {
        TranscriptContent::Raw(raw) => render_raw_channels(raw.channels()),
        TranscriptContent::Rich(lines) => lines,
    };
    app.shell_mut().draw(
        frame,
        view.title,
        lines,
        &view.placeholder,
        view.border_style,
    );
}

fn render_raw_channels(channels: &[ChannelTranscript]) -> Vec<Line<'static>> {
    channels.iter().filter_map(render_channel_line).collect()
}

fn render_channel_line(ch: &ChannelTranscript) -> Option<Line<'static>> {
    let confirmed = ch.confirmed_text();
    let partial = ch.partial();
    if confirmed.is_empty() && partial.is_empty() {
        return None;
    }

    let from_str = if ch.has_confirmed() {
        fmt_ts(0.0)
    } else {
        "--:--".to_string()
    };

    let prefix = format!("[{} / {}]", from_str, fmt_ts(ch.elapsed_secs()));
    let theme = Theme::DEFAULT;
    let (confirmed_style, partial_style) = match ch.kind() {
        ChannelKind::Mic => (theme.raw_mic_confirmed, theme.raw_mic_partial),
        ChannelKind::Speaker => (theme.raw_speaker_confirmed, theme.raw_speaker_partial),
    };

    let mut spans = vec![
        Span::styled(prefix, Style::new().fg(Color::DarkGray)),
        Span::raw(" "),
        Span::styled(confirmed, confirmed_style),
    ];

    if !partial.is_empty() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(partial.to_string(), partial_style));
    }

    Some(Line::from(spans))
}

fn fmt_ts(secs: f64) -> String {
    let m = (secs / 60.0) as u32;
    let s = secs % 60.0;
    format!("{:02}:{:02}", m, s as u32)
}
