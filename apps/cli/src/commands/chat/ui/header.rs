use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

use crate::commands::chat::app::App;
use crate::output::format_hhmmss;
use crate::theme::Theme;
use crate::widgets::InfoLine;

pub(super) fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let theme = Theme::DEFAULT;

    let mut info = InfoLine::new(&theme)
        .item(Span::styled("chat", theme.status_active))
        .item(Span::raw(app.model().to_string()));

    if let Some(session) = app.session() {
        info = info.item(Span::raw(format!("session {session}")));
    }

    info = info.item(Span::raw(format_hhmmss(app.elapsed())));

    frame.render_widget(info, area);
}
