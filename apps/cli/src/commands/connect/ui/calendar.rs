use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme::Theme;
use crate::widgets::{
    MultiSelect, MultiSelectEntry, MultiSelectState, PermissionButton, PermissionStatus,
};

use super::super::app::App;
use super::super::app::CalendarPhase;
use super::super::runtime::CalendarPermissionState;

pub(crate) fn draw(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    match app.calendar().phase() {
        CalendarPhase::Permission => draw_permission(frame, app, area, theme),
        CalendarPhase::Select => draw_select(frame, app, area, theme),
    }
}

fn draw_permission(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let [label_area, button_area] =
        Layout::vertical([Constraint::Length(2), Constraint::Min(1)]).areas(area);

    frame.render_widget(
        Paragraph::new("  Calendar access is required to list your calendars."),
        label_area,
    );

    let status = match app.calendar().auth_status() {
        None => PermissionStatus::Checking,
        Some(CalendarPermissionState::NotDetermined) => PermissionStatus::NotRequested,
        Some(CalendarPermissionState::Authorized) => PermissionStatus::Authorized,
        Some(CalendarPermissionState::Denied) => PermissionStatus::Denied,
    };

    frame.render_widget(PermissionButton::new(status, theme), button_area);
}

fn draw_select(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let cal = app.calendar();

    if cal.loading() {
        frame.render_widget(Span::styled("  Loading calendars...", theme.muted), area);
        return;
    }

    if let Some(err) = cal.error() {
        frame.render_widget(Span::styled(format!("  Error: {err}"), theme.error), area);
        return;
    }

    if cal.items().is_empty() {
        frame.render_widget(Span::styled("  No calendars found", theme.muted), area);
        return;
    }

    let items = cal.items();
    let enabled = cal.enabled();
    let mut entries: Vec<MultiSelectEntry> = Vec::new();
    let mut current_source = "";

    for (i, item) in items.iter().enumerate() {
        if item.source.as_str() != current_source {
            if !current_source.is_empty() {
                entries.push(MultiSelectEntry::Group(Line::from("")));
            }
            entries.push(MultiSelectEntry::Group(Line::from(Span::styled(
                item.source.clone(),
                theme.muted,
            ))));
            current_source = &item.source;
        }

        let checked = enabled.get(i).copied().unwrap_or(false);
        let color_dot = parse_hex_color(&item.color);
        let label = Line::from(vec![
            Span::styled("\u{25CF} ", Style::new().fg(color_dot)),
            Span::raw(item.name.clone()),
        ]);
        entries.push(MultiSelectEntry::Item { checked, label });
    }

    let data_idx = app.calendar_mut().list_state_mut().selected().unwrap_or(0);
    let mut state = MultiSelectState::new(data_idx);

    frame.render_stateful_widget(MultiSelect::new(entries, theme), area, &mut state);
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
