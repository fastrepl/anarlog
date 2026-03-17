use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Padding, Paragraph},
};

use crate::commands::listen::app::App;
use crate::theme::Theme;
use crate::widgets::{Scrollable, build_segment_lines};

pub(super) fn draw_transcript(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    elapsed: std::time::Duration,
    theme: &Theme,
) {
    let segments = app.segments();

    let border_style = if app.transcript_focused() {
        theme.border_focused
    } else {
        theme.border
    };

    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Transcript ")
        .padding(Padding::new(1, 1, 0, 0));

    let inner_area = block.inner(area);

    if segments.is_empty() {
        let empty_message = if app.can_accept_audio_drop() {
            "Drop an audio file to transcribe..."
        } else {
            "Waiting for speech..."
        };
        let lines = vec![Line::from(Span::styled(empty_message, theme.placeholder))];
        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    app.check_new_segments(segments.len(), inner_area);

    let content_width = area.width.saturating_sub(4) as usize;
    let word_age_fn = |id: &str| app.word_age_secs(id);
    let lines = build_segment_lines(&segments, theme, content_width, Some(&word_age_fn));

    let scrollable = Scrollable::new(lines).block(block);
    let scroll_state = app.scroll_state_mut();
    frame.render_stateful_widget(scrollable, area, scroll_state);

    app.process_effects(elapsed, frame.buffer_mut(), inner_area);
}
