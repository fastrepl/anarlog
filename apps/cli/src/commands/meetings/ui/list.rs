use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{ListItem, Paragraph};

use crate::theme::Theme;
use crate::widgets::{CenteredDialog, KeyHints, SelectList};

use super::super::app::App;

pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    let theme = Theme::DEFAULT;

    let inner = CenteredDialog::new("Meetings", &theme).render(frame);

    let [content_area, status_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);

    if app.loading() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("  Loading...", theme.muted))),
            content_area,
        );
    } else if let Some(error) = app.error() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(format!("  {error}"), theme.error))),
            content_area,
        );
    } else {
        draw_content(frame, app, content_area, &theme);
    }

    let hints = vec![("↑/↓", "navigate"), ("Enter", "select"), ("Esc", "back")];
    frame.render_widget(KeyHints::new(&theme).hints(hints), status_area);
}

fn draw_content(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let events_height = events_section_height(app);

    if events_height == 0 {
        draw_meetings_list(frame, app, area, theme);
        return;
    }

    let [events_area, meetings_area] =
        Layout::vertical([Constraint::Length(events_height), Constraint::Min(1)]).areas(area);

    draw_events_section(frame, app, events_area, theme);
    draw_meetings_list(frame, app, meetings_area, theme);
}

fn events_section_height(app: &App) -> u16 {
    match app.calendar_configured() {
        Some(false) => 3,
        Some(true) if app.events().is_empty() => 3,
        Some(true) => 2 + app.events().len() as u16,
        None => 0,
    }
}

fn draw_events_section(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let mut lines = Vec::new();

    match app.calendar_configured() {
        Some(false) => {
            lines.push(Line::from(Span::styled(
                "  Connect your calendar in the desktop app to see upcoming events.",
                theme.muted,
            )));
        }
        Some(true) if app.events().is_empty() => {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Upcoming", Style::new().add_modifier(Modifier::BOLD)),
            ]));
            lines.push(Line::from(Span::styled(
                "  No upcoming events",
                theme.muted,
            )));
        }
        Some(true) => {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Upcoming", Style::new().add_modifier(Modifier::BOLD)),
            ]));
            for event in app.events() {
                let time_range = format_event_time(&event.started_at, &event.ended_at);
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(time_range, theme.muted),
                    Span::raw("  "),
                    Span::raw(&event.title),
                ]));
            }
        }
        None => {}
    }

    lines.push(Line::from(""));

    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_meetings_list(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    if app.meetings().is_empty() {
        let lines = vec![
            Line::from(vec![
                Span::raw("  "),
                Span::styled("Past Meetings", Style::new().add_modifier(Modifier::BOLD)),
            ]),
            Line::from(Span::styled("  No meetings found", theme.muted)),
        ];
        frame.render_widget(Paragraph::new(lines), area);
        return;
    }

    let [header_area, list_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("  "),
            Span::styled("Past Meetings", Style::new().add_modifier(Modifier::BOLD)),
        ])),
        header_area,
    );

    let items: Vec<ListItem> = app
        .meetings()
        .iter()
        .map(|s| {
            let title = s.title.as_deref().unwrap_or("Untitled").to_string();
            let date = s.created_at.clone();
            let short_date = date.get(..10).unwrap_or(&date).to_string();
            ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(short_date, theme.muted),
                Span::raw("  "),
                Span::styled(title, Style::new().add_modifier(Modifier::BOLD)),
            ]))
        })
        .collect();

    frame.render_stateful_widget(
        SelectList::new(items, theme),
        list_area,
        app.list_state_mut(),
    );
}

fn format_event_time(started_at: &str, ended_at: &str) -> String {
    let start = extract_time(started_at);
    let end = extract_time(ended_at);
    format!("{start} - {end}")
}

fn extract_time(datetime: &str) -> &str {
    if let Some(t_pos) = datetime.find('T') {
        let time_part = &datetime[t_pos + 1..];
        time_part.get(..5).unwrap_or(time_part)
    } else if datetime.len() > 10 {
        datetime.get(11..16).unwrap_or(datetime)
    } else {
        datetime
    }
}
