use hypr_transcript::{Segment, SegmentWord, SpeakerLabelContext, SpeakerLabeler};
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use crate::output::format_timestamp_ms;
use crate::theme::Theme;

const FADE_IN_SECS: f64 = 0.4;

pub fn build_segment_lines<'a>(
    segments: &[Segment],
    theme: &Theme,
    content_width: usize,
    word_age_fn: Option<&dyn Fn(&str) -> f64>,
    speaker_ctx: Option<&SpeakerLabelContext>,
) -> Vec<Line<'a>> {
    let mut lines: Vec<Line> = Vec::new();
    let mut labeler = SpeakerLabeler::from_segments(segments, speaker_ctx);

    for (i, segment) in segments.iter().enumerate() {
        if i > 0 {
            lines.push(Line::default());
        }

        let label = labeler.label_for(&segment.key, speaker_ctx);
        let timestamp = segment
            .words
            .first()
            .map(|w| format_timestamp_ms(w.start_ms))
            .unwrap_or_default();

        lines.push(Line::from(vec![
            Span::styled(label, theme.speaker_label),
            Span::raw(" "),
            Span::styled(format!("[{timestamp}]"), theme.timestamp),
        ]));

        let indent = "  ";
        let wrap_width = content_width.saturating_sub(indent.len());

        if wrap_width == 0 {
            continue;
        }

        let mut current_spans: Vec<Span> = vec![Span::raw(indent.to_string())];
        let mut current_len = 0usize;

        for (j, word) in segment.words.iter().enumerate() {
            let text = &word.text;
            let separator = if j > 0 { " " } else { "" };
            let needed = separator.len() + text.len();

            if current_len > 0 && current_len + needed > wrap_width {
                lines.push(Line::from(std::mem::take(&mut current_spans)));
                current_spans = vec![Span::raw(indent.to_string())];
                current_len = 0;
            } else if !separator.is_empty() {
                current_spans.push(Span::raw(separator.to_string()));
                current_len += separator.len();
            }

            let style = word_style(word, theme, word_age_fn);
            current_spans.push(Span::styled(text.clone(), style));
            current_len += text.len();
        }

        if current_len > 0 {
            lines.push(Line::from(current_spans));
        }
    }

    lines
}

fn word_style(
    word: &SegmentWord,
    theme: &Theme,
    word_age_fn: Option<&dyn Fn(&str) -> f64>,
) -> Style {
    if !word.is_final {
        return theme.transcript_partial;
    }

    if let (Some(id), Some(age_fn)) = (&word.id, word_age_fn) {
        let age = age_fn(id);
        if age < FADE_IN_SECS {
            return fade_in_style(age);
        }
    }

    theme.transcript_final
}

fn fade_in_style(age: f64) -> Style {
    let t = (age / FADE_IN_SECS).clamp(0.0, 1.0);
    let t = ease_out_cubic(t);
    let start = 50u8;
    let end = 220u8;
    let v = start + ((end - start) as f64 * t) as u8;
    Style::new().fg(Color::Rgb(v, v, v))
}

fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}
