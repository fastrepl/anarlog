use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use textwrap::wrap;

use crate::commands::chat::app::{App, Speaker};
use crate::theme::Theme;
use crate::widgets::Scrollable;

pub(super) fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let theme = Theme::DEFAULT;
    let width = area.width as usize;
    let wrap_width = width.saturating_sub(6).max(8);

    let accent = Style::new().fg(Color::Indexed(69));

    let mut lines: Vec<Line<'static>> = Vec::new();

    for msg in app.transcript() {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        match msg.speaker {
            Speaker::User => {
                let wrapped = wrap(&msg.content, wrap_width);
                for w in wrapped {
                    lines.push(Line::from(vec![
                        Span::styled("  \u{258e} ", accent),
                        Span::raw(w.to_string()),
                    ]));
                }
            }
            Speaker::Assistant => {
                let wrapped = wrap(&msg.content, wrap_width);
                for w in wrapped {
                    lines.push(Line::from(Span::styled(
                        format!("    {w}"),
                        theme.transcript_final,
                    )));
                }
            }
            Speaker::Error => {
                let wrapped = wrap(&msg.content, wrap_width);
                for w in wrapped {
                    lines.push(Line::from(Span::styled(format!("    {w}"), theme.error)));
                }
            }
        }
    }

    if app.streaming() || !app.pending_assistant().is_empty() {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        let content = app.pending_assistant();
        if content.is_empty() {
            lines.push(Line::from(Span::styled(
                "    ...",
                theme.transcript_partial,
            )));
        } else {
            let wrapped = wrap(content, wrap_width);
            for w in wrapped {
                lines.push(Line::from(Span::styled(
                    format!("    {w}"),
                    theme.transcript_final,
                )));
            }
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "    Start a conversation below.",
            theme.placeholder,
        )));
    }

    let scrollable = Scrollable::new(lines);
    frame.render_stateful_widget(scrollable, area, app.scroll_state_mut());
}
