mod command_input;
mod command_popup;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};
use ratatui_image::{Resize, StatefulImage};

use crate::theme::Theme;

use super::app::App;

use command_input::{CommandInput, CursorState};
use command_popup::CommandPopup;

const APP_VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));

pub fn draw(frame: &mut Frame, app: &mut App) {
    let theme = Theme::DEFAULT;
    let logo_height = frame.area().height.saturating_div(4).clamp(7, 12);

    let [main_area, status_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());

    let [
        logo_area,
        title_area,
        _gap,
        input_area,
        _gap_hints,
        hint_area,
        _gap2,
        tip_area,
    ] = Layout::vertical([
        Constraint::Length(logo_height),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(5),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Length(1),
    ])
    .flex(Flex::Center)
    .areas(main_area);

    let logo_area = centered_width(logo_area, 78);
    let title_area = centered_width(title_area, 90);
    let input_area = centered_width(input_area, 90);
    let hint_area = centered_width(hint_area, 90);
    let tip_area = centered_width(tip_area, 90);

    draw_logo(frame, logo_area, app);
    draw_title(frame, title_area, &theme);
    draw_input(frame, input_area, app, &theme);
    draw_hints(frame, hint_area, app, &theme);
    draw_tip(frame, tip_area, app, &theme);
    draw_status(frame, status_area, app, &theme);

    if app.popup_visible() {
        let popup_height = app.popup_height();
        let popup_y = input_area.y.saturating_sub(popup_height);
        let popup_area = Rect {
            x: input_area.x,
            y: popup_y,
            width: input_area.width,
            height: input_area.y.saturating_sub(popup_y),
        };
        frame.render_widget(Clear, popup_area);

        let mut state = ();
        frame.render_stateful_widget(
            CommandPopup::new(
                app.filtered_commands(),
                app.selected_index(),
                &app.query(),
                &theme,
            ),
            popup_area,
            &mut state,
        );
    }
}

fn draw_logo(frame: &mut Frame, area: Rect, app: &mut App) {
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

fn draw_title(frame: &mut Frame, area: Rect, theme: &Theme) {
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("char", Style::new().add_modifier(Modifier::BOLD)),
            Span::styled("  type ", theme.muted),
            Span::styled("/", theme.accent),
            Span::styled(" or a command", theme.muted),
        ]))
        .style(theme.muted)
        .centered(),
        area,
    );
}

fn draw_input(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let mut cursor = CursorState::default();
    frame.render_stateful_widget(
        CommandInput::new(
            &app.input_text(),
            app.cursor_col(),
            app.stt_provider.as_deref(),
            app.llm_provider.as_deref(),
            theme,
        ),
        area,
        &mut cursor,
    );
    if let Some(pos) = cursor.position {
        frame.set_cursor_position(pos);
    }
}

fn draw_tip(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Tip: ", theme.accent),
            Span::styled(app.tip, theme.muted),
        ]))
        .centered(),
        area,
    );
}

fn draw_status(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let right_margin: u16 = 3;
    let version_width = (APP_VERSION.chars().count() as u16).min(area.width);
    let [left_area, right_area, _margin] = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(version_width),
        Constraint::Length(right_margin),
    ])
    .areas(area);

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

fn draw_hints(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let command_preview = app
        .selected_command()
        .map(|command| format!("{} {}", command.name, command.description))
        .unwrap_or_else(|| "/connect Connect provider".to_string());

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
