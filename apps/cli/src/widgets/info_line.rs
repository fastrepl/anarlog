use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::theme::Theme;

pub struct InfoLine<'a> {
    items: Vec<Span<'a>>,
    theme: &'a Theme,
}

impl<'a> InfoLine<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self {
            items: Vec::new(),
            theme,
        }
    }

    pub fn item(mut self, span: Span<'a>) -> Self {
        self.items.push(span);
        self
    }
}

impl Widget for InfoLine<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut spans: Vec<Span> = vec![Span::raw(" ")];

        for (i, item) in self.items.into_iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled("  |  ", self.theme.muted));
            }
            spans.push(item);
        }

        Paragraph::new(Line::from(spans)).render(area, buf);
    }
}
