use hypr_activity_capture::Capabilities;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span, Text},
    widgets::{Block, Paragraph, Widget},
};

use crate::{
    app::{SessionStats, View},
    options::Options,
    theme::Theme,
};

pub(super) struct SessionHeader<'a> {
    options: &'a Options,
    capabilities: Capabilities,
    theme: Theme,
    view: View,
    runtime_summary: &'a str,
    policy_label: &'a str,
    browser_policy_label: &'a str,
    session_stats: SessionStats,
    selection_summary: Option<&'a str>,
    status_message: Option<&'a str>,
}

impl<'a> SessionHeader<'a> {
    pub(super) fn new(
        options: &'a Options,
        capabilities: Capabilities,
        theme: Theme,
        view: View,
        runtime_summary: &'a str,
        policy_label: &'a str,
        browser_policy_label: &'a str,
        session_stats: SessionStats,
        selection_summary: Option<&'a str>,
        status_message: Option<&'a str>,
    ) -> Self {
        Self {
            options,
            capabilities,
            theme,
            view,
            runtime_summary,
            policy_label,
            browser_policy_label,
            session_stats,
            selection_summary,
            status_message,
        }
    }
}

impl Widget for SessionHeader<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let selection = self.selection_summary.unwrap_or("single row");
        let export = self.status_message.unwrap_or(
            "y copy JSON  s save selection/current  S save full session  r raw JSON  v range",
        );

        let lines = vec![
            Line::from(vec![
                Span::styled("activity-capture", self.theme.title()),
                Span::raw(format!("  poll={}ms", self.options.poll_ms)),
                Span::raw(format!("  runtime={}", self.runtime_summary)),
                Span::raw(match self.view {
                    View::List => "  view=list",
                    View::Details => "  view=details",
                }),
            ]),
            Line::from(vec![
                Span::styled("session", self.theme.label()),
                Span::raw(format!(
                    "  events={}  apps={}  focus={}  update={}  idle={}  snap={}  selected={selection}",
                    self.session_stats.event_count,
                    self.session_stats.distinct_apps,
                    self.session_stats.focus_count,
                    self.session_stats.update_count,
                    self.session_stats.idle_count,
                    self.session_stats.screenshot_count,
                )),
            ]),
            Line::from(vec![
                Span::styled("policy", self.theme.label()),
                Span::raw(format!("  {}", self.policy_label)),
            ]),
            Line::from(vec![
                Span::styled("browser", self.theme.label()),
                Span::raw(format!("  {}", self.browser_policy_label)),
            ]),
            Line::from(vec![
                Span::styled("capabilities", self.theme.label()),
                Span::raw(format!(
                    "  watch={} text={} url={} ax={}",
                    yes_no(self.capabilities.can_watch),
                    yes_no(self.capabilities.can_capture_visible_text),
                    yes_no(self.capabilities.can_capture_browser_url),
                    yes_no(self.capabilities.requires_accessibility_permission),
                )),
            ]),
            Line::from(vec![
                Span::styled("export", self.theme.label()),
                Span::raw(format!("  {export}")),
            ]),
        ];

        Paragraph::new(Text::from(lines))
            .block(Block::bordered().title("Session"))
            .render(area, buf);
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
