use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{ListItem, Paragraph};

use crate::theme::Theme;
use crate::widgets::{CenteredDialog, KeyHints, SelectList};

use super::app::App;

pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    let theme = Theme::DEFAULT;

    let inner = CenteredDialog::new("Models", &theme).render(frame);

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
    } else if app.models().is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("  No models found", theme.muted))),
            content_area,
        );
    } else {
        draw_list(frame, app, content_area, &theme);
    }

    let hints = vec![("↑/↓", "navigate"), ("Esc", "back")];
    frame.render_widget(KeyHints::new(&theme).hints(hints), status_area);
}

fn draw_list(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let row_data: Vec<_> = app
        .models()
        .iter()
        .map(|row| {
            (
                row.active,
                row.name.clone(),
                row.kind.clone(),
                row.status.clone(),
            )
        })
        .collect();

    let items: Vec<ListItem> = row_data
        .iter()
        .map(|(active, name, kind, status)| {
            let active_marker = if *active { "* " } else { "  " };

            let status_style = match status.as_str() {
                "downloaded" => theme.status_active,
                "not-downloaded" => theme.status_degraded,
                "unavailable" => theme.muted,
                "error" => theme.error,
                _ => theme.muted,
            };

            ListItem::new(Line::from(vec![
                Span::raw(active_marker),
                Span::styled(name.as_str(), Style::default()),
                Span::raw("  "),
                Span::styled(kind.as_str(), theme.muted),
                Span::raw("  "),
                Span::styled(status.as_str(), status_style),
            ]))
        })
        .collect();

    frame.render_stateful_widget(SelectList::new(items, theme), area, app.list_state_mut());
}
