use ratatui::{
    buffer::Buffer,
    layout::{Margin, Rect},
    text::Line,
    widgets::{
        Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget,
    },
};

pub struct Scrollable<'a> {
    lines: Vec<Line<'a>>,
    block: Option<Block<'a>>,
}

impl<'a> Scrollable<'a> {
    pub fn new(lines: Vec<Line<'a>>) -> Self {
        Self { lines, block: None }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}

pub struct ScrollState {
    pub offset: u16,
    pub max_scroll: u16,
}

impl ScrollState {
    pub fn new() -> Self {
        Self {
            offset: 0,
            max_scroll: 0,
        }
    }
}

impl StatefulWidget for Scrollable<'_> {
    type State = ScrollState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let block = self.block.unwrap_or_default();
        let inner = block.inner(area);
        let line_count = self.lines.len();
        let visible_lines = inner.height as usize;
        let max_scroll = line_count
            .saturating_sub(visible_lines)
            .min(u16::MAX as usize) as u16;

        state.max_scroll = max_scroll;
        state.offset = state.offset.min(max_scroll);

        let paragraph = Paragraph::new(self.lines)
            .block(block)
            .scroll((state.offset, 0));
        paragraph.render(area, buf);

        let mut scrollbar_state =
            ScrollbarState::new(line_count.max(1)).position(state.offset as usize);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        scrollbar.render(
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            buf,
            &mut scrollbar_state,
        );
    }
}
