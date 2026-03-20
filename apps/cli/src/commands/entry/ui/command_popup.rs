use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Widget},
};

use crate::theme::Theme;

use super::super::app::CommandEntry;

pub(crate) const GROUP_ORDER: &[&str] = &["Meeting", "Setup", "Models", "App"];

// --- Data layer: describe what to render ---

enum Row<'a> {
    Spacer,
    Header(&'a str),
    Command {
        entry: &'a CommandEntry,
        selected: bool,
    },
}

fn layout<'a>(commands: &'a [CommandEntry], selected_index: usize) -> Vec<Row<'a>> {
    let mut rows = Vec::new();
    let mut cmd_index = 0usize;

    for &group in GROUP_ORDER {
        let group_cmds: Vec<_> = commands.iter().filter(|c| c.group == group).collect();
        if group_cmds.is_empty() {
            continue;
        }

        if !rows.is_empty() {
            rows.push(Row::Spacer);
        }
        rows.push(Row::Header(group));

        for entry in group_cmds {
            rows.push(Row::Command {
                entry,
                selected: cmd_index == selected_index,
            });
            cmd_index += 1;
        }
    }

    rows
}

pub fn popup_row_count(commands: &[CommandEntry]) -> u16 {
    layout(commands, 0).len().min(u16::MAX as usize) as u16
}

// --- View layer: how to render each row ---

fn render_row<'a>(row: &Row<'_>, query: &str, theme: &Theme) -> ListItem<'a> {
    match row {
        Row::Spacer => ListItem::new(Line::raw("")),
        Row::Header(label) => ListItem::new(Line::from(vec![Span::styled(
            format!(" {}", label),
            theme.muted,
        )])),
        Row::Command { entry, .. } => render_command(entry, query, theme),
    }
}

fn render_command<'a>(entry: &CommandEntry, query: &str, theme: &Theme) -> ListItem<'a> {
    let disabled = entry.disabled_reason.is_some();
    let mut spans = vec![Span::raw("  ")];

    if disabled {
        spans.push(Span::styled(entry.name, theme.muted));
    } else {
        spans.extend(command_name_spans(entry.name, query, theme));
    }

    let name_width = entry.name.chars().count();
    if name_width < 20 {
        spans.push(Span::raw(" ".repeat(20 - name_width)));
    }

    spans.push(Span::styled(entry.description, theme.muted));

    if let Some(reason) = entry.disabled_reason {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            reason,
            theme.muted.add_modifier(Modifier::ITALIC),
        ));
    }

    let item = ListItem::new(Line::from(spans));
    if disabled {
        item.style(Style::new().bg(theme.disabled_bg))
    } else {
        item
    }
}

// --- Widget: wires layout + view together ---

pub struct CommandPopup<'a> {
    commands: &'a [CommandEntry],
    selected_index: usize,
    query: &'a str,
    theme: &'a Theme,
}

impl<'a> CommandPopup<'a> {
    pub fn new(
        commands: &'a [CommandEntry],
        selected_index: usize,
        query: &'a str,
        theme: &'a Theme,
    ) -> Self {
        Self {
            commands,
            selected_index,
            query,
            theme,
        }
    }
}

impl Widget for CommandPopup<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 1 || area.width < 4 {
            return;
        }

        let rows = layout(self.commands, self.selected_index);

        let selected_row = rows
            .iter()
            .position(|r| matches!(r, Row::Command { selected: true, .. }));
        let items: Vec<ListItem> = rows
            .iter()
            .map(|r| render_row(r, self.query, self.theme))
            .collect();

        let list = List::new(items)
            .style(Style::new().bg(self.theme.input_bg))
            .highlight_style(Style::new().bg(self.theme.highlight_bg));

        let mut state = ratatui::widgets::ListState::default().with_selected(selected_row);
        ratatui::widgets::StatefulWidget::render(list, area, buf, &mut state);
    }
}

// --- Helpers ---

fn command_name_spans<'a>(name: &str, query: &str, theme: &Theme) -> Vec<Span<'a>> {
    let body = name.trim_start_matches('/');
    let matched = highlight_indices(query, name);

    let bold = Style::new().add_modifier(Modifier::BOLD);
    let highlight = theme.accent.add_modifier(Modifier::BOLD);

    let mut spans = Vec::with_capacity(body.len() + 1);
    spans.push(Span::styled("/", bold));

    for (i, ch) in body.chars().enumerate() {
        let style = if matched.contains(&i) {
            highlight
        } else {
            bold
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
