use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

use crate::commands::chat::app::App;
use crate::theme::Theme;
use crate::widgets::KeyHints;

pub(super) fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let theme = Theme::DEFAULT;

    let status_style = if app.last_error().is_some() {
        theme.error
    } else if app.status().starts_with("Streaming") {
        theme.status_active
    } else {
        theme.muted
    };

    frame.render_widget(
        KeyHints::new(&theme)
            .hints(vec![
                ("Enter", "submit"),
                ("Ctrl+C", "quit"),
                ("Ctrl+Up/Down", "scroll"),
            ])
            .suffix(Span::styled(app.status().to_string(), status_style)),
        area,
    );
}
