mod command_input;
pub(crate) mod command_popup;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};
use ratatui_image::{Resize, StatefulImage};

use crate::theme::Theme;
use crate::widgets::AppShell;

use super::app::{App, Overlay};

use command_input::{CommandInput, CursorState};
use command_popup::{CommandPopup, popup_row_count};

const APP_VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));

pub fn draw(frame: &mut Frame, app: &mut App) {
    let theme = Theme::DEFAULT;

    let [main_area, status_area] = AppShell::new(&theme).render(frame);

    let logo_height = frame.area().height.saturating_div(6).clamp(5, 7);

    let [_top_pad, logo_area, input_area, _gap, tip_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(logo_height),
        Constraint::Length(5),
        Constraint::Length(3),
        Constraint::Length(1),
    ])
    .flex(Flex::Center)
    .areas(main_area);

    let logo_area = centered_width(logo_area, 64);
    let input_area = centered_width(input_area, 76);
    let tip_area = centered_width(tip_area, 76);

    draw_logo(frame, logo_area, app);
    if !app.has_overlay() {
        draw_input(frame, input_area, app, &theme);
    }
    draw_tip(frame, tip_area, app, &theme);
    draw_status(frame, status_area, app, &theme);

    if app.popup_visible() && !app.has_overlay() {
        let margin_h: u16 = 2;
        let commands = app.visible_commands();
        let popup_height = popup_row_count(&commands).min(16);
        let popup_y = input_area.y.saturating_sub(popup_height);
        let popup_area = Rect {
            x: input_area.x + margin_h,
            y: popup_y,
            width: input_area.width.saturating_sub(margin_h * 2),
            height: input_area.y.saturating_sub(popup_y),
        };
        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            CommandPopup::new(&commands, app.selected_index(), &app.query(), &theme),
            popup_area,
        );
    }

    match app.overlay_mut() {
        Overlay::Connect(connect_app) => {
            crate::commands::connect::ui::draw(frame, connect_app);
        }
        Overlay::Meetings(meetings_app) => {
            crate::commands::meetings::ui::list::draw(frame, meetings_app);
        }
        Overlay::Models(models_app) => {
            crate::commands::model::ui::draw(frame, models_app);
        }
        Overlay::Timeline(timeline_app) => {
            crate::commands::timeline::ui::draw(frame, timeline_app);
        }
        Overlay::None => {}
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
