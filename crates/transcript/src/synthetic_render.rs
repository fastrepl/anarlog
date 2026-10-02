use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::{
    ChannelProfile, RenderTranscriptInput, RenderTranscriptRequest, RenderedTranscriptSegment,
    StoredSpeakerHint, StoredTranscriptWord, render_input_from_stored, render_transcript_segments,
};

#[derive(Clone, Copy, Default)]
struct Timing {
    synthetic: bool,
    chunk_start_ms: Option<f64>,
}

struct Source {
    offset_ms: i64,
    transcript_index: usize,
    legacy_run: Option<usize>,
    timing: Timing,
}

#[derive(Default)]
struct LegacyRun {
    start_ms: Option<i64>,
    channels: HashSet<i32>,
}

pub type StoredTranscriptRenderRow = (
    Option<i64>,
    Vec<StoredTranscriptWord>,
    Vec<StoredSpeakerHint>,
);

// Rendering may group estimated timing by channel. Identity/refinement must
// still consume each complete stored row through render_input_from_stored.
pub fn render_stored_transcript_segments(
    rows: Vec<StoredTranscriptRenderRow>,
    mut request: RenderTranscriptRequest,
) -> Option<Vec<RenderedTranscriptSegment>> {
    let mut timing_by_id = HashMap::new();
    let mut transcripts = Vec::new();
    for (started_at, words, hints) in rows {
        let Some(transcript) = render_input_from_stored(started_at, &words, &hints) else {
            continue;
        };
        timing_by_id.extend(
            words
                .iter()
                .map(|word| (word.id.clone(), synthetic_timing(word.metadata.as_ref()))),
        );
        transcripts.extend(split_synthetic_transcript(transcript, &timing_by_id));
    }
    if transcripts.is_empty() {
        return None;
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
            .map(|word| timing_by_id.get(&word.id).copied().unwrap_or_default())
            .collect();
        let legacy = !timings.is_empty()
            && timings.iter().all(|timing| timing.synthetic)
            && timings.iter().any(|timing| timing.chunk_start_ms.is_none());
        if legacy && !previous_was_legacy {
            legacy_runs.push(LegacyRun::default());
        }
        previous_was_legacy = legacy;
        let legacy_run = legacy.then(|| legacy_runs.len() - 1);
        for (word, timing) in transcript.words.iter().zip(timings) {
            channels.insert(word.channel);
            has_complete_synthetic_chunk |= timing.chunk_start_ms.is_some();
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
    let mut segments = render_transcript_segments(request);
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
    Some(segments)
}

fn synthetic_timing(metadata: Option<&Value>) -> Timing {
    let Some(metadata) = metadata else {
        return Timing::default();
    };
    if let Value::String(json) = metadata {
        return serde_json::from_str::<Value>(json)
            .ok()
            .map(|value| synthetic_timing(Some(&value)))
            .unwrap_or_default();
    }
    let Some(timing) = metadata.get("timing").filter(|value| value.is_object()) else {
        return Timing::default();
    };
    let synthetic = timing.get("source").and_then(Value::as_str) == Some("synthetic_text");
    Timing {
        synthetic,
        chunk_start_ms: synthetic
            .then(|| {
                timing
                    .get("chunk_start_ms")
                    .and_then(Value::as_f64)
                    .filter(|start| start.is_finite())
            })
            .flatten(),
    }
}

fn split_synthetic_transcript(
    transcript: RenderTranscriptInput,
    timing_by_id: &HashMap<String, Timing>,
) -> Vec<RenderTranscriptInput> {
    let mut groups: Vec<(i32, f64, Vec<_>)> = Vec::new();
    let mut group_by_key = HashMap::new();
    let mut channels = HashSet::new();
    let mut timed_words = Vec::new();
    let mut legacy = false;
    for word in &transcript.words {
        channels.insert(word.channel);
        let timing = timing_by_id.get(&word.id).copied().unwrap_or_default();
        if !timing.synthetic {
            timed_words.push(word.clone());
            continue;
        }
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
        let start = timed_words.iter().map(|word| word.start_ms).min().unwrap() as f64;
        groups.push((-1, start, timed_words));
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
    let chunk_start = source.and_then(|source| source.timing.chunk_start_ms);
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

    fn word(id: &str, start_ms: i64, channel: i32, timing: Option<Value>) -> StoredTranscriptWord {
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
            word("mic-1", 0, 0, synthetic(Some(0))),
            word("remote-1", 0, 1, synthetic(Some(0))),
            word("mic-2", 400, 0, synthetic(Some(0))),
            word("remote-2", 400, 1, synthetic(Some(0))),
            word("mic-next", 10_000, 0, synthetic(Some(10_000))),
            word("remote-next", 10_000, 1, synthetic(Some(10_000))),
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
        let segments =
            render_stored_transcript_segments(vec![(Some(0), words, hints)], context).unwrap();
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
                    word("mic-old", 0, 0, synthetic(None)),
                    word("mic-old-2", 400, 0, synthetic(None)),
                ],
                vec![],
            ),
            (
                Some(1000),
                vec![
                    word("remote-old", 0, 1, synthetic(None)),
                    word("remote-old-2", 400, 1, synthetic(None)),
                ],
                vec![],
            ),
            (
                Some(2000),
                vec![
                    word("timed-remote", 0, 1, None),
                    word("timed-mic", 1000, 0, None),
                ],
                vec![],
            ),
            (
                Some(4000),
                vec![
                    word("remote-later", 0, 1, synthetic(None)),
                    word("mic-later", 400, 0, synthetic(None)),
                ],
                vec![],
            ),
        ];
        let segments = render_stored_transcript_segments(rows, request()).unwrap();
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
        let mut encoded = word("encoded-mic", 400, 0, synthetic(Some(0)));
        encoded.metadata = Some(Value::String(encoded.metadata.unwrap().to_string()));
        let rows = vec![
            (
                Some(5000),
                vec![word("late-remote", 0, 1, synthetic(Some(0))), encoded],
                vec![],
            ),
            (
                Some(1000),
                vec![
                    word("early-timed-remote", 0, 1, None),
                    word("early-timed-mic", 1000, 0, None),
                ],
                vec![],
            ),
            (
                Some(3000),
                vec![
                    word("mixed-remote", 0, 1, synthetic(None)),
                    word("mixed-timed", 1000, 0, None),
                ],
                vec![],
            ),
        ];
        let segments = render_stored_transcript_segments(rows, request()).unwrap();
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
            (Some(5000), vec![word("late-remote", 0, 1, None)], vec![]),
            (Some(1000), vec![word("early-mic", 0, 0, None)], vec![]),
            (Some(2000), vec![word("middle-remote", 0, 1, None)], vec![]),
            (Some(3000), vec![word("middle-mic", 0, 0, None)], vec![]),
        ];
        let segments = render_stored_transcript_segments(rows, request()).unwrap();
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
                word("remote-a", 0, 1, synthetic(Some(0))),
                word("mic-a", 0, 0, synthetic(Some(0))),
                word("remote-b", 1000, 1, synthetic(Some(0))),
                word("mic-b", 1000, 0, synthetic(Some(0))),
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
        let segments = render_stored_transcript_segments(rows, context).unwrap();
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
}
