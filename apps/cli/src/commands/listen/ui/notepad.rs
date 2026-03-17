use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders},
};

use crate::commands::listen::app::App;
use crate::theme::Theme;

pub(super) fn draw_notepad(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    if area.width < 3 || area.height < 3 {
        return;
    }

    let border_style = if app.memo_focused() {
        theme.border_focused
    } else {
        theme.border
    };

    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Notepad ");

    app.set_memo_block(block);
    frame.render_widget(app.memo(), area);
}
