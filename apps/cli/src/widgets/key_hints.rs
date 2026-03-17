use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::theme::Theme;

pub struct KeyHints<'a> {
    badge: Option<(&'a str, Style)>,
    hints: Vec<(&'a str, &'a str)>,
    suffix: Option<Span<'a>>,
    theme: &'a Theme,
}

impl<'a> KeyHints<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self {
            badge: None,
            hints: Vec::new(),
            suffix: None,
            theme,
        }
    }

    pub fn badge(mut self, text: &'a str, style: Style) -> Self {
        self.badge = Some((text, style));
        self
    }

    pub fn hints(mut self, hints: Vec<(&'a str, &'a str)>) -> Self {
        self.hints = hints;
        self
    }

    pub fn suffix(mut self, span: Span<'a>) -> Self {
        self.suffix = Some(span);
        self
    }
}

impl Widget for KeyHints<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut spans: Vec<Span> = Vec::new();

        if let Some((text, style)) = self.badge {
            spans.push(Span::styled(text, style));
            spans.push(Span::raw(" "));
        }

        for (i, (key, desc)) in self.hints.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw("  "));
            }
            spans.push(Span::styled(format!("[{key}]"), self.theme.shortcut_key));
            spans.push(Span::raw(format!(" {desc}")));
        }

        if let Some(suffix) = self.suffix {
            spans.push(Span::raw("  "));
            spans.push(suffix);
        }

        Paragraph::new(Line::from(spans)).render(area, buf);
    }
}
