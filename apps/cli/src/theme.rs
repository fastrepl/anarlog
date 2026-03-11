use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub border: Style,
    pub border_focused: Style,
    pub status_active: Style,
    pub status_degraded: Style,
    pub status_inactive: Style,
    pub error: Style,
    pub muted: Style,
    pub waveform_normal: Style,
    pub waveform_hot: Style,
    pub waveform_silent: Style,
    pub transcript_final: Style,
    pub transcript_pending: Style,
    pub transcript_partial: Style,
    pub placeholder: Style,
    pub shortcut_key: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            border: Style::new().fg(Color::DarkGray),
            border_focused: Style::new().fg(Color::Cyan),
            status_active: Style::new().fg(Color::Green),
            status_degraded: Style::new().fg(Color::Yellow),
            status_inactive: Style::new().fg(Color::Red),
            error: Style::new().fg(Color::Red),
            muted: Style::new().fg(Color::DarkGray),
            waveform_normal: Style::new().fg(Color::Red),
            waveform_hot: Style::new().fg(Color::LightRed),
            waveform_silent: Style::new().fg(Color::DarkGray),
            transcript_final: Style::new(),
            transcript_pending: Style::new().fg(Color::Yellow),
            transcript_partial: Style::new()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            placeholder: Style::new()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            shortcut_key: Style::new().fg(Color::DarkGray),
        }
    }
}
