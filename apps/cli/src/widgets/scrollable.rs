pub use tui_widgets::scrollview::ScrollViewState;

use ratatui::{
    Frame,
    layout::{Rect, Size},
    text::Line,
    widgets::{Block, Paragraph},
};
use tui_widgets::scrollview::ScrollView;

pub fn render_scrollable(
    frame: &mut Frame,
    lines: Vec<Line<'_>>,
    block: Option<Block<'_>>,
    area: Rect,
    state: &mut ScrollViewState,
) {
    let inner = if let Some(block) = block {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        inner
    } else {
        area
    };

    let content_height = lines.len() as u16;
    let mut scroll_view = ScrollView::new(Size::new(inner.width, content_height));
    scroll_view.render_widget(
        Paragraph::new(lines),
        Rect::new(0, 0, inner.width, content_height),
    );
    frame.render_stateful_widget(scroll_view, inner, state);
}
