use ratatui::{
    Frame,
    layout::{Constraint, Layout},
};

use super::app::App;
use crate::commands::meetings::ui::{self, status_bar::StatusBarConfig};
use crate::theme::Theme;
use crate::widgets::AppShell;

mod header;
mod transcript;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let elapsed = app.frame_elapsed();
    let theme = Theme::TRANSPARENT;

    let [content_area, status_area] = AppShell::new(&theme).render(frame);
    let [header_area, body_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(3)]).areas(content_area);

    header::draw_header_bar(frame, app, header_area, &theme);

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
        " Notepad ",
    );
    transcript::draw_transcript(frame, app, transcript_area, elapsed, &theme);

    ui::draw_status_bar(
        frame,
        StatusBarConfig {
            mode: app.mode(),
            command_buffer: app.command_buffer(),
            normal_hints: vec![
                (":q", "quit"),
                ("j/k", "scroll"),
                ("i", "notepad"),
                ("G/g", "bottom/top"),
            ],
            insert_hints: vec![
                ("esc", "normal"),
                ("tab", "normal"),
                ("ctrl+z/y", "undo/redo"),
                ("ctrl+u", "clear"),
            ],
            normal_suffix: Some(ratatui::text::Span::styled(
                format!("{} words", app.word_count()),
                theme.muted,
            )),
            insert_suffix: None,
        },
        status_area,
        &theme,
    );
}
