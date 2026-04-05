use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span, Text},
    widgets::{Block, Paragraph, Widget, Wrap},
};

use crate::{app::DetailTab, event_row::EventRow, theme::Theme};

use super::row::{app_width, row_line, selected_row};

pub(super) struct EventDetails<'a> {
    events: &'a [EventRow],
    selected_index: Option<usize>,
    detail_tab: DetailTab,
    selected_raw_json: Option<&'a str>,
    theme: Theme,
}

impl<'a> EventDetails<'a> {
    pub(super) fn new(
        events: &'a [EventRow],
        selected_index: Option<usize>,
        detail_tab: DetailTab,
        selected_raw_json: Option<&'a str>,
        theme: Theme,
    ) -> Self {
        Self {
            events,
            selected_index,
            detail_tab,
            selected_raw_json,
            theme,
        }
    }
}

impl Widget for EventDetails<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let Some(row) = selected_row(self.events, self.selected_index) else {
            Paragraph::new("No event selected")
                .block(Block::bordered().title("Details"))
                .render(area, buf);
            return;
        };

        let layout = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]);
        let [summary_area, details_area] = area.layout(&layout);
        let app_width = app_width(self.events);

        Paragraph::new(row_line(row, app_width, self.theme))
            .block(Block::bordered().title("Selected Event"))
            .render(summary_area, buf);

        match self.detail_tab {
            DetailTab::Details => {
                let detail_lines = row
                    .details
                    .iter()
                    .map(|detail| {
                        Line::from(vec![
                            Span::styled(format!("{:>14}: ", detail.label), self.theme.label()),
                            Span::raw(detail.value.clone()),
                        ])
                    })
                    .collect::<Vec<_>>();

                Paragraph::new(Text::from(detail_lines))
                    .block(Block::bordered().title("Details"))
                    .wrap(Wrap { trim: false })
                    .render(details_area, buf);
            }
            DetailTab::Raw => {
                let raw = self
                    .selected_raw_json
                    .unwrap_or("{\"error\":\"no record selected\"}");
                Paragraph::new(raw)
                    .block(Block::bordered().title("Raw JSON"))
                    .wrap(Wrap { trim: false })
                    .render(details_area, buf);
            }
        }
    }
}
