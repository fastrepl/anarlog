use std::collections::HashMap;

use crate::{
    FinalizedWord, IdentityAssignment, SegmentKey, SegmentWord, SpeakerLabelContext,
    SpeakerLabeler, WordState, build_segments, channel_assignments_for_participants,
    render_speaker_label, segment_options_for_participants,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct RenderTranscriptWordInput {
    pub id: String,
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub channel: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_index: Option<i32>,
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
    pub assignments: Vec<IdentityAssignment>,
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

    let base_started_at = earliest_started_at(&transcripts);
    let segment_options =
        segment_options_for_participants(&participant_human_ids, self_human_id.as_deref());

    let mut all_segments = Vec::new();

    for transcript in transcripts {
        let offset = transcript
            .started_at
            .map(|started_at| started_at - base_started_at)
            .unwrap_or(0);

        let (words, mut assignments) =
            offset_transcript_data(transcript.words, transcript.assignments, offset);
        let channel_assignments =
            channel_assignments_for_participants(&participant_human_ids, self_human_id.as_deref());
        assignments.extend(channel_assignments);

        let segments = build_segments(&words, &[], &assignments, Some(&segment_options));
        all_segments.extend(segments);
    }

    all_segments.sort_by_key(|seg| seg.words.first().map(|w| w.start_ms).unwrap_or(i64::MAX));

    let ctx = SpeakerLabelContext {
        self_human_id,
        human_name_by_id: humans
            .into_iter()
            .map(|human| (human.human_id, human.name))
            .collect::<HashMap<_, _>>(),
    };
    let mut labeler = SpeakerLabeler::from_segments(&all_segments, Some(&ctx));

    all_segments
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

fn offset_transcript_data(
    raw_words: Vec<RenderTranscriptWordInput>,
    assignments: Vec<IdentityAssignment>,
    time_offset: i64,
) -> (Vec<FinalizedWord>, Vec<IdentityAssignment>) {
    let words: Vec<FinalizedWord> = raw_words
        .into_iter()
        .map(|w| FinalizedWord {
            id: w.id,
            text: w.text,
            start_ms: w.start_ms + time_offset,
            end_ms: w.end_ms + time_offset,
            channel: w.channel,
            state: WordState::Final,
            speaker_index: w.speaker_index,
        })
        .collect();

    (words, assignments)
}

fn earliest_started_at(transcripts: &[RenderTranscriptInput]) -> i64 {
    transcripts
        .iter()
        .filter_map(|transcript| transcript.started_at)
        .min()
        .unwrap_or(0)
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
    use crate::{ChannelProfile, IdentityScope};

    fn word(
        id: &str,
        text: &str,
        start_ms: i64,
        end_ms: i64,
        channel: i32,
    ) -> RenderTranscriptWordInput {
        RenderTranscriptWordInput {
            id: id.to_string(),
            text: text.to_string(),
            start_ms,
            end_ms,
            channel,
            speaker_index: None,
        }
    }

    fn word_si(
        id: &str,
        text: &str,
        start_ms: i64,
        end_ms: i64,
        channel: i32,
        speaker_index: i32,
    ) -> RenderTranscriptWordInput {
        RenderTranscriptWordInput {
            id: id.to_string(),
            text: text.to_string(),
            start_ms,
            end_ms,
            channel,
            speaker_index: Some(speaker_index),
        }
    }

    fn channel_assignment(human_id: &str, channel: ChannelProfile) -> IdentityAssignment {
        IdentityAssignment {
            human_id: human_id.to_string(),
            scope: IdentityScope::Channel { channel },
        }
    }

    fn speaker_assignment(
        human_id: &str,
        channel: ChannelProfile,
        speaker_index: i32,
    ) -> IdentityAssignment {
        IdentityAssignment {
            human_id: human_id.to_string(),
            scope: IdentityScope::ChannelSpeaker {
                channel,
                speaker_index,
            },
        }
    }

    #[test]
    fn renders_segments_with_labels() {
        let segments = render_transcript_segments(RenderTranscriptRequest {
            transcripts: vec![RenderTranscriptInput {
                started_at: Some(0),
                words: vec![
                    word("w1", " hello", 0, 100, 0),
                    word("w2", " world", 120, 240, 1),
                ],
                assignments: vec![],
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
                    word("w1", " remote", 0, 100, 1),
                    word("w2", " reply", 120, 220, 1),
                ],
                assignments: vec![channel_assignment("remote", ChannelProfile::RemoteParty)],
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
    fn keeps_same_provider_speaker_index_isolated_per_channel() {
        let segments = render_transcript_segments(RenderTranscriptRequest {
            transcripts: vec![RenderTranscriptInput {
                started_at: Some(0),
                words: vec![
                    word_si("w1", " john", 0, 100, 0, 0),
                    word_si("w1b", " says", 120, 220, 0, 0),
                    word_si("w1c", " hi", 240, 340, 0, 0),
                    word_si("w2", " janet", 500, 600, 1, 0),
                    word_si("w2b", " replies", 620, 720, 1, 0),
                    word_si("w2c", " back", 740, 840, 1, 0),
                    word_si("w3", " again", 1_000, 1_100, 0, 0),
                ],
                assignments: vec![
                    speaker_assignment("john", ChannelProfile::DirectMic, 0),
                    speaker_assignment("janet", ChannelProfile::RemoteParty, 0),
                ],
            }],
            participant_human_ids: vec![],
            self_human_id: None,
            humans: vec![
                RenderTranscriptHuman {
                    human_id: "john".to_string(),
                    name: "John".to_string(),
                },
                RenderTranscriptHuman {
                    human_id: "janet".to_string(),
                    name: "Janet".to_string(),
                },
            ],
        });

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].speaker_label, "John");
        assert_eq!(segments[1].speaker_label, "Janet");
        assert_eq!(segments[2].speaker_label, "John");
    }

    #[test]
    fn normalizes_multi_row_offsets_from_earliest_transcript() {
        let segments = render_transcript_segments(RenderTranscriptRequest {
            transcripts: vec![
                RenderTranscriptInput {
                    started_at: Some(5_000),
                    words: vec![word("late", " later", 100, 200, 1)],
                    assignments: vec![],
                },
                RenderTranscriptInput {
                    started_at: Some(1_000),
                    words: vec![word("early", " hello", 0, 100, 0)],
                    assignments: vec![],
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
                    word("w1", " hello", 0, 100, 0),
                    word("w2", " remote", 120, 220, 1),
                    word("w3", " more", 240, 340, 1),
                ],
                assignments: vec![channel_assignment("remote", ChannelProfile::RemoteParty)],
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
                    words: vec![word("missing-start", " hello", 0, 100, 0)],
                    assignments: vec![],
                },
                RenderTranscriptInput {
                    started_at: Some(1_000),
                    words: vec![word("known-start", " later", 100, 200, 1)],
                    assignments: vec![],
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
