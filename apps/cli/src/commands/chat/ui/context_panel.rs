use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

use crate::commands::chat::app::App;
use crate::output::format_hhmmss;
use crate::theme::Theme;

pub(super) fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let theme = Theme::DEFAULT;
    let heading = Style::new().fg(Color::White);

    let title = app.session().unwrap_or("Chat");

    let mut lines = vec![
        Line::from(Span::styled(title, heading)),
        Line::default(),
        Line::from(Span::styled("Model", heading)),
        Line::from(Span::styled(app.model().to_string(), theme.muted)),
        Line::default(),
        Line::from(Span::styled("Elapsed", heading)),
        Line::from(Span::styled(format_hhmmss(app.elapsed()), theme.muted)),
        Line::default(),
        Line::from(Span::styled("Status", heading)),
        Line::from(Span::styled(
            app.status().to_string(),
            if app.streaming() {
                theme.status_active
            } else {
                theme.muted
            },
        )),
    ];

    if let Some(err) = app.last_error() {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled("Error", theme.error)));
        lines.push(Line::from(Span::styled(err.to_string(), theme.error)));
    }

    let block = Block::new()
        .borders(Borders::LEFT)
        .border_style(theme.border)
        .padding(Padding::horizontal(1));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}
