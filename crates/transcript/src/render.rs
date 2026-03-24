use std::collections::{HashMap, HashSet};

use crate::{
    ChannelProfile, FinalizedWord, RuntimeSpeakerHint, SegmentBuilderOptions, SegmentKey,
    SegmentWord, SpeakerHintData, SpeakerLabelContext, SpeakerLabeler, WordRef, build_segments,
    render_speaker_label,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct RenderTranscriptWordInput {
    pub id: String,
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub channel: i32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct RenderTranscriptSpeakerHint {
    pub word_id: String,
    pub data: SpeakerHintData,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct RenderTranscriptHuman {
    pub human_id: String,
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct RenderTranscriptInput {
    pub started_at: Option<i64>,
    pub words: Vec<RenderTranscriptWordInput>,
    pub speaker_hints: Vec<RenderTranscriptSpeakerHint>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct RenderTranscriptRequest {
    pub transcripts: Vec<RenderTranscriptInput>,
    pub participant_human_ids: Vec<String>,
    pub self_human_id: Option<String>,
    pub humans: Vec<RenderTranscriptHuman>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct RenderedTranscriptSegment {
    pub id: String,
    pub key: SegmentKey,
    pub speaker_label: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub words: Vec<SegmentWord>,
}

pub fn render_transcript_segments(
    request: RenderTranscriptRequest,
) -> Vec<RenderedTranscriptSegment> {
    let RenderTranscriptRequest {
        transcripts,
        participant_human_ids,
        self_human_id,
        humans,
    } = request;

    let (words, mut speaker_hints) = collect_render_words_and_hints(transcripts);
    inject_channel_speaker_hints(
        &words,
        &participant_human_ids,
        self_human_id.as_deref(),
        &mut speaker_hints,
    );
    let segment_options = render_segment_options(&participant_human_ids, self_human_id.as_deref());
    let segments = build_segments(&words, &[], &speaker_hints, Some(&segment_options));
    let ctx = SpeakerLabelContext {
        self_human_id,
        human_name_by_id: humans
            .into_iter()
            .map(|human| (human.human_id, human.name))
            .collect::<HashMap<_, _>>(),
    };
    let mut labeler = SpeakerLabeler::from_segments(&segments, Some(&ctx));

    segments
        .into_iter()
        .filter_map(|segment| {
            let words = normalize_rendered_segment_words(segment.words);
            let first = words.first()?;
            let last = words.last()?;
            let text = words
                .iter()
                .map(|word| word.text.as_str())
                .collect::<String>()
                .trim()
                .to_string();
            if text.is_empty() {
                return None;
            }

            Some(RenderedTranscriptSegment {
                id: stable_segment_id(&segment.key, &words),
                speaker_label: render_speaker_label(&segment.key, Some(&ctx), Some(&mut labeler)),
                start_ms: first.start_ms,
                end_ms: last.end_ms,
                text,
                words,
                key: segment.key,
            })
        })
        .collect()
}

fn collect_render_words_and_hints(
    transcripts: Vec<RenderTranscriptInput>,
) -> (Vec<FinalizedWord>, Vec<crate::RuntimeSpeakerHint>) {
    let base_started_at = earliest_started_at(&transcripts);

    let mut finalized_words = Vec::new();
    let mut runtime_hints = Vec::new();

    for transcript in transcripts {
        let offset = transcript
            .started_at
            .map(|started_at| started_at - base_started_at)
            .unwrap_or(0);

        finalized_words.extend(transcript.words.into_iter().map(|word| {
            let finalized = render_word_to_finalized(word);
            FinalizedWord {
                start_ms: finalized.start_ms + offset,
                end_ms: finalized.end_ms + offset,
                ..finalized
            }
        }));
        runtime_hints.extend(
            transcript
                .speaker_hints
                .into_iter()
                .map(render_hint_to_runtime),
        );
    }

    finalized_words.sort_by_key(|word| word.start_ms);

    (finalized_words, runtime_hints)
}

fn render_word_to_finalized(word: RenderTranscriptWordInput) -> FinalizedWord {
    FinalizedWord {
        id: word.id,
        text: word.text,
        start_ms: word.start_ms,
        end_ms: word.end_ms,
        channel: word.channel,
        state: crate::WordState::Final,
    }
}

fn render_hint_to_runtime(hint: RenderTranscriptSpeakerHint) -> crate::RuntimeSpeakerHint {
    crate::RuntimeSpeakerHint {
        target: crate::WordRef::FinalWordId(hint.word_id),
        data: hint.data,
    }
}

fn earliest_started_at(transcripts: &[RenderTranscriptInput]) -> i64 {
    transcripts
        .iter()
        .filter_map(|transcript| transcript.started_at)
        .min()
        .unwrap_or(0)
}

fn render_segment_options(
    participant_human_ids: &[String],
    self_human_id: Option<&str>,
) -> SegmentBuilderOptions {
    let mut unique_participants: HashSet<&str> = participant_human_ids
        .iter()
        .map(|s| s.as_str())
        .filter(|human_id| !human_id.is_empty())
        .collect();

    if let Some(self_id) = self_human_id {
        if !self_id.is_empty() {
            unique_participants.insert(self_id);
        }
    }

    let mut complete_channels = vec![ChannelProfile::DirectMic];
    if unique_participants.len() == 2 {
        complete_channels.push(ChannelProfile::RemoteParty);
    }

    SegmentBuilderOptions {
        complete_channels: Some(complete_channels),
        ..Default::default()
    }
}

pub fn normalize_rendered_segment_words(words: Vec<SegmentWord>) -> Vec<SegmentWord> {
    words
        .into_iter()
        .enumerate()
        .map(|(index, mut word)| {
            word.text = normalized_rendered_word_text(&word.text, index == 0);
            word
        })
        .collect()
}

pub fn stable_segment_id(key: &SegmentKey, words: &[SegmentWord]) -> String {
    let first_anchor = words
        .first()
        .map(|word| {
            word.id
                .clone()
                .unwrap_or_else(|| format!("start:{}", word.start_ms))
        })
        .unwrap_or_else(|| "none".to_string());
    let last_anchor = words
        .last()
        .map(|word| {
            word.id
                .clone()
                .unwrap_or_else(|| format!("end:{}", word.end_ms))
        })
        .unwrap_or_else(|| "none".to_string());

    format!(
        "{}:{}:{}:{}:{}",
        key.channel as i32,
        key.speaker_index
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
        key.speaker_human_id.as_deref().unwrap_or("none"),
        first_anchor,
        last_anchor
    )
}

fn inject_channel_speaker_hints(
    words: &[FinalizedWord],
    participant_human_ids: &[String],
    self_human_id: Option<&str>,
    hints: &mut Vec<RuntimeSpeakerHint>,
) {
    let self_id = match self_human_id {
        Some(id) if !id.is_empty() => id,
        _ => return,
    };

    let remote_id = unique_other_participant(participant_human_ids, self_id);
    let remote_id = match remote_id {
        Some(id) => id,
        None => return,
    };

    let first_on_direct_mic = words
        .iter()
        .find(|w| w.channel == ChannelProfile::DirectMic as i32);
    let first_on_remote = words
        .iter()
        .find(|w| w.channel == ChannelProfile::RemoteParty as i32);

    if let Some(word) = first_on_direct_mic {
        hints.push(RuntimeSpeakerHint {
            target: WordRef::FinalWordId(word.id.clone()),
            data: SpeakerHintData::UserSpeakerAssignment {
                human_id: self_id.to_string(),
            },
        });
    }

    if let Some(word) = first_on_remote {
        hints.push(RuntimeSpeakerHint {
            target: WordRef::FinalWordId(word.id.clone()),
            data: SpeakerHintData::UserSpeakerAssignment {
                human_id: remote_id.to_string(),
            },
        });
    }
}

fn unique_other_participant<'a>(
    participant_human_ids: &'a [String],
    self_human_id: &str,
) -> Option<&'a str> {
    let others: Vec<&str> = participant_human_ids
        .iter()
        .map(|s| s.as_str())
        .filter(|&id| !id.is_empty() && id != self_human_id)
        .collect();

    if others.len() == 1 {
        Some(others[0])
    } else {
        None
    }
}

fn normalized_rendered_word_text(text: &str, is_first_word: bool) -> String {
    let trimmed_start = text.trim_start();
    if trimmed_start.is_empty() {
        return text.to_string();
    }

    if is_first_word {
        return trimmed_start.to_string();
    }

    if text.starts_with(' ') {
        return text.to_string();
    }

    if trimmed_start.starts_with(|c: char| ",.;:!?)}]'".contains(c)) {
        return trimmed_start.to_string();
    }

    format!(" {trimmed_start}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_segments_with_labels() {
        let segments = render_transcript_segments(RenderTranscriptRequest {
            transcripts: vec![RenderTranscriptInput {
                started_at: Some(0),
                words: vec![
                    RenderTranscriptWordInput {
                        id: "w1".to_string(),
                        text: " hello".to_string(),
                        start_ms: 0,
                        end_ms: 100,
                        channel: 0,
                    },
                    RenderTranscriptWordInput {
                        id: "w2".to_string(),
                        text: " world".to_string(),
                        start_ms: 120,
                        end_ms: 240,
                        channel: 1,
                    },
                ],
                speaker_hints: vec![],
            }],
            participant_human_ids: vec!["human-1".to_string(), "human-2".to_string()],
            self_human_id: Some("human-1".to_string()),
            humans: vec![
                RenderTranscriptHuman {
                    human_id: "human-1".to_string(),
                    name: "Alice".to_string(),
                },
                RenderTranscriptHuman {
                    human_id: "human-2".to_string(),
                    name: "Bob".to_string(),
                },
            ],
        });

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].speaker_label, "Alice");
        assert_eq!(segments[0].text, "hello");
        assert_eq!(segments[1].speaker_label, "Bob");
        assert_eq!(segments[1].text, "world");
    }

    #[test]
    fn normalizes_word_spacing_for_rendered_segments() {
        let words = normalize_rendered_segment_words(vec![
            SegmentWord {
                text: "What".to_string(),
                start_ms: 0,
                end_ms: 100,
                channel: crate::ChannelProfile::DirectMic,
                is_final: true,
                id: Some("w1".to_string()),
            },
            SegmentWord {
                text: "do".to_string(),
                start_ms: 100,
                end_ms: 200,
                channel: crate::ChannelProfile::DirectMic,
                is_final: true,
                id: Some("w2".to_string()),
            },
            SegmentWord {
                text: "'s".to_string(),
                start_ms: 200,
                end_ms: 250,
                channel: crate::ChannelProfile::DirectMic,
                is_final: true,
                id: Some("w3".to_string()),
            },
        ]);

        assert_eq!(words[0].text, "What");
        assert_eq!(words[1].text, " do");
        assert_eq!(words[2].text, "'s");
    }

    #[test]
    fn propagates_remote_labels_when_complete_channel_is_requested() {
        let segments = render_transcript_segments(RenderTranscriptRequest {
            transcripts: vec![RenderTranscriptInput {
                started_at: Some(0),
                words: vec![
                    RenderTranscriptWordInput {
                        id: "w1".to_string(),
                        text: " remote".to_string(),
                        start_ms: 0,
                        end_ms: 100,
                        channel: 1,
                    },
                    RenderTranscriptWordInput {
                        id: "w2".to_string(),
                        text: " reply".to_string(),
                        start_ms: 120,
                        end_ms: 220,
                        channel: 1,
                    },
                ],
                speaker_hints: vec![RenderTranscriptSpeakerHint {
                    word_id: "w1".to_string(),
                    data: SpeakerHintData::UserSpeakerAssignment {
                        human_id: "remote".to_string(),
                    },
                }],
            }],
            participant_human_ids: vec!["self".to_string(), "remote".to_string()],
            self_human_id: None,
            humans: vec![RenderTranscriptHuman {
                human_id: "remote".to_string(),
                name: "Remote".to_string(),
            }],
        });

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].speaker_label, "Remote");
        assert_eq!(segments[0].text, "remote reply");
    }

    #[test]
    fn normalizes_multi_row_offsets_from_earliest_transcript() {
        let segments = render_transcript_segments(RenderTranscriptRequest {
            transcripts: vec![
                RenderTranscriptInput {
                    started_at: Some(5_000),
                    words: vec![RenderTranscriptWordInput {
                        id: "late".to_string(),
                        text: " later".to_string(),
                        start_ms: 100,
                        end_ms: 200,
                        channel: 1,
                    }],
                    speaker_hints: vec![],
                },
                RenderTranscriptInput {
                    started_at: Some(1_000),
                    words: vec![RenderTranscriptWordInput {
                        id: "early".to_string(),
                        text: " hello".to_string(),
                        start_ms: 0,
                        end_ms: 100,
                        channel: 0,
                    }],
                    speaker_hints: vec![],
                },
            ],
            participant_human_ids: vec!["self".to_string(), "remote".to_string()],
            self_human_id: Some("self".to_string()),
            humans: vec![RenderTranscriptHuman {
                human_id: "self".to_string(),
                name: "Me".to_string(),
            }],
        });

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "hello");
        assert_eq!(segments[0].start_ms, 0);
        assert_eq!(segments[1].text, "later");
        assert_eq!(segments[1].start_ms, 4_100);
    }

    #[test]
    fn propagates_remote_labels_when_self_not_in_participant_list() {
        let segments = render_transcript_segments(RenderTranscriptRequest {
            transcripts: vec![RenderTranscriptInput {
                started_at: Some(0),
                words: vec![
                    RenderTranscriptWordInput {
                        id: "w1".to_string(),
                        text: " hello".to_string(),
                        start_ms: 0,
                        end_ms: 100,
                        channel: 0,
                    },
                    RenderTranscriptWordInput {
                        id: "w2".to_string(),
                        text: " remote".to_string(),
                        start_ms: 120,
                        end_ms: 220,
                        channel: 1,
                    },
                    RenderTranscriptWordInput {
                        id: "w3".to_string(),
                        text: " more".to_string(),
                        start_ms: 240,
                        end_ms: 340,
                        channel: 1,
                    },
                ],
                speaker_hints: vec![RenderTranscriptSpeakerHint {
                    word_id: "w2".to_string(),
                    data: SpeakerHintData::UserSpeakerAssignment {
                        human_id: "remote".to_string(),
                    },
                }],
            }],
            participant_human_ids: vec!["remote".to_string()],
            self_human_id: Some("self".to_string()),
            humans: vec![
                RenderTranscriptHuman {
                    human_id: "self".to_string(),
                    name: "Me".to_string(),
                },
                RenderTranscriptHuman {
                    human_id: "remote".to_string(),
                    name: "Remote".to_string(),
                },
            ],
        });

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].speaker_label, "Me");
        assert_eq!(segments[1].speaker_label, "Remote");
        assert_eq!(segments[1].text, "remote more");
    }

    #[test]
    fn keeps_missing_started_at_rows_anchored_at_zero() {
        let segments = render_transcript_segments(RenderTranscriptRequest {
            transcripts: vec![
                RenderTranscriptInput {
                    started_at: None,
                    words: vec![RenderTranscriptWordInput {
                        id: "missing-start".to_string(),
                        text: " hello".to_string(),
                        start_ms: 0,
                        end_ms: 100,
                        channel: 0,
                    }],
                    speaker_hints: vec![],
                },
                RenderTranscriptInput {
                    started_at: Some(1_000),
                    words: vec![RenderTranscriptWordInput {
                        id: "known-start".to_string(),
                        text: " later".to_string(),
                        start_ms: 100,
                        end_ms: 200,
                        channel: 1,
                    }],
                    speaker_hints: vec![],
                },
            ],
            participant_human_ids: vec!["self".to_string(), "remote".to_string()],
            self_human_id: Some("self".to_string()),
            humans: vec![RenderTranscriptHuman {
                human_id: "self".to_string(),
                name: "Me".to_string(),
            }],
        });

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "hello");
        assert_eq!(segments[0].start_ms, 0);
        assert_eq!(segments[1].text, "later");
        assert_eq!(segments[1].start_ms, 100);
    }
}
