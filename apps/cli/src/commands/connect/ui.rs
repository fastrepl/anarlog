use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, Paragraph};

use crate::theme::Theme;
use crate::widgets::KeyHints;

use super::app::{App, Step};

pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    let theme = Theme::DEFAULT;
    let area = centered_dialog(frame.area());

    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .title(" Connect a provider ")
        .border_style(theme.border);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [header_area, content_area, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    draw_header(frame, app, header_area);

    match app.step() {
        Step::SelectType | Step::SelectProvider => draw_list(frame, app, content_area, &theme),
        Step::InputBaseUrl | Step::InputApiKey => draw_input(frame, app, content_area, &theme),
        Step::Done => {}
    }

    draw_status(frame, app, status_area, &theme);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let breadcrumb = app.breadcrumb();
    if breadcrumb.is_empty() {
        return;
    }
    frame.render_widget(
        Line::from(Span::styled(
            format!("  {breadcrumb}"),
            Style::new().fg(Color::DarkGray),
        )),
        area,
    );
}

fn draw_list(frame: &mut Frame, app: &mut App, area: Rect, _theme: &Theme) {
    let [label_area, _, list_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(area);

    let label = match app.step() {
        Step::SelectType => "  Connection type:",
        Step::SelectProvider => "  Provider:",
        _ => "",
    };
    frame.render_widget(Span::styled(label, Style::new().bold()), label_area);

    let items: Vec<&str> = match app.step() {
        Step::SelectType => vec!["stt", "llm"],
        Step::SelectProvider => app.provider_list().iter().map(|p| p.id()).collect(),
        _ => vec![],
    };

    let list = List::new(items)
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .highlight_symbol("  > ");

    frame.render_stateful_widget(list, list_area, app.list_state_mut());
}

fn draw_input(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let mut constraints = vec![
        Constraint::Length(1), // label
        Constraint::Length(3), // input box
    ];
    if app.input_default().is_some() {
        constraints.push(Constraint::Length(1));
    }
    if app.error().is_some() {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Min(0));

    let areas = Layout::vertical(constraints).split(area);
    let mut idx = 0;

    frame.render_widget(
        Span::styled(format!("  {}:", app.input_label()), Style::new().bold()),
        areas[idx],
    );
    idx += 1;

    let input_area = areas[idx];
    let input_block = Block::bordered().border_style(Style::new().fg(Color::Cyan));
    let inner_input = input_block.inner(input_area);

    let display_text = if app.input_masked() && !app.input().is_empty() {
        "*".repeat(app.input().chars().count())
    } else {
        app.input().to_string()
    };

    frame.render_widget(Paragraph::new(display_text).block(input_block), input_area);

    #[allow(clippy::cast_possible_truncation)]
    frame.set_cursor_position(Position::new(
        inner_input.x + app.cursor_pos() as u16,
        inner_input.y,
    ));
    idx += 1;

    if let Some(default) = app.input_default() {
        frame.render_widget(
            Span::styled(
                format!("  default: {default}"),
                Style::new().fg(Color::DarkGray),
            ),
            areas[idx],
        );
        idx += 1;
    }

    if let Some(error) = app.error() {
        frame.render_widget(Span::styled(format!("  {error}"), theme.error), areas[idx]);
    }
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let hints = match app.step() {
        Step::SelectType | Step::SelectProvider => {
            vec![("↑/↓", "navigate"), ("Enter", "select"), ("Esc", "quit")]
        }
        Step::InputBaseUrl | Step::InputApiKey => {
            vec![("Enter", "confirm"), ("Esc", "quit")]
        }
        Step::Done => vec![],
    };

    frame.render_widget(KeyHints::new(theme).hints(hints), area);
}

fn centered_dialog(area: Rect) -> Rect {
    let width = area.width.saturating_mul(3).saturating_div(5).clamp(40, 80);
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
