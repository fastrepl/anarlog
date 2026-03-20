use hypr_cli_editor::Editor;
use ratatui::{Frame, layout::Rect};

use crate::theme::Theme;

pub(crate) fn draw_notepad(
    frame: &mut Frame,
    editor: &mut Editor<Theme>,
    focused: bool,
    area: Rect,
    theme: &Theme,
    title: &str,
) {
    if area.width < 3 || area.height < 3 {
        return;
    }

    let block = theme.bordered_block(focused).title(title.to_string());
    editor.set_block(block);
    frame.render_widget(&*editor, area);
}
