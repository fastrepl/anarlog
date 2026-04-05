use hypr_activity_capture::Capabilities;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    widgets::{ListState, StatefulWidget, Widget},
};
use std::ops::RangeInclusive;

use crate::{
    app::{DetailTab, SessionStats, View},
    event_row::EventRow,
    options::Options,
    theme::Theme,
};

use super::{details::EventDetails, footer::Footer, header::SessionHeader, list::EventList};

pub(crate) struct ActivityScreen<'a> {
    options: &'a Options,
    capabilities: Capabilities,
    theme: Theme,
    view: View,
    detail_tab: DetailTab,
    runtime_summary: &'a str,
    policy_label: &'a str,
    browser_policy_label: &'a str,
    session_stats: SessionStats,
    events: &'a [EventRow],
    selected_index: Option<usize>,
    selected_range: Option<RangeInclusive<usize>>,
    selection_summary: Option<&'a str>,
    status_message: Option<&'a str>,
    selected_raw_json: Option<&'a str>,
    list_state: &'a mut ListState,
    list_inner_area: &'a mut Rect,
}

impl<'a> ActivityScreen<'a> {
    pub(crate) fn new(
        options: &'a Options,
        capabilities: Capabilities,
        theme: Theme,
        view: View,
        detail_tab: DetailTab,
        runtime_summary: &'a str,
        policy_label: &'a str,
        browser_policy_label: &'a str,
        session_stats: SessionStats,
        events: &'a [EventRow],
        selected_index: Option<usize>,
        selected_range: Option<RangeInclusive<usize>>,
        selection_summary: Option<&'a str>,
        status_message: Option<&'a str>,
        selected_raw_json: Option<&'a str>,
        list_state: &'a mut ListState,
        list_inner_area: &'a mut Rect,
    ) -> Self {
        Self {
            options,
            capabilities,
            theme,
            view,
            detail_tab,
            runtime_summary,
            policy_label,
            browser_policy_label,
            session_stats,
            events,
            selected_index,
            selected_range,
            selection_summary,
            status_message,
            selected_raw_json,
            list_state,
            list_inner_area,
        }
    }
}

impl Widget for ActivityScreen<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::vertical([
            Constraint::Length(7),
            Constraint::Min(1),
            Constraint::Length(1),
        ]);
        let [header_area, body_area, footer_area] = area.layout(&layout);

        SessionHeader::new(
            self.options,
            self.capabilities,
            self.theme,
            self.view,
            self.runtime_summary,
            self.policy_label,
            self.browser_policy_label,
            self.session_stats,
            self.selection_summary,
            self.status_message,
        )
        .render(header_area, buf);

        match self.view {
            View::List => EventList::new(
                self.events,
                self.theme,
                self.selected_range
                    .as_ref()
                    .map(|range| (*range.start(), *range.end())),
                self.list_inner_area,
            )
            .render(body_area, buf, self.list_state),
            View::Details => {
                *self.list_inner_area = Rect::default();
                EventDetails::new(
                    self.events,
                    self.selected_index,
                    self.detail_tab,
                    self.selected_raw_json,
                    self.theme,
                )
                .render(body_area, buf);
            }
        }

        Footer::new(self.view, self.detail_tab).render(footer_area, buf);
    }
}
