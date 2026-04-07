use std::ops::RangeInclusive;

use hypr_activity_capture::Capabilities;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span, Text},
    widgets::{
        Block, HighlightSpacing, List, ListItem, ListState, Paragraph, StatefulWidget, Widget, Wrap,
    },
};

use crate::{
    app::{DetailTab, SessionStats, View},
    event_row::EventRow,
    formatting::{compact, format_timestamp},
    options::Options,
    theme::Theme,
};

pub(crate) const APP_PREVIEW_LIMIT: usize = 18;
pub(crate) const APP_COLUMN_MIN_WIDTH: usize = 14;
pub(crate) const APP_COLUMN_MAX_WIDTH: usize = 22;
pub(crate) const TITLE_PREVIEW_LIMIT: usize = 56;
pub(crate) const URL_PREVIEW_LIMIT: usize = 72;
pub(crate) const TEXT_PREVIEW_LIMIT: usize = 40;
pub(crate) const DIFF_PREVIEW_LIMIT: usize = 28;
pub(crate) const STATUS_COLUMN_WIDTH: usize = 6;

pub(crate) struct ScreenData<'a> {
    pub(crate) options: &'a Options,
    pub(crate) capabilities: Capabilities,
    pub(crate) theme: Theme,
    pub(crate) view: View,
    pub(crate) detail_tab: DetailTab,
    pub(crate) runtime_summary: String,
    pub(crate) policy_label: String,
    pub(crate) browser_policy_label: String,
    pub(crate) session_stats: SessionStats,
    pub(crate) events: &'a [EventRow],
    pub(crate) selected_index: Option<usize>,
    pub(crate) selected_range: Option<RangeInclusive<usize>>,
    pub(crate) selection_summary: Option<String>,
    pub(crate) status_message: Option<String>,
    pub(crate) selected_raw_json: Option<String>,
    pub(crate) list_state: &'a mut ListState,
    pub(crate) list_inner_area: &'a mut Rect,
}

pub(crate) fn render(
    terminal: &mut DefaultTerminal,
    mut screen: ScreenData<'_>,
) -> std::io::Result<()> {
    terminal.draw(|frame| render_frame(frame, &mut screen))?;
    Ok(())
}

fn render_frame(frame: &mut Frame, screen: &mut ScreenData<'_>) {
    let layout = Layout::vertical([
        Constraint::Length(7),
        Constraint::Min(1),
        Constraint::Length(1),
    ]);
    let [header_area, body_area, footer_area] = frame.area().layout(&layout);

    render_header(screen, header_area, frame);

    match screen.view {
        View::List => render_list(screen, body_area, frame),
        View::Details => render_details(screen, body_area, frame),
    }

    Paragraph::new(footer_text(screen.view, screen.detail_tab))
        .render(footer_area, frame.buffer_mut());
}

fn render_header(screen: &ScreenData<'_>, area: Rect, frame: &mut Frame) {
    let selection = screen.selection_summary.as_deref().unwrap_or("single row");
    let export = screen.status_message.as_deref().unwrap_or(
        "y copy JSON  s save selection/current  S save full session  r raw JSON  v range",
    );

    let lines = vec![
        Line::from(vec![
            Span::styled("activity-capture", screen.theme.title()),
            Span::raw(format!("  poll={}ms", screen.options.poll_ms)),
            Span::raw(format!("  runtime={}", screen.runtime_summary)),
            Span::raw(match screen.view {
                View::List => "  view=list",
                View::Details => "  view=details",
            }),
        ]),
        Line::from(vec![
            Span::styled("session", screen.theme.label()),
            Span::raw(format!(
                "  events={}  apps={}  focus={}  update={}  idle={}  snap={}  selected={selection}",
                screen.session_stats.event_count,
                screen.session_stats.distinct_apps,
                screen.session_stats.focus_count,
                screen.session_stats.update_count,
                screen.session_stats.idle_count,
                screen.session_stats.screenshot_count,
            )),
        ]),
        Line::from(vec![
            Span::styled("policy", screen.theme.label()),
            Span::raw(format!("  {}", screen.policy_label)),
        ]),
        Line::from(vec![
            Span::styled("browser", screen.theme.label()),
            Span::raw(format!("  {}", screen.browser_policy_label)),
        ]),
        Line::from(vec![
            Span::styled("capabilities", screen.theme.label()),
            Span::raw(format!(
                "  watch={} text={} url={} ax={}",
                yes_no(screen.capabilities.can_watch),
                yes_no(screen.capabilities.can_capture_visible_text),
                yes_no(screen.capabilities.can_capture_browser_url),
                yes_no(screen.capabilities.requires_accessibility_permission),
            )),
        ]),
        Line::from(vec![
            Span::styled("export", screen.theme.label()),
            Span::raw(format!("  {export}")),
        ]),
    ];

    Paragraph::new(Text::from(lines))
        .block(Block::bordered().title("Session"))
        .render(area, frame.buffer_mut());
}

fn render_list(screen: &mut ScreenData<'_>, area: Rect, frame: &mut Frame) {
    let app_width = app_width(screen.events);
    let block = Block::bordered().title("Events");
    *screen.list_inner_area = block.inner(area);

    let items = screen
        .events
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let item = ListItem::new(row_line(row, app_width, screen.theme));
            if screen
                .selected_range
                .as_ref()
                .is_some_and(|range| index >= *range.start() && index <= *range.end())
            {
                item.style(screen.theme.range_row())
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
            .highlight_style(screen.theme.selected_row()),
        area,
        frame.buffer_mut(),
        screen.list_state,
    );
}

fn render_details(screen: &mut ScreenData<'_>, area: Rect, frame: &mut Frame) {
    *screen.list_inner_area = Rect::default();

    let Some(row) = selected_row(screen.events, screen.selected_index) else {
        Paragraph::new("No event selected")
            .block(Block::bordered().title("Details"))
            .render(area, frame.buffer_mut());
        return;
    };

    let layout = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]);
    let [summary_area, details_area] = area.layout(&layout);
    let app_width = app_width(screen.events);

    Paragraph::new(row_line(row, app_width, screen.theme))
        .block(Block::bordered().title("Selected Event"))
        .render(summary_area, frame.buffer_mut());

    match screen.detail_tab {
        DetailTab::Details => {
            let detail_lines = row
                .details
                .iter()
                .map(|detail| {
                    Line::from(vec![
                        Span::styled(format!("{:>14}: ", detail.label), screen.theme.label()),
                        Span::raw(detail.value.clone()),
                    ])
                })
                .collect::<Vec<_>>();

            Paragraph::new(Text::from(detail_lines))
                .block(Block::bordered().title("Details"))
                .wrap(Wrap { trim: false })
                .render(details_area, frame.buffer_mut());
        }
        DetailTab::Raw => {
            let raw = screen
                .selected_raw_json
                .as_deref()
                .unwrap_or("{\"error\":\"no record selected\"}");
            Paragraph::new(raw)
                .block(Block::bordered().title("Raw JSON"))
                .wrap(Wrap { trim: false })
                .render(details_area, frame.buffer_mut());
        }
    }
}

fn footer_text(view: View, detail_tab: DetailTab) -> &'static str {
    match view {
        View::List => {
            "q/Esc quit  •  ↑↓ or j/k move  •  g/G home/end  •  Enter details  •  r raw JSON  •  v toggle range"
        }
        View::Details => match detail_tab {
            DetailTab::Details => {
                "q quit  •  Esc/←/h back  •  ↑↓ or j/k move  •  Tab/r raw JSON  •  s save current/range"
            }
            DetailTab::Raw => {
                "q quit  •  Esc/←/h back  •  ↑↓ or j/k move  •  Tab/d details  •  s save current/range"
            }
        },
    }
}

fn app_width(events: &[EventRow]) -> usize {
    events
        .iter()
        .map(|row| compact(&row.app_name, APP_PREVIEW_LIMIT).chars().count())
        .fold(APP_COLUMN_MIN_WIDTH, usize::max)
        .min(APP_COLUMN_MAX_WIDTH)
}

fn row_line(row: &EventRow, app_width: usize, theme: Theme) -> Line<'static> {
    let app_name = compact(&row.app_name, APP_PREVIEW_LIMIT);
    let status = row.status.label();

    Line::from(vec![
        Span::styled(format_timestamp(row.captured_at), theme.timestamp()),
        Span::raw("  "),
        Span::styled(format!("{app_name:app_width$}"), theme.app(&row.app_name)),
        Span::raw("  "),
        Span::styled(
            format!("{status:STATUS_COLUMN_WIDTH$}"),
            theme.status(row.status),
        ),
        Span::raw("  "),
        Span::styled(format!("[{}]", row.context), theme.label()),
        Span::raw("  "),
        Span::raw(row.summary.clone()),
    ])
}

fn selected_row(events: &[EventRow], selected_index: Option<usize>) -> Option<&EventRow> {
    selected_index.and_then(|index| events.get(index))
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
