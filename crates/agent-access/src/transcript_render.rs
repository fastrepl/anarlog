use std::collections::HashMap;

use anlg_transcript::{
    ChannelProfile, IdentityAssignment, IdentityScope, RenderTranscriptHuman,
    RenderTranscriptInput, RenderTranscriptRequest, RenderTranscriptWordInput,
    render_transcript_segments,
};
use serde_json::Value;

use crate::{MeetingExport, Transcript};

/// Renders transcripts as `Speaker: text` paragraphs, mirroring the desktop
/// transcript view. Falls back to the flat per-transcript text when word-level
/// data is unavailable (for example in trimmed cloud snapshots).
pub fn render_transcripts_markdown(export: &MeetingExport) -> String {
    let Some(request) = build_render_request(export) else {
        return render_flat_transcripts(&export.transcripts);
    };

    let segments = render_transcript_segments(request);
    if segments.is_empty() {
        return render_flat_transcripts(&export.transcripts);
    }

    segments
        .iter()
        .map(|segment| format!("{}: {}", segment.speaker_label, segment.text))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn render_flat_transcripts(transcripts: &[Transcript]) -> String {
    transcripts
        .iter()
        .filter(|transcript| !transcript.text.trim().is_empty())
        .map(|transcript| transcript.text.trim())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn assigned_human_ids(transcripts: &[Transcript]) -> Vec<String> {
    let mut ids = Vec::new();
    for transcript in transcripts {
        for hint in &transcript.speaker_hints {
            if !matches!(
                hint.get("type").and_then(Value::as_str),
                Some("automatic_speaker_assignment" | "user_speaker_assignment")
            ) {
                continue;
            }
            let Some(value) = parse_hint_value(hint.get("value")) else {
                continue;
            };
            if let Some(human_id) = value.get("human_id").and_then(Value::as_str)
                && !human_id.is_empty()
                && !ids.iter().any(|id| id == human_id)
            {
                ids.push(human_id.to_string());
            }
        }
    }
    ids
}

fn build_render_request(export: &MeetingExport) -> Option<RenderTranscriptRequest> {
    let transcripts = export
        .transcripts
        .iter()
        .filter_map(normalize_transcript)
        .collect::<Vec<_>>();
    if transcripts.is_empty() {
        return None;
    }

    let participant_human_ids = export
        .meeting
        .participants
        .iter()
        .map(|participant| participant.human_id.clone())
        .filter(|id| !id.is_empty())
        .collect::<Vec<_>>();

    let mut humans: HashMap<String, String> = export
        .meeting
        .participants
        .iter()
        .filter(|participant| {
            !participant.human_id.is_empty() && !participant.display_name.trim().is_empty()
        })
        .map(|participant| {
            (
                participant.human_id.clone(),
                participant.display_name.trim().to_string(),
            )
        })
        .collect();
    let mut self_human_id = None;
    for speaker in &export.speakers {
        if speaker.is_self {
            self_human_id = Some(speaker.human_id.clone());
        }
        if !speaker.name.trim().is_empty() {
            humans
                .entry(speaker.human_id.clone())
                .or_insert_with(|| speaker.name.trim().to_string());
        }
    }
    let mut humans = humans
        .into_iter()
        .map(|(human_id, name)| RenderTranscriptHuman { human_id, name })
        .collect::<Vec<_>>();
    humans.sort_by(|a, b| a.human_id.cmp(&b.human_id));

    Some(RenderTranscriptRequest {
        speaker_context: None,
        preview: None,
        transcripts,
        participant_human_ids,
        self_human_id,
        humans,
    })
}

fn normalize_transcript(transcript: &Transcript) -> Option<RenderTranscriptInput> {
    let mut words = Vec::new();
    let mut word_index_by_id = HashMap::new();

    for word in &transcript.words {
        let (Some(id), Some(text), Some(start_ms), Some(end_ms)) = (
            word.get("id").and_then(Value::as_str),
            word.get("text").and_then(Value::as_str),
            word.get("start_ms").and_then(Value::as_i64),
            word.get("end_ms").and_then(Value::as_i64),
        ) else {
            continue;
        };
        word_index_by_id.insert(id.to_string(), words.len());
        words.push(RenderTranscriptWordInput {
            id: id.to_string(),
            text: text.to_string(),
            start_ms,
            end_ms,
            channel: word
                .get("channel")
                .and_then(Value::as_i64)
                .map(|channel| channel as i32)
                .unwrap_or(0),
            speaker_index: None,
        });
    }

    if words.is_empty() {
        return None;
    }

    for hint in hints_of(&transcript.speaker_hints, "provider_speaker_index") {
        normalize_speaker_hint(hint, &mut words, &word_index_by_id);
    }
    let mut assignments = Vec::new();
    for kind in ["automatic_speaker_assignment", "user_speaker_assignment"] {
        for hint in hints_of(&transcript.speaker_hints, kind) {
            if let Some(assignment) = normalize_speaker_hint(hint, &mut words, &word_index_by_id) {
                assignments.push(assignment);
            }
        }
    }

    Some(RenderTranscriptInput {
        started_at: Some(transcript.started_at_ms),
        words,
        assignments,
    })
}

fn hints_of<'a>(hints: &'a [Value], kind: &'a str) -> impl Iterator<Item = &'a Value> + 'a {
    hints
        .iter()
        .filter(move |hint| hint.get("type").and_then(Value::as_str) == Some(kind))
}

fn normalize_speaker_hint(
    hint: &Value,
    words: &mut [RenderTranscriptWordInput],
    word_index_by_id: &HashMap<String, usize>,
) -> Option<IdentityAssignment> {
    let word_id = hint.get("word_id").and_then(Value::as_str)?;
    let kind = hint.get("type").and_then(Value::as_str)?;
    let value = parse_hint_value(hint.get("value"))?;

    let is_speaker_assignment = matches!(
        kind,
        "automatic_speaker_assignment" | "user_speaker_assignment"
    );
    let human_id = value
        .get("human_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty());

    if is_speaker_assignment && let Some(human_id) = human_id {
        if let Some(scope) = explicit_speaker_scope(&value) {
            return Some(IdentityAssignment {
                human_id: human_id.to_string(),
                scope,
            });
        }
        if value.get("scope").and_then(Value::as_str) == Some("segment")
            && let Some(word_ids) = value.get("word_ids").and_then(Value::as_array)
        {
            let word_ids = word_ids
                .iter()
                .filter_map(Value::as_str)
                .filter(|id| !id.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>();
            if !word_ids.is_empty() {
                return Some(IdentityAssignment {
                    human_id: human_id.to_string(),
                    scope: IdentityScope::Words { word_ids },
                });
            }
        }
    }

    let word = words.get_mut(*word_index_by_id.get(word_id)?)?;

    if kind == "provider_speaker_index" {
        if let Some(speaker_index) = value.get("speaker_index").and_then(Value::as_i64) {
            word.speaker_index = Some(speaker_index as i32);
            if let Some(channel) = value.get("channel").and_then(Value::as_i64) {
                word.channel = channel as i32;
            }
        }
        return None;
    }

    if is_speaker_assignment && let Some(human_id) = human_id {
        let channel = ChannelProfile::from(word.channel);
        return Some(IdentityAssignment {
            human_id: human_id.to_string(),
            scope: match word.speaker_index {
                None => IdentityScope::Channel { channel },
                Some(speaker_index) => IdentityScope::ChannelSpeaker {
                    channel,
                    speaker_index,
                },
            },
        });
    }

    None
}

fn explicit_speaker_scope(value: &Value) -> Option<IdentityScope> {
    if value.get("scope").and_then(Value::as_str) != Some("speaker") {
        return None;
    }
    let channel = match value.get("channel").and_then(Value::as_i64)? {
        channel @ 0..=2 => ChannelProfile::from(channel as i32),
        _ => return None,
    };
    match value.get("speaker_index") {
        Some(Value::Number(index)) => Some(IdentityScope::ChannelSpeaker {
            channel,
            speaker_index: index.as_i64()? as i32,
        }),
        Some(Value::Null) => Some(IdentityScope::Channel { channel }),
        _ => None,
    }
}

fn parse_hint_value(value: Option<&Value>) -> Option<Value> {
    match value? {
        Value::String(raw) => serde_json::from_str::<Value>(raw).ok(),
        other => Some(other.clone()),
    }
    .filter(Value::is_object)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Meeting, Participant, Speaker};
    use serde_json::json;

    fn word(id: &str, text: &str, start_ms: i64, channel: i32) -> Value {
        json!({
            "id": id,
            "text": text,
            "start_ms": start_ms,
            "end_ms": start_ms + 500,
            "channel": channel,
        })
    }

    fn transcript(words: Vec<Value>, speaker_hints: Vec<Value>) -> Transcript {
        Transcript {
            id: "transcript-1".to_string(),
            source: String::new(),
            provider: String::new(),
            model: String::new(),
            language: "en".to_string(),
            started_at_ms: 0,
            ended_at_ms: None,
            memo: String::new(),
            text: words
                .iter()
                .filter_map(|word| word.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(" "),
            words,
            speaker_hints,
        }
    }

    fn export(transcripts: Vec<Transcript>, speakers: Vec<Speaker>) -> MeetingExport {
        MeetingExport {
            meeting: Meeting {
                id: "meeting-1".to_string(),
                title: "Planning".to_string(),
                kind: String::new(),
                status: String::new(),
                created_at: String::new(),
                updated_at: String::new(),
                started_at: String::new(),
                ended_at: String::new(),
                timezone: String::new(),
                language: String::new(),
                series_id: String::new(),
                note: None,
                summaries: Vec::new(),
                participants: [("human-1", "Alice"), ("human-2", "Bob")]
                    .into_iter()
                    .map(|(human_id, display_name)| Participant {
                        human_id: human_id.to_string(),
                        display_name: display_name.to_string(),
                        email: String::new(),
                        role: String::new(),
                        job_title: String::new(),
                        organization_id: String::new(),
                        organization_name: String::new(),
                    })
                    .collect(),
                action_items: Vec::new(),
            },
            transcripts,
            speakers,
        }
    }

    #[test]
    fn falls_back_to_flat_text_without_word_metadata() {
        let export = export(
            vec![transcript(
                vec![json!({"text": "one"}), json!({"text": "two"})],
                Vec::new(),
            )],
            Vec::new(),
        );
        assert_eq!(render_transcripts_markdown(&export), "one two");
    }

    #[test]
    fn labels_speakers_from_assignments_and_provider_indexes() {
        let words = vec![
            word("w1", "Hello", 0, 0),
            word("w2", "there", 500, 0),
            word("w3", "Hi", 1000, 1),
            word("w4", "Alice", 1500, 1),
            word("w5", "Anyone", 2000, 1),
            word("w6", "else?", 2500, 1),
        ];
        let hints = vec![
            json!({
                "type": "provider_speaker_index",
                "word_id": "w5",
                "value": json!({"speaker_index": 2}).to_string(),
            }),
            json!({
                "type": "provider_speaker_index",
                "word_id": "w6",
                "value": {"speaker_index": 2},
            }),
            json!({
                "type": "user_speaker_assignment",
                "word_id": "w3",
                "value": {"human_id": "human-1", "scope": "segment", "word_ids": ["w3", "w4"]},
            }),
        ];
        let export = export(
            vec![transcript(words, hints)],
            vec![Speaker {
                human_id: "self-1".to_string(),
                name: "Me".to_string(),
                is_self: true,
            }],
        );

        assert_eq!(
            render_transcripts_markdown(&export),
            "Me: Hello there\n\nAlice: Hi Alice\n\nSpeaker 1: Anyone else?"
        );
    }
}
