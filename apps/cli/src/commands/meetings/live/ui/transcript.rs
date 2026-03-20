use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Padding, Paragraph},
};

use super::super::app::App;
use crate::theme::Theme;
use crate::widgets::{build_segment_lines, render_scrollable};

pub(super) fn draw_transcript(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    elapsed: std::time::Duration,
    theme: &Theme,
) {
    let segments = app.segments();

    let block = theme
        .bordered_block(app.transcript_focused())
        .title(" Transcript ")
        .padding(Padding::new(1, 1, 0, 0));

    let inner_area = block.inner(area);

    if segments.is_empty() {
        let lines = vec![Line::from(Span::styled(
            "Waiting for speech...",
            theme.placeholder,
        ))];
        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    app.check_new_segments(segments.len(), inner_area);

    let content_width = area.width.saturating_sub(4) as usize;
    let word_age_fn = |id: &str| app.word_age_secs(id);
    let ctx = app.speaker_label_context();
    let lines = build_segment_lines(
        &segments,
        theme,
        content_width,
        Some(&word_age_fn),
        ctx.as_ref(),
    );

    render_scrollable(frame, lines, Some(block), area, app.scroll_state_mut());

    app.process_effects(elapsed, frame.buffer_mut(), inner_area);
}
