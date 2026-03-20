use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::Block;

use crate::theme::Theme;

pub struct AppShell<'a> {
    theme: &'a Theme,
}

impl<'a> AppShell<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self { theme }
    }

    /// Fill the background and return `[content, status_bar]` areas.
    pub fn render(self, frame: &mut Frame) -> [Rect; 2] {
        frame.render_widget(
            Block::default().style(Style::default().bg(self.theme.bg)),
            frame.area(),
        );
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area())
    }
}
