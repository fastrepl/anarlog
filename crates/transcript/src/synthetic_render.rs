use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::ChannelProfile;
use crate::render::{
    RenderTranscriptInput, RenderTranscriptRequest, RenderedTranscriptSegment, SyntheticTiming,
    render_transcript_segments_ungrouped,
};

#[derive(Default)]
struct Source {
    offset_ms: i64,
    transcript_index: usize,
    legacy_run: Option<usize>,
    timing: Option<SyntheticTiming>,
}

#[derive(Default)]
struct LegacyRun {
    start_ms: Option<i64>,
    channels: HashSet<i32>,
}

// Providers like Soniqo batch emit per-channel text without real word
// alignment: every word gets a synthetic timestamp, so naive chronological
// interleaving alternates channels every few words. Words that carry
// `synthetic_timing` are instead grouped into per-channel blocks per chunk
// (or per legacy run when chunk starts are absent) for display only.
pub(crate) fn render_segments(
    mut request: RenderTranscriptRequest,
) -> Vec<RenderedTranscriptSegment> {
    if !request.transcripts.iter().any(|transcript| {
        transcript
            .words
            .iter()
            .any(|word| word.synthetic_timing.is_some())
    }) {
        return render_transcript_segments_ungrouped(request);
    }

    let mut transcripts = Vec::new();
    for transcript in request.transcripts {
        transcripts.extend(split_synthetic_transcript(transcript));
    }

    let base_started_at = transcripts
        .iter()
        .filter_map(|row| row.started_at)
        .min()
        .unwrap_or(0);
    let mut source_by_id = HashMap::new();
    let mut legacy_runs: Vec<LegacyRun> = Vec::new();
    let mut previous_was_legacy = false;
    let mut channels = HashSet::new();
    let mut has_complete_synthetic_chunk = false;
    for (transcript_index, transcript) in transcripts.iter().enumerate() {
        let offset_ms = transcript
            .started_at
            .map_or(0, |start| start - base_started_at);
        let timings: Vec<_> = transcript
            .words
            .iter()
            .map(|word| word.synthetic_timing)
            .collect();
        let legacy = !timings.is_empty()
            && timings.iter().all(|timing| timing.is_some())
            && timings
                .iter()
                .any(|timing| timing.and_then(|t| t.chunk_start_ms).is_none());
        if legacy && !previous_was_legacy {
            legacy_runs.push(LegacyRun::default());
        }
        previous_was_legacy = legacy;
        let legacy_run = legacy.then(|| legacy_runs.len() - 1);
        for (word, timing) in transcript.words.iter().zip(timings) {
            channels.insert(word.channel);
            has_complete_synthetic_chunk |= timing.and_then(|t| t.chunk_start_ms).is_some();
            if let Some(run_index) = legacy_run {
                let run = &mut legacy_runs[run_index];
                let start_ms = word.start_ms + offset_ms;
                run.start_ms = Some(run.start_ms.map_or(start_ms, |start| start.min(start_ms)));
                run.channels.insert(word.channel);
            }
            source_by_id.insert(
                word.id.clone(),
                Source {
                    offset_ms,
                    transcript_index,
                    legacy_run,
                    timing,
                },
            );
        }
    }
    request.transcripts = transcripts;
    let mut segments = render_transcript_segments_ungrouped(request);
    if legacy_runs.iter().any(|run| run.channels.len() > 1)
        || (channels.len() > 1 && has_complete_synthetic_chunk)
    {
        segments.sort_by(|left, right| {
            let left_key = sort_key(left, &source_by_id, &legacy_runs);
            let right_key = sort_key(right, &source_by_id, &legacy_runs);
            left_key
                .0
                .total_cmp(&right_key.0)
                .then_with(|| left_key.1.cmp(&right_key.1))
                .then_with(|| {
                    if left_key.1 == 0 && right_key.1 == 0 {
                        left_key
                            .2
                            .cmp(&right_key.2)
                            .then_with(|| left_key.3.cmp(&right_key.3))
                            .then_with(|| left_key.4.cmp(&right_key.4))
                    } else if left_key.1 == 1 && right_key.1 == 1 {
                        left_key.3.cmp(&right_key.3)
                    } else {
                        std::cmp::Ordering::Equal
                    }
                })
                .then_with(|| left.start_ms.cmp(&right.start_ms))
        });
    }
    segments
}

pub fn synthetic_timing_from_metadata(metadata: Option<&Value>) -> Option<SyntheticTiming> {
    let metadata = metadata?;
    if let Value::String(json) = metadata {
        return serde_json::from_str::<Value>(json)
            .ok()
            .and_then(|value| synthetic_timing_from_metadata(Some(&value)));
    }
    let timing = metadata.get("timing").filter(|value| value.is_object())?;
    if timing.get("source").and_then(Value::as_str) != Some("synthetic_text") {
        return None;
    }
    Some(SyntheticTiming {
        chunk_start_ms: timing
            .get("chunk_start_ms")
            .and_then(Value::as_f64)
            .filter(|start| start.is_finite()),
    })
}

fn split_synthetic_transcript(transcript: RenderTranscriptInput) -> Vec<RenderTranscriptInput> {
    let mut groups: Vec<(i32, f64, Vec<_>)> = Vec::new();
    let mut group_by_key = HashMap::new();
    let mut channels = HashSet::new();
    let mut timed_words = Vec::new();
    let mut legacy = false;
    for word in &transcript.words {
        channels.insert(word.channel);
        let Some(timing) = word.synthetic_timing else {
            timed_words.push(word.clone());
            continue;
        };
        let Some(chunk_start_ms) = timing.chunk_start_ms else {
            legacy = true;
            continue;
        };
        let key = (
            word.channel,
            if chunk_start_ms == 0.0 {
                0
            } else {
                chunk_start_ms.to_bits()
            },
        );
        let index = *group_by_key.entry(key).or_insert_with(|| {
            groups.push((word.channel, chunk_start_ms, Vec::new()));
            groups.len() - 1
        });
        groups[index].2.push(word.clone());
    }
    if channels.len() < 2 {
        return vec![transcript];
    }
    if legacy {
        if !timed_words.is_empty() {
            return vec![transcript];
        }
        let mut channel_words = HashMap::<i32, Vec<_>>::new();
        for word in &transcript.words {
            channel_words
                .entry(word.channel)
                .or_default()
                .push(word.clone());
        }
        let mut groups: Vec<_> = channel_words.into_iter().collect();
        groups.sort_by_key(|group| group.0);
        return groups
            .into_iter()
            .map(|(_, words)| RenderTranscriptInput {
                started_at: transcript.started_at,
                words,
                assignments: transcript.assignments.clone(),
            })
            .collect();
    }
    if groups.is_empty() {
        return vec![transcript];
    }
    if !timed_words.is_empty() {
        // A timed word belongs to the latest synthetic chunk that started at
        // or before it; merging all of them into one trailing group lets the
        // renderer bridge speech across intervening chunks.
        let mut chunk_starts: Vec<f64> = groups.iter().map(|group| group.1).collect();
        chunk_starts.sort_by(f64::total_cmp);
        chunk_starts.dedup();
        let mut timed_groups: Vec<(f64, Vec<_>)> = Vec::new();
        for word in timed_words {
            let window = chunk_starts
                .iter()
                .rev()
                .find(|start| **start <= word.start_ms as f64)
                .copied()
                .unwrap_or(f64::NEG_INFINITY);
            match timed_groups.iter_mut().find(|group| group.0 == window) {
                Some(group) => group.1.push(word),
                None => timed_groups.push((window, vec![word])),
            }
        }
        for (window, words) in timed_groups {
            let start = if window.is_infinite() {
                words.iter().map(|word| word.start_ms).min().unwrap() as f64
            } else {
                window
            };
            groups.push((-1, start, words));
        }
    }
    groups.sort_by(|left, right| {
        left.1
            .total_cmp(&right.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    groups
        .into_iter()
        .map(|(_, _, words)| RenderTranscriptInput {
            started_at: transcript.started_at,
            words,
            assignments: transcript.assignments.clone(),
        })
        .collect()
}

fn sort_key(
    segment: &RenderedTranscriptSegment,
    source_by_id: &HashMap<String, Source>,
    legacy_runs: &[LegacyRun],
) -> (f64, u8, Option<usize>, u8, Option<usize>) {
    let source = segment
        .words
        .first()
        .and_then(|word| word.id.as_ref())
        .and_then(|id| source_by_id.get(id));
    let run = source
        .and_then(|source| source.legacy_run)
        .and_then(|index| legacy_runs.get(index))
        .filter(|run| run.channels.len() > 1);
    let chunk_start = source.and_then(|source| source.timing.and_then(|t| t.chunk_start_ms));
    let (start_ms, group) = if let Some(run) = run {
        (run.start_ms.unwrap_or(segment.start_ms) as f64, 0)
    } else if let Some(start) = chunk_start {
        (
            start + source.map_or(0, |source| source.offset_ms) as f64,
            1,
        )
    } else {
        (segment.start_ms as f64, 2)
    };
    let channel = match segment.key.channel {
        ChannelProfile::DirectMic => 0,
        ChannelProfile::RemoteParty => 1,
        ChannelProfile::MixedCapture => 2,
    };
    (
        start_ms,
        group,
        source.and_then(|source| source.legacy_run),
        channel,
        source.map(|source| source.transcript_index),
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{
        RenderTranscriptWordInput, StoredSpeakerHint, StoredTranscriptWord,
        render_input_from_stored,
    };

    fn stored_word(
        id: &str,
        start_ms: i64,
        channel: i32,
        timing: Option<Value>,
    ) -> StoredTranscriptWord {
        StoredTranscriptWord {
            id: id.to_string(),
            text: Some(format!(" {id}")),
            start_ms: Some(start_ms as f64),
            end_ms: Some((start_ms + 400) as f64),
            channel: Some(channel as f64),
            speaker: None,
            metadata: timing.map(|timing| json!({"timing": timing})),
        }
    }

    fn synthetic(chunk: Option<i64>) -> Option<Value> {
        Some(json!({"source": "synthetic_text", "chunk_start_ms": chunk}))
    }

    fn request() -> RenderTranscriptRequest {
        RenderTranscriptRequest {
            transcripts: vec![],
            participant_human_ids: vec![],
            self_human_id: None,
            humans: vec![],
            speaker_context: None,
            preview: None,
        }
    }

    fn render_rows(
        rows: Vec<(
            Option<i64>,
            Vec<StoredTranscriptWord>,
            Vec<StoredSpeakerHint>,
        )>,
        mut request: RenderTranscriptRequest,
    ) -> Vec<RenderedTranscriptSegment> {
        request.transcripts = rows
            .into_iter()
            .filter_map(|(started_at, words, hints)| {
                render_input_from_stored(started_at, &words, &hints)
            })
            .collect();
        crate::render_transcript_segments(request)
    }

    fn word_ids(segments: &[RenderedTranscriptSegment]) -> Vec<Vec<&str>> {
        segments
            .iter()
            .map(|segment| {
                segment
                    .words
                    .iter()
                    .map(|word| word.id.as_deref().unwrap())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn synthetic_chunks_keep_channel_sentences_and_word_ids_with_assignments() {
        let words = vec![
            stored_word("mic-1", 0, 0, synthetic(Some(0))),
            stored_word("remote-1", 0, 1, synthetic(Some(0))),
            stored_word("mic-2", 400, 0, synthetic(Some(0))),
            stored_word("remote-2", 400, 1, synthetic(Some(0))),
            stored_word("mic-next", 10_000, 0, synthetic(Some(10_000))),
            stored_word("remote-next", 10_000, 1, synthetic(Some(10_000))),
        ];
        let mut hints = vec![StoredSpeakerHint {
            id: "remote-assignment".into(),
            word_id: Some("remote-1".into()),
            hint_type: "user_speaker_assignment".into(),
            value: json!({"human_id": "guest", "scope": "speaker", "channel": 1, "speaker_index": 7}),
        }];
        hints.extend(
            ["remote-1", "remote-2", "remote-next"].map(|id| StoredSpeakerHint {
                id: format!("{id}:provider_speaker_index"),
                word_id: Some(id.into()),
                hint_type: "provider_speaker_index".into(),
                value: json!({"channel": 1, "speaker_index": 7}),
            }),
        );
        let mut context = request();
        context.humans.push(crate::RenderTranscriptHuman {
            human_id: "guest".into(),
            name: "Guest".into(),
        });
        let segments = render_rows(vec![(Some(0), words, hints)], context);
        assert_eq!(
            word_ids(&segments),
            vec![
                vec!["mic-1", "mic-2"],
                vec!["remote-1", "remote-2"],
                vec!["mic-next"],
                vec!["remote-next"]
            ]
        );
        assert_eq!(segments[1].speaker_label, "Guest");
        assert_eq!(segments[3].key.speaker_human_id.as_deref(), Some("guest"));
        assert_eq!(segments[1].words[1].start_ms, 400);
    }

    #[test]
    fn legacy_channel_runs_do_not_pull_later_rows_across_accurate_provider_words() {
        let rows = vec![
            (
                Some(1000),
                vec![
                    stored_word("mic-old", 0, 0, synthetic(None)),
                    stored_word("mic-old-2", 400, 0, synthetic(None)),
                ],
                vec![],
            ),
            (
                Some(1000),
                vec![
                    stored_word("remote-old", 0, 1, synthetic(None)),
                    stored_word("remote-old-2", 400, 1, synthetic(None)),
                ],
                vec![],
            ),
            (
                Some(2000),
                vec![
                    stored_word("timed-remote", 0, 1, None),
                    stored_word("timed-mic", 1000, 0, None),
                ],
                vec![],
            ),
            (
                Some(4000),
                vec![
                    stored_word("remote-later", 0, 1, synthetic(None)),
                    stored_word("mic-later", 400, 0, synthetic(None)),
                ],
                vec![],
            ),
        ];
        let segments = render_rows(rows, request());
        assert_eq!(
            word_ids(&segments),
            vec![
                vec!["mic-old", "mic-old-2"],
                vec!["remote-old", "remote-old-2"],
                vec!["timed-remote"],
                vec!["timed-mic"],
                vec!["mic-later"],
                vec!["remote-later"]
            ]
        );
        assert_eq!(segments[2].start_ms, 1000);
        assert_eq!(segments[3].start_ms, 2000);
    }

    #[test]
    fn synthetic_order_uses_row_offsets_and_preserves_accurate_word_chronology() {
        let mut encoded = stored_word("encoded-mic", 400, 0, synthetic(Some(0)));
        encoded.metadata = Some(Value::String(encoded.metadata.unwrap().to_string()));
        let rows = vec![
            (
                Some(5000),
                vec![
                    stored_word("late-remote", 0, 1, synthetic(Some(0))),
                    encoded,
                ],
                vec![],
            ),
            (
                Some(1000),
                vec![
                    stored_word("early-timed-remote", 0, 1, None),
                    stored_word("early-timed-mic", 1000, 0, None),
                ],
                vec![],
            ),
            (
                Some(3000),
                vec![
                    stored_word("mixed-remote", 0, 1, synthetic(None)),
                    stored_word("mixed-timed", 1000, 0, None),
                ],
                vec![],
            ),
        ];
        let segments = render_rows(rows, request());
        assert_eq!(
            word_ids(&segments),
            vec![
                vec!["early-timed-remote"],
                vec!["early-timed-mic"],
                vec!["mixed-remote"],
                vec!["mixed-timed"],
                vec!["encoded-mic"],
                vec!["late-remote"]
            ]
        );
        assert_eq!(segments[4].start_ms, 4400);
        assert_eq!(segments[5].start_ms, 4000);
    }

    #[test]
    fn accurate_provider_timestamps_keep_chronology_without_synthetic_grouping() {
        let rows = vec![
            (
                Some(5000),
                vec![stored_word("late-remote", 0, 1, None)],
                vec![],
            ),
            (
                Some(1000),
                vec![stored_word("early-mic", 0, 0, None)],
                vec![],
            ),
            (
                Some(2000),
                vec![stored_word("middle-remote", 0, 1, None)],
                vec![],
            ),
            (
                Some(3000),
                vec![stored_word("middle-mic", 0, 0, None)],
                vec![],
            ),
        ];
        let segments = render_rows(rows, request());
        assert_eq!(
            word_ids(&segments),
            vec![
                vec!["early-mic"],
                vec!["middle-remote"],
                vec!["middle-mic"],
                vec!["late-remote"]
            ]
        );
        assert_eq!(
            segments
                .iter()
                .map(|segment| segment.start_ms)
                .collect::<Vec<_>>(),
            vec![0, 1000, 2000, 4000]
        );
    }

    #[test]
    fn synthetic_channel_grouping_preserves_speaker_context_boundaries() {
        let rows = vec![(
            Some(1000),
            vec![
                stored_word("remote-a", 0, 1, synthetic(Some(0))),
                stored_word("mic-a", 0, 0, synthetic(Some(0))),
                stored_word("remote-b", 1000, 1, synthetic(Some(0))),
                stored_word("mic-b", 1000, 0, synthetic(Some(0))),
            ],
            vec![],
        )];
        let mut context = request();
        context.self_human_id = Some("owner".into());
        context.speaker_context = Some(crate::SpeakerContext {
            intervals: [(1000, 1600, "a", "Alice"), (1600, 4000, "b", "Bob")]
                .map(
                    |(start_ms, end_ms, human_id, name)| crate::SpeakerContextInterval {
                        start_ms,
                        end_ms,
                        active_call: true,
                        calendar_call: false,
                        mic_isolated: None,
                        shared_microphone: false,
                        title: String::new(),
                        self_names: vec![],
                        participants: vec![crate::RenderTranscriptHuman {
                            human_id: human_id.into(),
                            name: name.into(),
                        }],
                    },
                )
                .into(),
        });
        let segments = render_rows(rows, context);
        let remote: Vec<_> = segments
            .iter()
            .filter(|segment| segment.key.channel == ChannelProfile::RemoteParty)
            .collect();
        assert_eq!(
            remote
                .iter()
                .map(|segment| segment.speaker_label.as_str())
                .collect::<Vec<_>>(),
            vec!["Alice", "Bob"]
        );
        assert_eq!(remote[0].words[0].id.as_deref(), Some("remote-a"));
        assert_eq!(remote[1].words[0].id.as_deref(), Some("remote-b"));
        assert_eq!(remote[1].start_ms, 1000);
    }

    #[test]
    fn timed_words_split_into_their_own_chunk_windows() {
        let rows = vec![(
            Some(0),
            vec![
                stored_word("mic-first", 0, 0, synthetic(Some(0))),
                stored_word("remote-first", 0, 1, synthetic(Some(0))),
                stored_word("timed-early", 5_000, 2, None),
                stored_word("mic-second", 30_000, 0, synthetic(Some(30_000))),
                stored_word("remote-second", 30_000, 1, synthetic(Some(30_000))),
                stored_word("timed-late", 35_000, 2, None),
            ],
            vec![StoredSpeakerHint {
                id: "timed-provider".into(),
                word_id: None,
                hint_type: "provider_speaker_index".into(),
                value: json!({"channel": 2, "speaker_index": 3}),
            }],
        )];
        let segments = render_rows(rows, request());
        assert_eq!(
            word_ids(&segments),
            vec![
                vec!["mic-first"],
                vec!["remote-first"],
                vec!["timed-early"],
                vec!["mic-second"],
                vec!["remote-second"],
                vec!["timed-late"]
            ]
        );
    }

    #[test]
    fn request_words_can_carry_synthetic_timing_directly() {
        fn word(
            id: &str,
            start_ms: i64,
            channel: i32,
            chunk_start_ms: i64,
        ) -> RenderTranscriptWordInput {
            RenderTranscriptWordInput {
                id: id.to_string(),
                text: format!(" {id}"),
                start_ms,
                end_ms: start_ms + 400,
                channel,
                speaker_index: None,
                synthetic_timing: Some(SyntheticTiming {
                    chunk_start_ms: Some(chunk_start_ms as f64),
                }),
            }
        }
        let mut context = request();
        context.transcripts = vec![RenderTranscriptInput {
            started_at: Some(0),
            words: vec![
                word("mic-1", 0, 0, 0),
                word("remote-1", 0, 1, 0),
                word("mic-2", 400, 0, 0),
                word("remote-2", 400, 1, 0),
            ],
            assignments: vec![],
        }];
        let segments = crate::render_transcript_segments(context);
        assert_eq!(
            word_ids(&segments),
            vec![vec!["mic-1", "mic-2"], vec!["remote-1", "remote-2"]]
        );
    }
}
