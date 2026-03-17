use hypr_listener_core::State;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::Span,
};

use crate::commands::listen::app::App;
use crate::output::format_hhmmss;
use crate::theme::Theme;
use crate::widgets::{InfoLine, Waveform};

const WAVEFORM_WIDTH: u16 = 20;

pub(super) fn draw_header_bar(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let state_style = match app.listener_state() {
        State::Active if app.degraded().is_some() => theme.status_degraded,
        State::Active => theme.status_active,
        State::Finalizing => theme.status_degraded,
        State::Inactive => theme.status_inactive,
    };

    let mut info = InfoLine::new(theme)
        .item(Span::styled(app.status(), state_style))
        .item(Span::raw(format_hhmmss(app.elapsed())))
        .item(Span::raw(format!("{} words", app.word_count())));

    if let Some(err) = app.last_error() {
        info = info.item(Span::styled(err, theme.error));
    }

    if app.mic_muted() {
        info = info.item(Span::styled("mic muted", theme.muted));
    }

    let waveform_width = WAVEFORM_WIDTH.min(area.width / 3);
    let [info_area, waveform_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(waveform_width)]).areas(area);

    frame.render_widget(info, info_area);

    let mic: Vec<u64> = app.mic_history().iter().copied().collect();
    let speaker: Vec<u64> = app.speaker_history().iter().copied().collect();
    frame.render_widget(
        Waveform::new(&mic, &speaker, theme).muted(app.mic_muted()),
        waveform_area,
    );
}
