mod context_panel;
mod header;
mod input;
mod status_bar;
mod transcript;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

use crate::theme::Theme;
use crate::widgets::AppShell;

use super::app::App;

pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    let theme = Theme::DEFAULT;

    let [main_area, status_area] = AppShell::new(&theme).render(frame);

    let show_context = main_area.width >= 80;
    let (chat_col, context_col) = if show_context {
        let [chat, ctx] =
            Layout::horizontal([Constraint::Min(40), Constraint::Length(28)]).areas(main_area);
        (chat, Some(ctx))
    } else {
        (main_area, None)
    };

    let input_height = (app.input().lines().len() as u16).clamp(1, 8) + 2;
    let [header_area, body_area, input_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(input_height),
    ])
    .areas(chat_col);

    app.apply_autoscroll();

    header::draw(frame, app, header_area, &theme);
    transcript::draw(frame, app, body_area, &theme);
    input::draw(frame, app, input_area, &theme);
    if let Some(ctx) = context_col {
        context_panel::draw(frame, app, ctx, &theme);
    }
    status_bar::draw(frame, app, status_area, &theme);
}
