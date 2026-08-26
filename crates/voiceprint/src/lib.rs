//! Selects clean single-speaker audio spans from transcript words so a
//! speaker-embedding model can turn them into voiceprint candidates.

mod matching;

pub use matching::{
    MIN_UNIQUE_MARGIN, MIN_UNIQUE_SCORE, VoiceprintAssignment, VoiceprintSpeakerKey,
    collect_match_scores, cosine_similarity, pick_unique_voiceprint_assignments,
    remote_participant_human_ids,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanWord {
    pub start_ms: i64,
    pub end_ms: i64,
    pub channel: i64,
    pub speaker_index: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectedSpan {
    pub channel: i64,
    pub speaker_index: Option<i64>,
    pub start_ms: i64,
    pub end_ms: i64,
    pub quality_score: f64,
}

impl SelectedSpan {
    pub fn duration_ms(&self) -> i64 {
        self.end_ms - self.start_ms
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SpanConfig {
    pub merge_gap_ms: i64,
    pub min_span_ms: i64,
    pub max_span_ms: i64,
    pub max_spans_per_speaker: usize,
}

impl Default for SpanConfig {
    fn default() -> Self {
        Self {
            merge_gap_ms: 400,
            min_span_ms: 1_500,
            max_span_ms: 10_000,
            max_spans_per_speaker: 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Run {
    channel: i64,
    speaker_index: Option<i64>,
    start_ms: i64,
    end_ms: i64,
}

/// Returns the longest clean spans per (channel, speaker), longest first
/// within each speaker. Portions where two speakers overlap on the same
/// channel are cut out entirely: overlapped speech contaminates embeddings.
pub fn select_speaker_spans(words: &[SpanWord], config: &SpanConfig) -> Vec<SelectedSpan> {
    let runs = merge_runs(words, config.merge_gap_ms);

    let mut selected = Vec::new();
    let mut index = 0;
    while index < runs.len() {
        let key = (runs[index].channel, runs[index].speaker_index);
        let speaker_runs: Vec<Run> = runs[index..]
            .iter()
            .take_while(|run| (run.channel, run.speaker_index) == key)
            .copied()
            .collect();
        index += speaker_runs.len();

        let others: Vec<Run> = runs
            .iter()
            .filter(|run| run.channel == key.0 && run.speaker_index != key.1)
            .copied()
            .collect();

        let mut spans: Vec<SelectedSpan> = speaker_runs
            .iter()
            .flat_map(|run| subtract_overlaps(*run, &others))
            .map(|run| clip_span(run, config.max_span_ms))
            .filter(|run| run.end_ms - run.start_ms >= config.min_span_ms)
            .map(|run| SelectedSpan {
                channel: run.channel,
                speaker_index: run.speaker_index,
                start_ms: run.start_ms,
                end_ms: run.end_ms,
                quality_score: quality(run, config.max_span_ms),
            })
            .collect();

        spans.sort_by_key(|span| std::cmp::Reverse(span.duration_ms()));
        spans.truncate(config.max_spans_per_speaker);
        selected.extend(spans);
    }

    selected
}

fn merge_runs(words: &[SpanWord], merge_gap_ms: i64) -> Vec<Run> {
    let mut sorted: Vec<SpanWord> = words
        .iter()
        .filter(|word| word.end_ms > word.start_ms)
        .copied()
        .collect();
    sorted.sort_by_key(|word| (word.channel, word.speaker_index, word.start_ms));

    let mut runs: Vec<Run> = Vec::new();
    for word in sorted {
        match runs.last_mut() {
            Some(run)
                if run.channel == word.channel
                    && run.speaker_index == word.speaker_index
                    && word.start_ms - run.end_ms <= merge_gap_ms
                    && word.start_ms >= run.start_ms =>
            {
                run.end_ms = run.end_ms.max(word.end_ms);
            }
            _ => runs.push(Run {
                channel: word.channel,
                speaker_index: word.speaker_index,
                start_ms: word.start_ms,
                end_ms: word.end_ms,
            }),
        }
    }

    runs.sort_by_key(|run| (run.channel, run.speaker_index, run.start_ms));
    runs
}

fn subtract_overlaps(run: Run, others: &[Run]) -> Vec<Run> {
    let mut pieces = vec![run];
    for other in others {
        let mut next = Vec::new();
        for piece in pieces {
            if other.end_ms <= piece.start_ms || other.start_ms >= piece.end_ms {
                next.push(piece);
                continue;
            }
            if other.start_ms > piece.start_ms {
                next.push(Run {
                    end_ms: other.start_ms,
                    ..piece
                });
            }
            if other.end_ms < piece.end_ms {
                next.push(Run {
                    start_ms: other.end_ms,
                    ..piece
                });
            }
        }
        pieces = next;
    }
    pieces
}

fn clip_span(run: Run, max_span_ms: i64) -> Run {
    if run.end_ms - run.start_ms <= max_span_ms {
        return run;
    }
    // Keep the middle of long spans: edges border silence or turn-taking and
    // carry more hesitation noise than mid-utterance speech.
    let excess = (run.end_ms - run.start_ms) - max_span_ms;
    Run {
        start_ms: run.start_ms + excess / 2,
        end_ms: run.start_ms + excess / 2 + max_span_ms,
        ..run
    }
}

fn quality(run: Run, max_span_ms: i64) -> f64 {
    let duration = (run.end_ms - run.start_ms) as f64;
    (duration / max_span_ms as f64).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(start_ms: i64, end_ms: i64, channel: i64, speaker_index: Option<i64>) -> SpanWord {
        SpanWord {
            start_ms,
            end_ms,
            channel,
            speaker_index,
        }
    }

    #[test]
    fn merges_gapped_words_into_one_span() {
        let words = [
            word(0, 900, 1, Some(0)),
            word(1_100, 2_000, 1, Some(0)),
            word(2_200, 3_100, 1, Some(0)),
        ];
        let spans = select_speaker_spans(&words, &SpanConfig::default());
        assert_eq!(spans.len(), 1);
        assert_eq!((spans[0].start_ms, spans[0].end_ms), (0, 3_100));
    }

    #[test]
    fn long_silence_splits_spans_and_short_pieces_are_dropped() {
        let words = [word(0, 2_000, 1, Some(0)), word(8_000, 8_600, 1, Some(0))];
        let spans = select_speaker_spans(&words, &SpanConfig::default());
        assert_eq!(spans.len(), 1);
        assert_eq!((spans[0].start_ms, spans[0].end_ms), (0, 2_000));
    }

    #[test]
    fn overlapped_speech_on_same_channel_is_cut_out() {
        let words = [word(0, 6_000, 2, Some(0)), word(3_500, 4_000, 2, Some(1))];
        let spans = select_speaker_spans(&words, &SpanConfig::default());
        let speaker0: Vec<_> = spans
            .iter()
            .filter(|span| span.speaker_index == Some(0))
            .collect();
        assert_eq!(speaker0.len(), 2);
        assert_eq!(
            speaker0
                .iter()
                .map(|span| (span.start_ms, span.end_ms))
                .collect::<Vec<_>>(),
            vec![(0, 3_500), (4_000, 6_000)]
        );
        assert!(!spans.iter().any(|span| span.speaker_index == Some(1)));
    }

    #[test]
    fn different_channels_do_not_suppress_each_other() {
        let words = [word(0, 3_000, 0, None), word(500, 2_800, 1, Some(0))];
        let spans = select_speaker_spans(&words, &SpanConfig::default());
        assert_eq!(spans.len(), 2);
    }

    #[test]
    fn long_spans_keep_their_middle_and_speakers_keep_top_spans() {
        let config = SpanConfig {
            max_spans_per_speaker: 1,
            ..SpanConfig::default()
        };
        let words = [
            word(0, 30_000, 1, Some(3)),
            word(40_000, 42_000, 1, Some(3)),
        ];
        let spans = select_speaker_spans(&words, &config);
        assert_eq!(spans.len(), 1);
        assert_eq!((spans[0].start_ms, spans[0].end_ms), (10_000, 20_000));
        assert_eq!(spans[0].quality_score, 1.0);
    }

    #[test]
    fn zero_length_words_are_ignored() {
        let words = [word(100, 100, 1, Some(0))];
        assert!(select_speaker_spans(&words, &SpanConfig::default()).is_empty());
    }
}
