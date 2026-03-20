use hypr_transcript::{Segment, SpeakerLabelContext};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Padding, Paragraph},
};

use crate::theme::Theme;
use crate::widgets::{ScrollViewState, build_segment_lines, render_scrollable};

pub(crate) fn draw_transcript(
    frame: &mut Frame,
    segments: &[Segment],
    focused: bool,
    area: Rect,
    theme: &Theme,
    scroll_state: &mut ScrollViewState,
    empty_message: &str,
    word_age_fn: Option<&dyn Fn(&str) -> f64>,
    speaker_ctx: Option<&SpeakerLabelContext>,
) {
    let block = theme
        .bordered_block(focused)
        .title(" Transcript ")
        .padding(Padding::new(1, 1, 0, 0));

    if segments.is_empty() {
        let lines = vec![Line::from(Span::styled(empty_message, theme.placeholder))];
        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    let content_width = area.width.saturating_sub(4) as usize;
    let lines = build_segment_lines(segments, theme, content_width, word_age_fn, speaker_ctx);

    render_scrollable(frame, lines, Some(block), area, scroll_state);
}
