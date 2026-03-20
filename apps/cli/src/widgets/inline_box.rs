use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::{Block, BorderType, Padding};

pub struct InlineBox;

impl InlineBox {
    pub fn render(frame: &mut Frame) -> Rect {
        let area = frame.area();
        let [_, box_area, _] = Layout::horizontal([
            Constraint::Length(2),
            Constraint::Max(80),
            Constraint::Length(2),
        ])
        .areas(area);
        let [_, box_area, _] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(box_area);
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .padding(Padding::new(2, 2, 1, 1));
        let inner = block.inner(box_area);
        frame.render_widget(block, box_area);
        inner
    }

    pub fn viewport_height(content_lines: u16) -> u16 {
        content_lines + 6
    }
}
