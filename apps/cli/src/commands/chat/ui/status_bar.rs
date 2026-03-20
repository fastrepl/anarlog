use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

use crate::commands::chat::app::App;
use crate::theme::Theme;
use crate::widgets::KeyHints;

pub(super) fn draw(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let status_style = if app.last_error().is_some() {
        theme.error
    } else if app.streaming() {
        theme.status_active
    } else {
        theme.muted
    };

    frame.render_widget(
        KeyHints::new(theme)
            .hints(vec![
                ("Enter", "submit"),
                ("Ctrl+C", "quit"),
                ("Ctrl+Up/Down", "scroll"),
            ])
            .suffix(Span::styled(app.status().to_string(), status_style)),
        area,
    );
}
