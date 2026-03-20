use ratatui::style::{Color, Modifier, Style};

pub trait StyleSheet: Clone + Send + Sync + 'static {
    fn heading(&self, level: u8) -> Style;
    fn strong(&self) -> Style;
    fn emphasis(&self) -> Style;
    fn strikethrough(&self) -> Style;
    fn code_inline(&self) -> Style;
    fn code_fence(&self) -> Style;
    fn link(&self) -> Style;
    fn blockquote(&self) -> Style;
    fn list_marker(&self) -> Style;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultStyleSheet;

impl StyleSheet for DefaultStyleSheet {
    fn heading(&self, level: u8) -> Style {
        let color = match level {
            1 => Color::Cyan,
            2 => Color::Blue,
            3 => Color::Green,
            4 => Color::Yellow,
            _ => Color::Magenta,
        };
        Style::new().fg(color).add_modifier(Modifier::BOLD)
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
        Style::new().fg(Color::Gray)
    }

    fn code_fence(&self) -> Style {
        Style::new().fg(Color::Gray)
    }

    fn link(&self) -> Style {
        Style::new()
            .fg(Color::Blue)
            .add_modifier(Modifier::UNDERLINED)
    }

    fn blockquote(&self) -> Style {
        Style::new()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC)
    }

    fn list_marker(&self) -> Style {
        Style::new().fg(Color::Cyan)
    }
}
