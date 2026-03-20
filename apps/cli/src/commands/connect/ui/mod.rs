mod calendar;
mod input_form;
mod provider_list;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};

use crate::theme::Theme;
use crate::widgets::{CenteredDialog, KeyHints};

use super::app::{App, CalendarPhase, Step};
use super::runtime::CalendarPermissionState;

pub(crate) fn draw(frame: &mut Frame, app: &mut App) {
    let theme = Theme::DEFAULT;

    let inner = CenteredDialog::new("Connect a provider", &theme).render(frame);

    let show_header = app.step() != Step::Calendar || app.calendar().show_header();

    let [header_area, content_area, gap_area, status_area] = Layout::vertical([
        Constraint::Length(if show_header { 1 } else { 0 }),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    if show_header {
        draw_header(frame, app, header_area);
    }

    match app.step() {
        Step::SelectProvider => provider_list::draw(frame, app, content_area, &theme),
        Step::InputForm => input_form::draw(frame, app, content_area, &theme),
        Step::Calendar => calendar::draw(frame, app, content_area, &theme),
        Step::Done => {}
    }

    let _ = gap_area;
    draw_status(frame, app, status_area, &theme);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let breadcrumb = app.breadcrumb();
    if breadcrumb.is_empty() {
        return;
    }
    frame.render_widget(
        Line::from(Span::styled(
            format!("  {breadcrumb}"),
            Theme::DEFAULT.muted,
        )),
        area,
    );
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let hints = match app.step() {
        Step::SelectProvider => vec![],
        Step::InputForm => {
            let mut hints = vec![("Enter", "confirm"), ("Esc", "quit")];
            if app.form_fields().len() > 1 {
                hints.insert(0, ("Tab", "next field"));
            }
            hints
        }
        Step::Calendar => match app.calendar().phase() {
            CalendarPhase::Permission => match app.calendar().auth_status() {
                Some(CalendarPermissionState::NotDetermined) => {
                    vec![("Enter", "request access"), ("Esc", "quit")]
                }
                Some(CalendarPermissionState::Authorized) => {
                    vec![("Enter", "continue"), ("Esc", "quit")]
                }
                Some(CalendarPermissionState::Denied) => {
                    vec![("Enter", "reset"), ("Esc", "quit")]
                }
                None => vec![("Esc", "quit")],
            },
            CalendarPhase::Select => {
                vec![("Space", "toggle"), ("Enter", "confirm")]
            }
        },
        Step::Done => vec![],
    };

    frame.render_widget(KeyHints::new(theme).hints(hints), area);
}
