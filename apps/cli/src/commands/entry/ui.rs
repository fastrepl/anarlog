use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use ratatui_image::{Resize, StatefulImage};

use crate::theme::Theme;

use super::app::{COMMANDS, EntryApp, SessionEntry, SessionsOverlay, command_highlight_indices};

const APP_VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));

pub fn draw(frame: &mut Frame, app: &mut EntryApp) {
    let theme = Theme::default();
    let popup_height = if app.popup_visible() {
        app.popup_height()
    } else {
        0
    };
    let logo_height = frame
        .area()
        .height
        .saturating_div(4)
        .saturating_sub(popup_height.saturating_div(3))
        .clamp(7, 12);

    let [main_area, status_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());

    let [logo_area, title_area, popup_area, input_area, hint_area] = Layout::vertical([
        Constraint::Length(logo_height),
        Constraint::Length(1),
        Constraint::Length(popup_height),
        Constraint::Length(3),
        Constraint::Length(1),
    ])
    .flex(Flex::Center)
    .areas(main_area);

    let logo_area = centered_width(logo_area, 78);
    let title_area = centered_width(title_area, 90);
    let popup_area = centered_width(popup_area, 90);
    let input_area = centered_width(input_area, 90);
    let hint_area = centered_width(hint_area, 90);

    draw_logo(frame, logo_area, app);
    draw_main(frame, title_area, &theme);

    if app.popup_visible() {
        draw_popup(frame, popup_area, app, &theme);
    }

    draw_input(frame, input_area, app, &theme);
    draw_hints(frame, hint_area, app, &theme);
    draw_status(frame, status_area, app, &theme);

    if let Some(overlay) = app.sessions_overlay() {
        draw_sessions_overlay(frame, main_area, overlay, &theme);
    }
}

fn draw_logo(frame: &mut Frame, area: Rect, app: &mut EntryApp) {
    if area.width < 4 || area.height < 4 {
        return;
    }

    let Some(logo_protocol) = app.logo_protocol() else {
        return;
    };

    let resize = Resize::Fit(None);
    let render_area = logo_protocol.size_for(resize.clone(), area);
    let render_area = centered_rect(area, render_area.width.max(1), render_area.height.max(1));

    frame.render_stateful_widget(
        StatefulImage::default().resize(resize),
        render_area,
        logo_protocol,
    );
}

fn draw_main(frame: &mut Frame, area: ratatui::layout::Rect, theme: &Theme) {
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("char", Style::new().add_modifier(Modifier::BOLD)),
            Span::styled("  type ", theme.muted),
            Span::styled("/", Style::new().fg(ratatui::style::Color::Cyan)),
            Span::styled(" or a command", theme.muted),
        ]))
        .style(theme.muted)
        .centered(),
        area,
    );
}

fn draw_popup(frame: &mut Frame, area: ratatui::layout::Rect, app: &EntryApp, theme: &Theme) {
    if area.height < 3 {
        return;
    }

    let query = app.query();

    let items = app
        .filtered_commands()
        .iter()
        .map(|index| {
            let command = COMMANDS[*index];
            let mut spans = command_name_spans(command.name, &query);
            let command_width = command.name.chars().count();
            if command_width < 10 {
                spans.push(Span::raw(" ".repeat(10 - command_width)));
            }
            spans.push(Span::raw("  "));
            spans.push(Span::styled(command.description, theme.muted));
            ListItem::new(Line::from(spans))
        })
        .collect::<Vec<_>>();

    let list = List::new(items)
        .block(
            Block::new()
                .borders(Borders::ALL)
                .border_style(theme.border)
                .title(" Commands "),
        )
        .highlight_style(Style::new().bg(Color::Rgb(55, 60, 70)))
        .highlight_symbol("› ");

    let mut state =
        ratatui::widgets::ListState::default().with_selected(Some(app.selected_index()));
    frame.render_stateful_widget(list, area, &mut state);
}

fn command_name_spans(command: &str, query: &str) -> Vec<Span<'static>> {
    let command_body = command.trim_start_matches('/');
    let highlight_indices = command_highlight_indices(query, command);

    let mut spans = Vec::with_capacity(command_body.chars().count() + 1);
    spans.push(Span::styled(
        "/",
        Style::new().fg(ratatui::style::Color::Cyan),
    ));

    for (i, ch) in command_body.chars().enumerate() {
        let style = if highlight_indices.contains(&i) {
            Style::new()
                .fg(ratatui::style::Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().add_modifier(Modifier::BOLD)
        };
        spans.push(Span::styled(ch.to_string(), style));
    }

    spans
}

fn draw_input(frame: &mut Frame, area: ratatui::layout::Rect, app: &EntryApp, theme: &Theme) {
    let value = app.input_text();
    let line = if value.is_empty() {
        Line::from(Span::styled("/listen", theme.placeholder))
    } else {
        Line::from(value)
    };

    frame.render_widget(
        Paragraph::new(line).block(
            Block::new()
                .borders(Borders::ALL)
                .border_style(theme.border_focused)
                .title(" Command "),
        ),
        area,
    );

    let cursor_x = area
        .x
        .saturating_add(1)
        .saturating_add(app.cursor_col() as u16)
        .min(area.x + area.width.saturating_sub(2));
    let cursor_y = area.y.saturating_add(1);
    frame.set_cursor_position(Position {
        x: cursor_x,
        y: cursor_y,
    });
}

fn draw_status(frame: &mut Frame, area: ratatui::layout::Rect, app: &EntryApp, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let version_width = (APP_VERSION.chars().count() as u16).min(area.width);
    let [left_area, right_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(version_width)]).areas(area);

    if let Some(status) = &app.status_message {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                status.as_str(),
                theme.shortcut_key,
            ))),
            left_area,
        );
    }

    frame.render_widget(
        Paragraph::new(APP_VERSION)
            .style(theme.muted)
            .alignment(Alignment::Right),
        right_area,
    );
}

fn draw_hints(frame: &mut Frame, area: ratatui::layout::Rect, app: &EntryApp, theme: &Theme) {
    if app.sessions_overlay().is_some() {
        return;
    }

    let command_preview = app
        .selected_command()
        .map(|command| format!("{} {}", command.name, command.description))
        .unwrap_or_else(|| "/listen Start live transcription".to_string());

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("[enter]", theme.shortcut_key),
            Span::styled(" run  ", theme.muted),
            Span::styled("[tab]", theme.shortcut_key),
            Span::styled(" fill  ", theme.muted),
            Span::styled("[up/down]", theme.shortcut_key),
            Span::styled(" choose  ", theme.muted),
            Span::styled("[esc]", theme.shortcut_key),
            Span::styled(" clear  ", theme.muted),
            Span::styled(command_preview, theme.placeholder),
        ]))
        .centered(),
        area,
    );
}

fn draw_sessions_overlay(frame: &mut Frame, area: Rect, overlay: &SessionsOverlay, theme: &Theme) {
    let popup_width = area.width.saturating_mul(4).saturating_div(5).max(40);
    let popup_height = area.height.saturating_mul(3).saturating_div(4).max(16);
    let popup_area = centered_rect(area, popup_width, popup_height);

    frame.render_widget(
        Block::new()
            .borders(Borders::ALL)
            .border_style(theme.border)
            .title(" Sessions "),
        popup_area,
    );

    let inner = popup_area;
    let [header_area, search_area, list_area, hint_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .margin(1)
    .areas(inner);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Sessions", Style::new().add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled("esc", theme.muted),
        ])),
        header_area,
    );

    if let Some(index) = overlay.viewing_session {
        if let Some(entry) = overlay.entries.get(index) {
            let body_area = Rect {
                x: search_area.x,
                y: search_area.y,
                width: search_area.width.max(list_area.width),
                height: search_area.height.saturating_add(list_area.height),
            };
            draw_session_readonly(frame, body_area, hint_area, entry, theme);
        }
        frame.set_cursor_position(Position {
            x: popup_area.x,
            y: popup_area.y,
        });
        return;
    }

    let search = overlay.search_text();
    let search_line = if search.is_empty() {
        Line::from(Span::styled("Search", theme.placeholder))
    } else {
        Line::from(search)
    };
    frame.render_widget(Paragraph::new(search_line), search_area);

    let lines = session_list_lines(overlay, list_area.width as usize, theme);
    frame.render_widget(Paragraph::new(lines), list_area);

    let hint = if let Some(status) = &overlay.status_message {
        Line::from(vec![
            Span::styled(status.as_str(), theme.shortcut_key),
            Span::raw("  "),
            Span::styled("delete ctrl+d", theme.muted),
            Span::raw("  "),
            Span::styled("rename ctrl+r", theme.muted),
        ])
    } else {
        Line::from(vec![
            Span::styled("open enter", theme.muted),
            Span::raw("  "),
            Span::styled("delete ctrl+d", theme.muted),
            Span::raw("  "),
            Span::styled("rename ctrl+r", theme.muted),
            Span::raw("  "),
            Span::styled("close esc", theme.muted),
        ])
    };
    frame.render_widget(Paragraph::new(hint), hint_area);

    let cursor_x = search_area
        .x
        .saturating_add(overlay.search_cursor_col() as u16)
        .min(search_area.x + search_area.width.saturating_sub(1));
    frame.set_cursor_position(Position {
        x: cursor_x,
        y: search_area.y,
    });
}

fn session_list_lines(
    overlay: &SessionsOverlay,
    width: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    if overlay.filtered_indices.is_empty() {
        return vec![Line::from(Span::styled("No sessions", theme.placeholder))];
    }

    let mut lines = Vec::new();
    let mut current_day = String::new();
    for (filtered_pos, session_index) in overlay.filtered_indices.iter().enumerate() {
        let Some(entry) = overlay.entries.get(*session_index) else {
            continue;
        };

        if current_day != entry.day_label {
            current_day = entry.day_label.clone();
            lines.push(Line::from(Span::styled(
                current_day.clone(),
                Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )));
        }

        let time_width = entry.time_label.chars().count();
        let content_width = width.saturating_sub(1);
        let title_width = content_width.saturating_sub(time_width + 1);
        let title = truncate_to_width(&entry.title, title_width);
        let padding = " ".repeat(title_width.saturating_sub(title.chars().count()));

        let mut style = Style::new();
        if filtered_pos == overlay.selected_index {
            style = style.bg(Color::Rgb(77, 150, 225));
        }

        lines.push(Line::from(vec![Span::styled(
            format!("{title}{padding} {}", entry.time_label),
            style,
        )]));
    }

    lines
}

fn draw_session_readonly(
    frame: &mut Frame,
    body_area: Rect,
    hint_area: Rect,
    entry: &SessionEntry,
    theme: &Theme,
) {
    let [meta_area, notes_area, transcript_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Percentage(35),
        Constraint::Percentage(65),
    ])
    .areas(body_area);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                entry.title.clone(),
                Style::new().add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(entry.time_label.clone(), theme.muted),
        ])),
        meta_area,
    );

    frame.render_widget(
        Paragraph::new(entry.notes.clone()).block(
            Block::new()
                .borders(Borders::ALL)
                .border_style(theme.border)
                .title(" Notes (read-only) "),
        ),
        notes_area,
    );

    frame.render_widget(
        Paragraph::new(entry.transcript.clone()).block(
            Block::new()
                .borders(Borders::ALL)
                .border_style(theme.border)
                .title(" Transcript (read-only) "),
        ),
        transcript_area,
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("back esc", theme.muted),
            Span::raw("  "),
            Span::styled("delete ctrl+d", theme.muted),
            Span::raw("  "),
            Span::styled("rename ctrl+r", theme.muted),
        ])),
        hint_area,
    );
}

fn truncate_to_width(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= width {
        return value.to_string();
    }

    if width <= 3 {
        return chars.into_iter().take(width).collect();
    }

    let mut out = chars.into_iter().take(width - 3).collect::<String>();
    out.push_str("...");
    out
}

fn centered_width(area: Rect, max_width: u16) -> Rect {
    let width = area.width.min(max_width).max(1);
    let [centered] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(area);
    centered
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width).max(1);
    let height = height.min(area.height).max(1);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;

    Rect {
        x,
        y,
        width,
        height,
    }
}
