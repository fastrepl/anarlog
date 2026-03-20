use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::commands::meetings::ui::{self, status_bar::StatusBarConfig};
use crate::theme::Theme;
use crate::widgets::{AppShell, InfoLine};

use super::app::App;

pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    let theme = Theme::TRANSPARENT;

    let [content_area, status_area] = AppShell::new(&theme).render(frame);
    let [header_area, body_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(content_area);

    draw_header(frame, app, header_area, &theme);

    if app.loading() {
        let msg = Paragraph::new(Line::from(Span::styled("  Loading...", theme.muted)));
        frame.render_widget(msg, body_area);
    } else if let Some(error) = app.error() {
        let msg = Paragraph::new(Line::from(Span::styled(format!("  {error}"), theme.error)));
        frame.render_widget(msg, body_area);
    } else {
        let [memo_area, transcript_area] = Layout::horizontal([
            Constraint::Percentage(app.notepad_width_percent()),
            Constraint::Percentage(100 - app.notepad_width_percent()),
        ])
        .areas(body_area);

        let memo_focused = app.memo_focused();
        ui::draw_notepad(
            frame,
            app.memo_mut(),
            memo_focused,
            memo_area,
            &theme,
            " Memo ",
        );

        let transcript_focused = app.transcript_focused();
        let (segments, scroll) = app.segments_and_scroll();
        ui::draw_transcript(
            frame,
            segments,
            transcript_focused,
            transcript_area,
            &theme,
            scroll,
            "No transcript",
            None,
            None,
        );
    }

    let mut normal_hints = vec![
        (":q", "quit"),
        (":w", "save"),
        ("j/k", "scroll"),
        ("i", "memo"),
    ];
    if app.memo_dirty() {
        normal_hints.push(("", "[modified]"));
    }

    let normal_suffix = app
        .save_message()
        .map(|msg| Span::styled(msg, theme.status_active));

    let insert_suffix = if app.memo_dirty() {
        Some(Span::styled("[modified]", theme.muted))
    } else {
        None
    };

    ui::draw_status_bar(
        frame,
        StatusBarConfig {
            mode: app.mode(),
            command_buffer: app.command_buffer(),
            normal_hints,
            insert_hints: vec![
                ("esc", "normal"),
                ("tab", "normal"),
                ("ctrl+z/y", "undo/redo"),
            ],
            normal_suffix,
            insert_suffix,
        },
        status_area,
        &theme,
    );
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let title = if app.title().is_empty() {
        "Untitled"
    } else {
        app.title()
    };

    let date = app.created_at();
    let short_date = date.get(..10).unwrap_or(date);

    let info = InfoLine::new(theme)
        .item(Span::styled(title, theme.accent))
        .item(Span::raw(short_date));

    frame.render_widget(info, area);
}
