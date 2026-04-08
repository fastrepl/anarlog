use ratatui::style::{Color, Modifier, Style};

use crate::event_row::RowStatus;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Theme {
    colors: bool,
}

impl Theme {
    pub(crate) const fn new(colors: bool) -> Self {
        Self { colors }
    }

    pub(crate) fn title(self) -> Style {
        self.style(Color::Cyan, true)
    }

    pub(crate) fn label(self) -> Style {
        self.style(Color::Gray, true)
    }

    pub(crate) fn timestamp(self) -> Style {
        self.style(Color::DarkGray, false)
    }

    pub(crate) fn selected_row(self) -> Style {
        if self.colors {
            Style::default()
                .bg(Color::Rgb(32, 45, 66))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        }
    }

    pub(crate) fn range_row(self) -> Style {
        if self.colors {
            Style::default().bg(Color::Rgb(19, 27, 38))
        } else {
            Style::default().add_modifier(Modifier::UNDERLINED)
        }
    }

    pub(crate) fn status(self, status: RowStatus) -> Style {
        match status {
            RowStatus::Focus => self.style(Color::Cyan, true),
            RowStatus::Update => self.style(Color::Yellow, true),
            RowStatus::Idle => self.style(Color::DarkGray, true),
            RowStatus::Screenshot => self.style(Color::Green, true),
        }
    }

    pub(crate) fn app(self, app_name: &str) -> Style {
        if !self.colors {
            return Style::default().add_modifier(Modifier::BOLD);
        }

        let palette = [
            Color::Blue,
            Color::Magenta,
            Color::Cyan,
            Color::Green,
            Color::Yellow,
            Color::Red,
        ];
        let index = app_name.bytes().fold(0usize, |acc, byte| {
            acc.wrapping_mul(31).wrapping_add(byte as usize)
        }) % palette.len();

        Style::default()
            .fg(palette[index])
            .add_modifier(Modifier::BOLD)
    }

    fn style(self, color: Color, bold: bool) -> Style {
        let mut style = if self.colors {
            Style::default().fg(color)
        } else {
            Style::default()
        };
        if bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        style
    }
}
