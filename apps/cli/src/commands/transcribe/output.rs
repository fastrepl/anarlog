use colored::Colorize;

use crate::output::format_timestamp_secs;

const PAUSE_THRESHOLD_SECS: f64 = 0.5;

const SPEAKER_COLORS: &[colored::Color] = &[
    colored::Color::Cyan,
    colored::Color::Green,
    colored::Color::Yellow,
    colored::Color::Magenta,
    colored::Color::Blue,
    colored::Color::Red,
];

fn speaker_color(speaker: usize) -> colored::Color {
    SPEAKER_COLORS[speaker % SPEAKER_COLORS.len()]
}

struct Segment<'a> {
    start: f64,
    end: f64,
    words: Vec<&'a str>,
    identity: usize,
}

pub(super) fn format_pretty(response: &owhisper_interface::batch::Response) -> String {
    let mut segments: Vec<Segment> = Vec::new();
    let num_channels = response.results.channels.len();

    struct TaggedWord<'a> {
        text: &'a str,
        start: f64,
        end: f64,
        identity: usize,
    }

    let mut all_words: Vec<TaggedWord> = Vec::new();
    for (channel_idx, channel) in response.results.channels.iter().enumerate() {
        let Some(alt) = channel.alternatives.first() else {
            continue;
        };
        for word in &alt.words {
            all_words.push(TaggedWord {
                text: word
                    .punctuated_word
                    .as_deref()
                    .unwrap_or(word.word.as_str()),
                start: word.start,
                end: word.end,
                identity: word_identity(word, channel_idx, num_channels),
            });
        }
    }
    all_words.sort_by(|a, b| {
        a.start
            .partial_cmp(&b.start)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for word in &all_words {
        let should_split = segments
            .last()
            .map(|seg| word.start - seg.end > PAUSE_THRESHOLD_SECS || word.identity != seg.identity)
            .unwrap_or(true);

        if should_split {
            segments.push(Segment {
                start: word.start,
                end: word.end,
                words: vec![word.text],
                identity: word.identity,
            });
        } else if let Some(seg) = segments.last_mut() {
            seg.end = word.end;
            seg.words.push(word.text);
        }
    }

    if segments.is_empty() {
        return extract_transcript(response);
    }

    let term_width = textwrap::termwidth();
    let show_speaker =
        num_channels > 1 || segments.iter().any(|s| s.identity != segments[0].identity);

    segments
        .iter()
        .map(|seg| {
            let timestamp = format!(
                "[{} \u{2192} {}]",
                format_timestamp_secs(seg.start),
                format_timestamp_secs(seg.end)
            )
            .dimmed()
            .to_string();

            let label = format!("{}  ", timestamp);
            let text = seg.words.join(" ");
            let text = if show_speaker {
                text.color(speaker_color(seg.identity)).to_string()
            } else {
                text
            };

            let visible_prefix_len = 22;
            let wrap_width = term_width.saturating_sub(visible_prefix_len);

            if wrap_width == 0 || text.len() <= wrap_width {
                format!("{}{}", label, text)
            } else {
                let indent = " ".repeat(visible_prefix_len);
                let wrapped = textwrap::fill(
                    &text,
                    textwrap::Options::new(wrap_width).subsequent_indent(""),
                );
                let mut lines = wrapped.lines();
                let first = lines.next().unwrap_or("");
                let rest: Vec<&str> = lines.collect();
                if rest.is_empty() {
                    format!("{}{}", label, first)
                } else {
                    format!(
                        "{}{}\n{}",
                        label,
                        first,
                        rest.iter()
                            .map(|l| format!("{}{}", indent, l))
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                }
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn word_identity(
    word: &owhisper_interface::batch::Word,
    channel_idx: usize,
    total_channels: usize,
) -> usize {
    if total_channels > 1 {
        channel_idx
    } else {
        word.speaker.unwrap_or(word.channel.max(0) as usize)
    }
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

#[cfg(test)]
mod tests {
    use owhisper_interface::batch;

    use super::*;

    fn response_with_channels(channel_words: Vec<Vec<batch::Word>>) -> batch::Response {
        batch::Response {
            metadata: serde_json::json!({}),
            results: batch::Results {
                channels: channel_words
                    .into_iter()
                    .map(|words| batch::Channel {
                        alternatives: vec![batch::Alternatives {
                            transcript: words
                                .iter()
                                .map(|word| word.word.as_str())
                                .collect::<Vec<_>>()
                                .join(" "),
                            confidence: 1.0,
                            words,
                        }],
                    })
                    .collect(),
            },
        }
    }

    fn word(text: &str, start: f64, end: f64, channel: i32, speaker: Option<usize>) -> batch::Word {
        batch::Word {
            word: text.to_string(),
            start,
            end,
            confidence: 1.0,
            channel,
            speaker,
            punctuated_word: Some(text.to_string()),
        }
    }

    #[test]
    fn pretty_output_splits_multichannel_words_by_channel() {
        colored::control::set_override(false);

        let response = response_with_channels(vec![
            vec![word("left", 0.0, 0.4, 0, None)],
            vec![word("right", 0.1, 0.5, 0, None)],
        ]);

        let pretty = format_pretty(&response);
        let blocks = pretty.split("\n\n").collect::<Vec<_>>();

        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].contains("left"));
        assert!(blocks[1].contains("right"));
    }

    #[test]
    fn pretty_output_splits_single_channel_words_by_speaker() {
        colored::control::set_override(false);

        let response = response_with_channels(vec![vec![
            word("hello", 0.0, 0.4, 0, Some(0)),
            word("again", 0.45, 0.8, 0, Some(1)),
        ]]);

        let pretty = format_pretty(&response);
        let blocks = pretty.split("\n\n").collect::<Vec<_>>();

        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].contains("hello"));
        assert!(blocks[1].contains("again"));
    }
}
