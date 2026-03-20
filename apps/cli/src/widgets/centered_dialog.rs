use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Clear};

use crate::theme::Theme;

pub struct CenteredDialog<'a> {
    title: &'a str,
    theme: &'a Theme,
}

impl<'a> CenteredDialog<'a> {
    pub fn new(title: &'a str, theme: &'a Theme) -> Self {
        Self { title, theme }
    }

    pub fn render(&self, frame: &mut Frame) -> Rect {
        frame.render_widget(
            Block::default().style(Style::new().bg(self.theme.overlay_bg)),
            frame.area(),
        );

        let area = centered_area(frame.area());

        frame.render_widget(Clear, area);
        frame.render_widget(
            Block::default().style(Style::new().bg(self.theme.dialog_bg)),
            area,
        );

        let padded = Rect {
            x: area.x + 3,
            y: area.y + 2,
            width: area.width.saturating_sub(6),
            height: area.height.saturating_sub(4),
        };

        let [title_area, _gap, content_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(padded);

        let [title_left, title_right] =
            Layout::horizontal([Constraint::Min(0), Constraint::Length(3)]).areas(title_area);

        frame.render_widget(
            Span::styled(
                self.title,
                Style::new()
                    .fg(self.theme.dialog_title_fg)
                    .add_modifier(Modifier::BOLD),
            ),
            title_left,
        );

        frame.render_widget(Span::styled("esc", self.theme.muted), title_right);

        content_area
    }
}

fn centered_area(area: Rect) -> Rect {
    let width = area.width.saturating_mul(2).saturating_div(5).clamp(40, 60);
    let height = area
        .height
        .saturating_mul(3)
        .saturating_div(5)
        .clamp(12, 30);
    let [v] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    let [h] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(v);
    h
}
