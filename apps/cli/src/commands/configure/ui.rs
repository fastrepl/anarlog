use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;

use crate::theme::Theme;
use crate::widgets::{CenteredDialog, KeyHints, MultiSelect, MultiSelectEntry, SelectList};

use super::app::{App, ProviderTab, Tab};
use super::runtime::CalendarPermissionState;

const THEME: Theme = Theme::DEFAULT;

pub fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let content = CenteredDialog::new("Configure", &THEME).render(frame);

    let [tabs_area, _gap, content_area, _, hints_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(content);

    draw_tabs(frame, app, tabs_area);

    match app.tab {
        Tab::Stt => draw_provider_content(frame, &mut app.stt, content_area),
        Tab::Llm => draw_provider_content(frame, &mut app.llm, content_area),
        Tab::Calendar => draw_calendar_content(frame, app, content_area),
    }

    let cal_has_items = app.cal_permission == Some(CalendarPermissionState::Authorized)
        && !app.calendars.is_empty();
    let hints = match app.tab {
        Tab::Stt | Tab::Llm => KeyHints::new(&THEME).hints(vec![
            ("\u{2190}\u{2192}", "tab"),
            ("\u{2191}\u{2193}", "navigate"),
            ("Enter", "confirm"),
            ("Esc", "quit"),
        ]),
        Tab::Calendar if cal_has_items => KeyHints::new(&THEME).hints(vec![
            ("\u{2190}\u{2192}", "tab"),
            ("\u{2191}\u{2193}", "navigate"),
            ("Space", "toggle"),
            ("Enter", "save"),
            ("Esc", "quit"),
        ]),
        Tab::Calendar => {
            KeyHints::new(&THEME).hints(vec![("\u{2190}\u{2192}", "tab"), ("Esc", "quit")])
        }
    };
    frame.render_widget(hints, hints_area);
}

fn draw_tabs(frame: &mut ratatui::Frame, app: &App, area: ratatui::layout::Rect) {
    let mut spans: Vec<Span> = Vec::new();
    for (i, tab) in Tab::ALL.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        if *tab == app.tab {
            spans.push(Span::styled(
                format!(" {} ", tab.title()),
                Style::new()
                    .fg(THEME.bg)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(tab.title(), THEME.muted));
        }
    }
    frame.render_widget(Line::from(spans), area);
}

fn draw_provider_content(
    frame: &mut ratatui::Frame,
    pt: &mut ProviderTab,
    area: ratatui::layout::Rect,
) {
    if pt.providers.is_empty() {
        let [label_area, _, msg_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(area);

        let label = Line::from(Span::styled(
            "Provider",
            Style::new().add_modifier(Modifier::BOLD),
        ));
        frame.render_widget(label, label_area);

        let msg = Line::from(Span::styled(
            "No providers configured. Run `char connect` first.",
            THEME.muted,
        ));
        frame.render_widget(msg, msg_area);
        return;
    }

    let [label_area, current_area, _gap, list_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);

    let label = Line::from(Span::styled(
        "Provider",
        Style::new().add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(label, label_area);

    if let Some(cur) = &pt.current {
        let current_line = Line::from(vec![
            Span::raw("Current: "),
            Span::styled(cur.as_str(), THEME.status_active),
        ]);
        frame.render_widget(current_line, current_area);
    }

    let items: Vec<ListItem> = pt
        .providers
        .iter()
        .map(|p| {
            let marker = if pt.current.as_deref() == Some(p.as_str()) {
                "\u{2713} "
            } else {
                "  "
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::new().fg(Color::Green)),
                Span::raw(p.as_str()),
            ]))
        })
        .collect();

    let list = SelectList::new(items, &THEME);
    frame.render_stateful_widget(list, list_area, &mut pt.list_state);
}

fn draw_calendar_content(frame: &mut ratatui::Frame, app: &mut App, area: ratatui::layout::Rect) {
    let authorized = app.cal_permission == Some(CalendarPermissionState::Authorized);

    if !authorized {
        let [label_area, _, msg_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(area);

        let label = Line::from(Span::styled(
            "Apple Calendar",
            Style::new().add_modifier(Modifier::BOLD),
        ));
        frame.render_widget(label, label_area);

        let msg = Line::from(Span::styled(
            "Permission not granted. Run `char connect` to set up calendar access.",
            THEME.muted,
        ));
        frame.render_widget(msg, msg_area);
        return;
    }

    if app.calendars.is_empty() {
        let [label_area, _, msg_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(area);

        let label = Line::from(Span::styled(
            "Apple Calendar",
            Style::new().add_modifier(Modifier::BOLD),
        ));
        frame.render_widget(label, label_area);

        let msg = Line::from(Span::styled("No calendars found.", THEME.muted));
        frame.render_widget(msg, msg_area);
        return;
    }

    let [label_area, _, cal_list_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);

    let label = Line::from(Span::styled(
        "Apple Calendar",
        Style::new().add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(label, label_area);

    let mut current_source: Option<&str> = None;
    let mut entries: Vec<MultiSelectEntry> = Vec::new();

    for cal in &app.calendars {
        let source = cal.source.as_str();
        if current_source != Some(source) {
            if current_source.is_some() {
                entries.push(MultiSelectEntry::Group(Line::from("")));
            }
            current_source = Some(source);
            entries.push(MultiSelectEntry::Group(Line::from(Span::styled(
                source,
                THEME.muted,
            ))));
        }
        let color_dot = parse_hex_color(&cal.color);
        let label = Line::from(vec![
            Span::styled("\u{25CF} ", Style::new().fg(color_dot)),
            Span::raw(cal.name.as_str()),
        ]);
        entries.push(MultiSelectEntry::Item {
            checked: cal.enabled,
            label,
        });
    }

    let mut ms_state = crate::widgets::MultiSelectState::new(app.cal_cursor);
    let ms = MultiSelect::new(entries, &THEME);
    frame.render_stateful_widget(ms, cal_list_area, &mut ms_state);
}

fn parse_hex_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() >= 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return Color::Rgb(r, g, b);
        }
    }
    Color::White
}
