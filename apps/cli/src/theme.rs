use hypr_cli_editor::StyleSheet;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders};

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub bg: Color,
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
    pub highlight_bg: Color,
    pub disabled_bg: Color,
    pub overlay_bg: Color,
    pub dialog_bg: Color,
    pub dialog_title_fg: Color,
    pub user_bar: Style,
    pub mode_insert: Style,
    pub mode_normal: Style,
    pub mode_command: Style,
}

impl Theme {
    pub fn bordered_block(&self, focused: bool) -> Block<'static> {
        let style = if focused {
            self.border_focused
        } else {
            self.border
        };
        Block::new().borders(Borders::ALL).border_style(style)
    }

    pub const TRANSPARENT: Self = Self {
        bg: Color::Reset,
        ..Self::DEFAULT
    };

    pub const DEFAULT: Self = Self {
        bg: Color::Rgb(13, 17, 22),
        accent: Style::new().fg(Color::Yellow),
        input_bg: Color::Rgb(22, 27, 34),
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
        highlight_bg: Color::Rgb(30, 60, 100),
        disabled_bg: Color::Rgb(22, 22, 30),
        overlay_bg: Color::Rgb(2, 4, 10),
        dialog_bg: Color::Rgb(18, 22, 28),
        dialog_title_fg: Color::White,
        user_bar: Style::new().fg(Color::Indexed(69)),
        mode_insert: Style::new().fg(Color::Black).bg(Color::Green),
        mode_normal: Style::new().fg(Color::Black).bg(Color::Cyan),
        mode_command: Style::new().fg(Color::Black).bg(Color::Yellow),
    };
}

impl StyleSheet for Theme {
    fn heading(&self, level: u8) -> Style {
        match level {
            1 => self.accent.add_modifier(Modifier::BOLD),
            2 => Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            3 => Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
            _ => Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD),
        }
    }

    fn strong(&self) -> Style {
        Style::new().add_modifier(Modifier::BOLD)
    }

    fn emphasis(&self) -> Style {
        Style::new().add_modifier(Modifier::ITALIC)
    }

    fn strikethrough(&self) -> Style {
        Style::new().add_modifier(Modifier::CROSSED_OUT)
    }

    fn code_inline(&self) -> Style {
        self.muted
    }

    fn code_fence(&self) -> Style {
        self.muted
    }

    fn link(&self) -> Style {
        Style::new()
            .fg(Color::Blue)
            .add_modifier(Modifier::UNDERLINED)
    }

    fn blockquote(&self) -> Style {
        self.muted.add_modifier(Modifier::ITALIC)
    }

    fn list_marker(&self) -> Style {
        self.accent
    }
}
