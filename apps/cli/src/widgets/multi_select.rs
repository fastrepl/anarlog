use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    List, ListItem, ListState, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
};

use crate::theme::Theme;

pub enum MultiSelectEntry<'a> {
    Group(Line<'a>),
    Item { checked: bool, label: Line<'a> },
}

pub struct MultiSelectState {
    selected: usize,
}

impl MultiSelectState {
    pub fn new(selected: usize) -> Self {
        Self { selected }
    }
}

pub struct MultiSelect<'a> {
    entries: Vec<MultiSelectEntry<'a>>,
    theme: &'a Theme,
}

impl<'a> MultiSelect<'a> {
    pub fn new(entries: Vec<MultiSelectEntry<'a>>, theme: &'a Theme) -> Self {
        Self { entries, theme }
    }
}

impl StatefulWidget for MultiSelect<'_> {
    type State = MultiSelectState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut MultiSelectState) {
        let mut list_items: Vec<ListItem> = Vec::new();
        let mut data_to_visual: Vec<usize> = Vec::new();

        for entry in self.entries {
            match entry {
                MultiSelectEntry::Group(line) => {
                    list_items.push(ListItem::new(line));
                }
                MultiSelectEntry::Item { checked, label } => {
                    data_to_visual.push(list_items.len());
                    let checkbox = if checked { "[x] " } else { "[ ] " };
                    let mut spans = vec![Span::raw(checkbox)];
                    spans.extend(label.spans);
                    list_items.push(ListItem::new(Line::from(spans)));
                }
            }
        }

        let total_items = list_items.len();
        let visual_idx = data_to_visual.get(state.selected).copied().unwrap_or(0);
        let mut visual_state = ListState::default().with_selected(Some(visual_idx));

        let list = List::new(list_items)
            .highlight_style(Style::new().bg(self.theme.highlight_bg))
            .scroll_padding(1);
        StatefulWidget::render(list, area, buf, &mut visual_state);

        if total_items as u16 > area.height {
            let mut scrollbar_state =
                ScrollbarState::new(total_items).position(visual_state.offset());
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
            StatefulWidget::render(scrollbar, area, buf, &mut scrollbar_state);
        }
    }
}
