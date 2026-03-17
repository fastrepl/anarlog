use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders};

use crate::commands::chat::app::App;
use crate::theme::Theme;

pub(super) fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let theme = Theme::DEFAULT;

    let border_style = if app.streaming() {
        theme.border
    } else {
        theme.border_focused
    };

    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(border_style);

    app.input_mut().set_block(block);
    frame.render_widget(app.input(), area);
}
