use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{List, ListItem, ListState, StatefulWidget};

use crate::theme::Theme;

pub struct SelectList<'a> {
    items: Vec<ListItem<'a>>,
    theme: &'a Theme,
}

impl<'a> SelectList<'a> {
    pub fn new(items: Vec<ListItem<'a>>, theme: &'a Theme) -> Self {
        Self { items, theme }
    }
}

impl StatefulWidget for SelectList<'_> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut ListState) {
        let list = List::new(self.items)
            .highlight_style(Style::new().bg(self.theme.highlight_bg))
            .scroll_padding(1);
        StatefulWidget::render(list, area, buf, state);
    }
}
