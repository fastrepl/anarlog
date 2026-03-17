use colored::Colorize;

use crate::output::format_timestamp_secs;

const PAUSE_THRESHOLD_SECS: f64 = 0.5;

pub(super) fn format_pretty(response: &owhisper_interface::batch::Response) -> String {
    use owhisper_interface::batch::Word;

    let words: Vec<&Word> = response
        .results
        .channels
        .iter()
        .filter_map(|c| c.alternatives.first())
        .flat_map(|alt| &alt.words)
        .collect();

    if words.is_empty() {
        return extract_transcript(response);
    }

    let mut segments: Vec<(f64, f64, Vec<&str>)> = Vec::new();

    for word in &words {
        let text = word
            .punctuated_word
            .as_deref()
            .unwrap_or(word.word.as_str());

        let should_split = segments
            .last()
            .map(|(_, end, _)| word.start - *end > PAUSE_THRESHOLD_SECS)
            .unwrap_or(true);

        if should_split {
            segments.push((word.start, word.end, vec![text]));
        } else if let Some(seg) = segments.last_mut() {
            seg.1 = word.end;
            seg.2.push(text);
        }
    }

    let term_width = textwrap::termwidth();

    segments
        .iter()
        .map(|(start, end, words)| {
            let prefix = format!(
                "{}  ",
                format!(
                    "[{} \u{2192} {}]",
                    format_timestamp_secs(*start),
                    format_timestamp_secs(*end)
                )
                .dimmed(),
            );
            // "[00:00.0 → 00:00.0]  " = 22 visible chars
            let prefix_visible_len = 22;
            let indent = " ".repeat(prefix_visible_len);
            let text = words.join(" ");

            let opts = textwrap::Options::new(term_width)
                .initial_indent(&prefix)
                .subsequent_indent(&indent);
            textwrap::fill(&text, opts)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(super) fn extract_transcript(response: &owhisper_interface::batch::Response) -> String {
    response
        .results
        .channels
        .iter()
        .filter_map(|c| c.alternatives.first())
        .map(|alt| alt.transcript.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
