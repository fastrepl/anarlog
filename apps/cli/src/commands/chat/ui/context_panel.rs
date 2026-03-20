use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

use crate::commands::chat::app::App;
use crate::output::format_hhmmss;
use crate::theme::Theme;

// --- Data layer: describe what to render ---

enum Section<'a> {
    Title(&'a str),
    Field {
        label: &'static str,
        value: String,
        active: bool,
    },
    Error {
        label: &'static str,
        value: String,
    },
}

fn sections(app: &App) -> Vec<Section<'_>> {
    let mut out = vec![
        Section::Title(app.meeting().unwrap_or("Chat")),
        Section::Field {
            label: "Model",
            value: app.model().to_string(),
            active: false,
        },
        Section::Field {
            label: "Elapsed",
            value: format_hhmmss(app.elapsed()),
            active: false,
        },
        Section::Field {
            label: "Status",
            value: app.status(),
            active: app.streaming(),
        },
    ];

    if let Some(err) = app.last_error() {
        out.push(Section::Error {
            label: "Error",
            value: err.to_string(),
        });
    }

    out
}

// --- View layer: how to render each section ---

fn render_section(section: &Section<'_>, theme: &Theme) -> Vec<Line<'static>> {
    let heading = Style::new().fg(Color::White);

    match section {
        Section::Title(title) => {
            vec![Line::from(Span::styled(title.to_string(), heading))]
        }
        Section::Field {
            label,
            value,
            active,
        } => {
            let value_style = if *active {
                theme.status_active
            } else {
                theme.muted
            };
            vec![
                Line::from(Span::styled(*label, heading)),
                Line::from(Span::styled(value.clone(), value_style)),
            ]
        }
        Section::Error { label, value } => {
            vec![
                Line::from(Span::styled(*label, theme.error)),
                Line::from(Span::styled(value.clone(), theme.error)),
            ]
        }
    }
}

pub(super) fn draw(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    for section in &sections(app) {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        lines.extend(render_section(section, theme));
    }

    let block = Block::new()
        .borders(Borders::LEFT)
        .border_style(theme.border)
        .padding(Padding::horizontal(1));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}
