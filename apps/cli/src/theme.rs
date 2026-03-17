use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub accent: Style,
    pub input_bg: Color,
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
    pub transcript_partial: Style,
    pub placeholder: Style,
    pub shortcut_key: Style,
    pub speaker_label: Style,
    pub timestamp: Style,
    pub raw_mic_confirmed: Style,
    pub raw_mic_partial: Style,
    pub raw_speaker_confirmed: Style,
    pub raw_speaker_partial: Style,
}

impl Theme {
    pub const DEFAULT: Self = Self {
        accent: Style::new().fg(Color::Yellow),
        input_bg: Color::Rgb(28, 30, 38),
        border: Style::new().fg(Color::DarkGray),
        border_focused: Style::new().fg(Color::Yellow),
        status_active: Style::new().fg(Color::Green),
        status_degraded: Style::new().fg(Color::Yellow),
        status_inactive: Style::new().fg(Color::Red),
        error: Style::new().fg(Color::Red),
        muted: Style::new().fg(Color::DarkGray),
        waveform_normal: Style::new().fg(Color::Red),
        waveform_hot: Style::new().fg(Color::LightRed),
        waveform_silent: Style::new().fg(Color::DarkGray),
        transcript_final: Style::new(),
        transcript_partial: Style::new()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
        placeholder: Style::new()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
        shortcut_key: Style::new().fg(Color::DarkGray),
        speaker_label: Style::new().fg(Color::Yellow),
        timestamp: Style::new().fg(Color::DarkGray),
        raw_mic_confirmed: Style::new()
            .fg(Color::Rgb(255, 190, 190))
            .add_modifier(Modifier::BOLD),
        raw_mic_partial: Style::new().fg(Color::Rgb(128, 95, 95)),
        raw_speaker_confirmed: Style::new()
            .fg(Color::Rgb(190, 200, 255))
            .add_modifier(Modifier::BOLD),
        raw_speaker_partial: Style::new().fg(Color::Rgb(95, 100, 128)),
    };
}
