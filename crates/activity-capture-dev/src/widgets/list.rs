use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Block, HighlightSpacing, List, ListItem, ListState, StatefulWidget},
};

use crate::{event_row::EventRow, theme::Theme};

use super::row::{app_width, row_line};

pub(super) struct EventList<'a> {
    events: &'a [EventRow],
    theme: Theme,
    selected_range: Option<(usize, usize)>,
    list_inner_area: &'a mut Rect,
}

impl<'a> EventList<'a> {
    pub(super) fn new(
        events: &'a [EventRow],
        theme: Theme,
        selected_range: Option<(usize, usize)>,
        list_inner_area: &'a mut Rect,
    ) -> Self {
        Self {
            events,
            theme,
            selected_range,
            list_inner_area,
        }
    }
}

impl StatefulWidget for EventList<'_> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let app_width = app_width(self.events);
        let block = Block::bordered().title("Events");
        *self.list_inner_area = block.inner(area);

        let items = self
            .events
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let item = ListItem::new(row_line(row, app_width, self.theme));
                if self
                    .selected_range
                    .is_some_and(|(start, end)| index >= start && index <= end)
                {
                    item.style(self.theme.range_row())
                } else {
                    item
                }
            })
            .collect::<Vec<_>>();

        StatefulWidget::render(
            List::new(items)
                .block(block)
                .highlight_symbol("› ")
                .highlight_spacing(HighlightSpacing::Always)
                .highlight_style(self.theme.selected_row()),
            area,
            buf,
            state,
        );
    }
}
