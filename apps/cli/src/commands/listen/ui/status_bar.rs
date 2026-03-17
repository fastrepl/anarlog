use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::commands::listen::app::{App, Mode};
use crate::theme::Theme;
use crate::widgets::KeyHints;

pub(super) fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    match app.mode() {
        Mode::Command => {
            let cmd_display = format!(":{}", app.command_buffer());
            let line = Line::from(vec![
                Span::styled(" COMMAND ", Style::new().fg(Color::Black).bg(Color::Yellow)),
                Span::raw(" "),
                Span::styled(cmd_display, Style::new().fg(Color::White)),
                Span::styled("\u{2588}", Style::new().fg(Color::Gray)),
            ]);
            frame.render_widget(Paragraph::new(line), area);
        }
        Mode::Insert => {
            frame.render_widget(
                KeyHints::new(theme)
                    .badge(" INSERT ", Style::new().fg(Color::Black).bg(Color::Green))
                    .hints(vec![
                        ("esc", "normal"),
                        ("tab", "normal"),
                        ("ctrl+z/y", "undo/redo"),
                        ("ctrl+u", "clear"),
                    ]),
                area,
            );
        }
        Mode::Normal => {
            frame.render_widget(
                KeyHints::new(theme)
                    .badge(" NORMAL ", Style::new().fg(Color::Black).bg(Color::Cyan))
                    .hints(vec![
                        (":q", "quit"),
                        ("j/k", "scroll"),
                        ("i", "notepad"),
                        ("G/g", "bottom/top"),
                    ])
                    .suffix(Span::styled(
                        format!("{} words", app.word_count()),
                        theme.muted,
                    )),
                area,
            );
        }
    }
}
