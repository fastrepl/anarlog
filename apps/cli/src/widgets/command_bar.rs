use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::theme::Theme;

pub struct CommandBar<'a> {
    buffer: &'a str,
    theme: &'a Theme,
}

impl<'a> CommandBar<'a> {
    pub fn new(buffer: &'a str, theme: &'a Theme) -> Self {
        Self { buffer, theme }
    }
}

impl Widget for CommandBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let line = Line::from(vec![
            Span::styled(" COMMAND ", self.theme.mode_command),
            Span::raw(" "),
            Span::styled(format!(":{}", self.buffer), Style::new().fg(Color::White)),
            Span::styled("\u{2588}", Style::new().fg(Color::Gray)),
        ]);
        Paragraph::new(line).render(area, buf);
    }
}
