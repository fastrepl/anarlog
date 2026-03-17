use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, StatefulWidget},
};

use crate::theme::Theme;

use super::super::app::COMMANDS;

pub struct CommandPopup<'a> {
    filtered_commands: &'a [usize],
    selected_index: usize,
    query: &'a str,
    theme: &'a Theme,
}

impl<'a> CommandPopup<'a> {
    pub fn new(
        filtered_commands: &'a [usize],
        selected_index: usize,
        query: &'a str,
        theme: &'a Theme,
    ) -> Self {
        Self {
            filtered_commands,
            selected_index,
            query,
            theme,
        }
    }
}

impl StatefulWidget for CommandPopup<'_> {
    type State = ();

    fn render(self, area: Rect, buf: &mut Buffer, _state: &mut Self::State) {
        if area.height < 3 {
            return;
        }

        let items = self
            .filtered_commands
            .iter()
            .map(|index| {
                let command = COMMANDS[*index];
                let mut spans = command_name_spans(command.name, self.query);
                let command_width = command.name.chars().count();
                if command_width < 10 {
                    spans.push(Span::raw(" ".repeat(10 - command_width)));
                }
                spans.push(Span::raw("  "));
                spans.push(Span::styled(command.description, self.theme.muted));
                ListItem::new(Line::from(spans))
            })
            .collect::<Vec<_>>();

        let list = List::new(items)
            .block(
                Block::new()
                    .borders(Borders::ALL)
                    .border_style(self.theme.border)
                    .title(" Commands "),
            )
            .highlight_style(Style::new().bg(Color::Rgb(55, 60, 70)))
            .highlight_symbol("› ");

        let mut state =
            ratatui::widgets::ListState::default().with_selected(Some(self.selected_index));
        StatefulWidget::render(list, area, buf, &mut state);
    }
}

fn command_name_spans(command: &str, query: &str) -> Vec<Span<'static>> {
    let command_body = command.trim_start_matches('/');
    let highlight_indices = highlight_indices(query, command);

    let mut spans = Vec::with_capacity(command_body.chars().count() + 1);
    spans.push(Span::styled(
        "/",
        Style::new().fg(ratatui::style::Color::Yellow),
    ));

    for (i, ch) in command_body.chars().enumerate() {
        let style = if highlight_indices.contains(&i) {
            Style::new()
                .fg(ratatui::style::Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().add_modifier(Modifier::BOLD)
        };
        spans.push(Span::styled(ch.to_string(), style));
    }

    spans
}

fn highlight_indices(query: &str, command: &str) -> Vec<usize> {
    let query = query.trim().to_ascii_lowercase();
    let command = command.trim_start_matches('/').to_ascii_lowercase();

    if query.is_empty() {
        return Vec::new();
    }

    if command.starts_with(&query) {
        return (0..query.chars().count()).collect();
    }

    if let Some(start) = command.find(&query) {
        let width = query.chars().count();
        return (start..start + width).collect();
    }

    let mut query_chars = query.chars();
    let mut target = match query_chars.next() {
        Some(ch) => ch,
        None => return Vec::new(),
    };
    let mut indices = Vec::new();

    for (i, ch) in command.chars().enumerate() {
        if ch != target {
            continue;
        }

        indices.push(i);
        if let Some(next) = query_chars.next() {
            target = next;
        } else {
            return indices;
        }
    }

    Vec::new()
}
