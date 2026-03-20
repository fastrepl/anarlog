use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, ListItem, ListState, Paragraph};

use crate::theme::Theme;
use crate::widgets::{KeyHints, SelectList};

use super::app::{App, Pane};

pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    let theme = Theme::DEFAULT;

    frame.render_widget(
        Block::default().style(Style::new().bg(theme.overlay_bg)),
        frame.area(),
    );

    let area = wide_area(frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::default().style(Style::new().bg(theme.dialog_bg)),
        area,
    );

    let padded = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };

    let [title_area, _gap, content_area, hints_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(padded);

    let [title_left, title_right] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(3)]).areas(title_area);
    frame.render_widget(
        Span::styled(
            "Timeline",
            Style::new()
                .fg(theme.dialog_title_fg)
                .add_modifier(Modifier::BOLD),
        ),
        title_left,
    );
    frame.render_widget(Span::styled("esc", theme.muted), title_right);

    if app.loading_contacts() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("Loading...", theme.muted))),
            content_area,
        );
    } else if let Some(error) = app.error() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(format!("  {error}"), theme.error))),
            content_area,
        );
    } else {
        draw_columns(frame, app, content_area, &theme);
    }

    let hints = vec![
        ("Tab", "pane"),
        ("\u{2191}/\u{2193}", "navigate"),
        ("Enter", "select"),
        ("Esc", "back"),
    ];
    frame.render_widget(KeyHints::new(&theme).hints(hints), hints_area);
}

fn draw_columns(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let [orgs_area, humans_area, timeline_area] = Layout::horizontal([
        Constraint::Percentage(25),
        Constraint::Percentage(30),
        Constraint::Percentage(45),
    ])
    .areas(area);

    draw_orgs(frame, app, orgs_area, theme);
    draw_humans(frame, app, humans_area, theme);
    draw_entries(frame, app, timeline_area, theme);
}

enum ColumnBody<'a> {
    Loading,
    Empty(&'a str),
    Items(Vec<ListItem<'a>>, &'a mut ListState),
}

fn render_column(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    focused: bool,
    body: ColumnBody<'_>,
    theme: &Theme,
) {
    let [header, list_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);

    let header_style = if focused { theme.accent } else { theme.muted };
    frame.render_widget(
        Paragraph::new(Span::styled(format!(" {title}"), header_style)),
        header,
    );

    match body {
        ColumnBody::Loading => {
            frame.render_widget(
                Paragraph::new(Span::styled("  Loading...", theme.muted)),
                list_area,
            );
        }
        ColumnBody::Empty(msg) => {
            frame.render_widget(Paragraph::new(Span::styled(msg, theme.muted)), list_area);
        }
        ColumnBody::Items(items, state) => {
            frame.render_stateful_widget(SelectList::new(items, theme), list_area, state);
        }
    }
}

fn draw_orgs(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let mut items: Vec<ListItem> = vec![ListItem::new(Line::from(vec![
        Span::raw("  "),
        Span::styled("All", Style::new().add_modifier(Modifier::ITALIC)),
    ]))];
    for org in app.orgs() {
        items.push(ListItem::new(Line::from(vec![
            Span::raw("  "),
            Span::raw(org.name.clone()),
        ])));
    }

    render_column(
        frame,
        area,
        "Organizations",
        app.pane() == Pane::Orgs,
        ColumnBody::Items(items, app.org_state_mut()),
        theme,
    );
}

fn draw_humans(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let focused = app.pane() == Pane::Humans;
    let items: Vec<ListItem> = app
        .filtered_humans()
        .iter()
        .map(|h| {
            let name = if h.name.is_empty() {
                "Unnamed"
            } else {
                &h.name
            };
            let mut parts = vec![
                Span::raw("  "),
                Span::styled(name.to_string(), Style::new().add_modifier(Modifier::BOLD)),
            ];
            if !h.job_title.is_empty() {
                parts.push(Span::raw(" "));
                parts.push(Span::styled(h.job_title.clone(), theme.muted));
            }
            ListItem::new(Line::from(parts))
        })
        .collect();

    let body = if items.is_empty() {
        ColumnBody::Empty("  No humans")
    } else {
        ColumnBody::Items(items, app.human_state_mut())
    };
    render_column(frame, area, "Humans", focused, body, theme);
}

fn draw_entries(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let focused = app.pane() == Pane::Timeline;

    if app.loading_entries() {
        render_column(frame, area, "Activity", focused, ColumnBody::Loading, theme);
        return;
    }

    let items: Vec<ListItem> = app
        .entries()
        .iter()
        .map(|e| {
            let date = e.happened_at.get(..10).unwrap_or(&e.happened_at);
            let badge = match e.source_type.as_str() {
                "meeting" => "mtg",
                "slack" => "slk",
                "note" => "note",
                _ => "?",
            };
            ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(date.to_string(), theme.muted),
                Span::raw("  "),
                Span::styled(format!("[{badge}]"), theme.accent),
                Span::raw(" "),
                Span::raw(e.title.clone()),
            ]))
        })
        .collect();

    let body = if items.is_empty() {
        let msg = if app.selected_human().is_some() {
            "  No activity"
        } else {
            "  Select a human"
        };
        ColumnBody::Empty(msg)
    } else {
        ColumnBody::Items(items, app.entry_state_mut())
    };
    render_column(frame, area, "Activity", focused, body, theme);
}

fn wide_area(area: Rect) -> Rect {
    let width = area.width.saturating_mul(4) / 5;
    let width = width.clamp(60, 140);
    let height = area.height.saturating_mul(4) / 5;
    let height = height.clamp(16, 50);
    let [v] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    let [h] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(v);
    h
}
