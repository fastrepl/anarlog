use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, ListItem, Paragraph};

use crate::theme::Theme;
use crate::widgets::SelectList;

use super::super::app::App;

const BADGE_STT: Style = Style::new().fg(Color::Cyan);
const BADGE_LLM: Style = Style::new().fg(Color::Yellow);
const BADGE_CAL: Style = Style::new().fg(Color::Magenta);

pub(crate) fn draw(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let [search_area, list_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).areas(area);

    // Search input
    let search_block = Block::bordered()
        .title(" Search ")
        .border_style(Style::new().fg(Color::Cyan));
    let search_inner = search_block.inner(search_area);
    frame.render_widget(
        Paragraph::new(app.search_query()).block(search_block),
        search_area,
    );
    #[allow(clippy::cast_possible_truncation)]
    let cursor_x = app.search_query().chars().count() as u16;
    frame.set_cursor_position(Position::new(search_inner.x + cursor_x, search_inner.y));

    // Provider list with tags
    let filtered = app.filtered_providers();
    if filtered.is_empty() {
        frame.render_widget(Span::styled("  No matches", theme.muted), list_area);
        return;
    }

    let configured = app.configured_providers();
    let current_stt = app.current_stt_provider();
    let current_llm = app.current_llm_provider();
    let available_width = list_area.width as usize;
    let items: Vec<ListItem> = filtered
        .iter()
        .map(|p| {
            let disabled = p.is_disabled();
            let name = p.display_name();
            let caps = p.capabilities();
            let is_configured = configured.contains(p.id());

            let is_active_stt = current_stt == Some(p.id());
            let is_active_llm = current_llm == Some(p.id());

            let mut tag_spans: Vec<Span> = Vec::new();

            if is_configured {
                tag_spans.push(Span::styled("✓", theme.status_active));
            }

            if disabled {
                if !tag_spans.is_empty() {
                    tag_spans.push(Span::raw(" "));
                }
                tag_spans.push(Span::styled("soon", theme.muted));
            }

            for cap in &caps {
                if !tag_spans.is_empty() {
                    tag_spans.push(Span::raw(" "));
                }

                let is_active = match cap {
                    super::super::ConnectionType::Stt => is_active_stt,
                    super::super::ConnectionType::Llm => is_active_llm,
                    super::super::ConnectionType::Cal => false,
                };

                let style = if disabled {
                    theme.muted
                } else if is_active {
                    match cap {
                        super::super::ConnectionType::Stt => BADGE_STT,
                        super::super::ConnectionType::Llm => BADGE_LLM,
                        super::super::ConnectionType::Cal => BADGE_CAL,
                    }
                } else {
                    theme.muted
                };

                let label = match cap {
                    super::super::ConnectionType::Stt => "[STT]",
                    super::super::ConnectionType::Llm => "[LLM]",
                    super::super::ConnectionType::Cal => "[CAL]",
                };
                tag_spans.push(Span::styled(label, style));
            }

            let tags_width: usize = tag_spans.iter().map(|s| s.width()).sum();
            let padding = if available_width > name.len() + tags_width + 2 {
                available_width - name.len() - tags_width - 2
            } else {
                1
            };

            let name_style = if disabled {
                theme.muted
            } else {
                Style::default()
            };

            let mut spans = vec![
                Span::styled(name.to_string(), name_style),
                Span::raw(" ".repeat(padding)),
            ];
            spans.extend(tag_spans);
            ListItem::new(Line::from(spans))
        })
        .collect();

    frame.render_stateful_widget(
        SelectList::new(items, theme),
        list_area,
        app.list_state_mut(),
    );
}
