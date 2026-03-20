use ratatui::Frame;
use ratatui::layout::Rect;

use crate::commands::chat::app::App;
use crate::theme::Theme;

pub(super) fn draw(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let block = theme.bordered_block(!app.streaming());
    app.input_mut().set_block(block);
    frame.render_widget(app.input(), area);
}
