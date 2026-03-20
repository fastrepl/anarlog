use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Paragraph};

use crate::theme::Theme;

use super::super::app::App;

enum Section {
    Label {
        text: String,
        focused: bool,
    },
    Input {
        text: String,
        cursor_x: Option<u16>,
        focused: bool,
    },
    Default(String),
    Error(String),
    Gap,
}

fn sections(app: &App) -> Vec<Section> {
    let fields = app.form_fields();
    let focused_idx = app.focused_field();
    let mut out = Vec::new();

    for (i, field) in fields.iter().enumerate() {
        if i > 0 {
            out.push(Section::Gap);
        }

        let is_focused = i == focused_idx;

        let display_text = if field.masked && !field.value.is_empty() {
            "*".repeat(field.value.chars().count())
        } else {
            field.value.clone()
        };

        #[allow(clippy::cast_possible_truncation)]
        let cursor_x = if is_focused {
            Some(field.cursor_pos as u16)
        } else {
            None
        };

        out.push(Section::Label {
            text: format!("  {}:", field.label),
            focused: is_focused,
        });
        out.push(Section::Input {
            text: display_text,
            cursor_x,
            focused: is_focused,
        });

        if let Some(ref default) = field.default {
            out.push(Section::Default(format!("  default: {default}")));
        }

        if let Some(ref error) = field.error {
            out.push(Section::Error(format!("  {error}")));
        }
    }

    out
}

fn section_constraint(section: &Section) -> Constraint {
    match section {
        Section::Input { .. } => Constraint::Length(3),
        Section::Gap => Constraint::Length(1),
        _ => Constraint::Length(1),
    }
}

fn render_section(frame: &mut Frame, section: &Section, area: Rect, theme: &Theme) {
    match section {
        Section::Label { text, focused } => {
            let style = if *focused {
                Style::new().bold()
            } else {
                theme.muted
            };
            frame.render_widget(Span::styled(text.as_str(), style), area);
        }
        Section::Input {
            text,
            cursor_x,
            focused,
        } => {
            let border_style = if *focused {
                Style::new().fg(Color::Cyan)
            } else {
                theme.border
            };
            let input_block = Block::bordered().border_style(border_style);
            let inner = input_block.inner(area);
            frame.render_widget(Paragraph::new(text.as_str()).block(input_block), area);
            if let Some(cx) = cursor_x {
                frame.set_cursor_position(Position::new(inner.x + cx, inner.y));
            }
        }
        Section::Default(text) => {
            frame.render_widget(Span::styled(text.as_str(), theme.muted), area);
        }
        Section::Error(text) => {
            frame.render_widget(Span::styled(text.as_str(), theme.error), area);
        }
        Section::Gap => {}
    }
}

pub(crate) fn draw(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let secs = sections(app);

    let mut constraints: Vec<Constraint> = secs.iter().map(section_constraint).collect();
    constraints.push(Constraint::Min(0));

    let areas = Layout::vertical(constraints).split(area);

    for (section, &area) in secs.iter().zip(areas.iter()) {
        render_section(frame, section, area, theme);
    }
}
