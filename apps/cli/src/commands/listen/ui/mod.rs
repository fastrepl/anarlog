use ratatui::{
    Frame,
    layout::{Constraint, Layout},
};

use crate::commands::listen::app::App;
use crate::theme::Theme;

mod header;
mod notepad;
mod status_bar;
mod transcript;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let elapsed = app.frame_elapsed();
    let theme = Theme::DEFAULT;
    let [header_area, body_area, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    header::draw_header_bar(frame, app, header_area, &theme);

    let [memo_area, transcript_area] = Layout::horizontal([
        Constraint::Percentage(app.notepad_width_percent()),
        Constraint::Percentage(100 - app.notepad_width_percent()),
    ])
    .areas(body_area);

    notepad::draw_notepad(frame, app, memo_area, &theme);
    transcript::draw_transcript(frame, app, transcript_area, elapsed, &theme);
    status_bar::draw_status_bar(frame, app, status_area, &theme);
}
