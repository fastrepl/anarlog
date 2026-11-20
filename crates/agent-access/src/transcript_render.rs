use std::collections::HashMap;

use anlg_transcript::{
    RenderTranscriptHuman, RenderTranscriptInput, RenderTranscriptRequest, SpeakerContext,
    render_transcript_segments,
};
use serde_json::Value;

use crate::{MeetingExport, Transcript};

/// Renders transcripts as `Speaker: text` paragraphs, mirroring the desktop
/// transcript view. Transcript rows without complete word-level data (for
/// example in trimmed cloud snapshots) keep their flat text, interleaved with
/// the rendered segments by start time.
pub fn render_transcripts_markdown(export: &MeetingExport) -> String {
    let mut flat = Vec::new();
    let mut renderable = Vec::new();
    for transcript in &export.transcripts {
        match normalize_transcript(transcript) {
            Some(input) => renderable.push(input),
            None => {
                let text = transcript.text.trim();
                if !text.is_empty() {
                    flat.push((transcript.started_at_ms, text.to_string()));
                }
            }
        }
    }

    let mut paragraphs = flat;
    if !renderable.is_empty() {
        let base_started_at = renderable
            .iter()
            .filter_map(|transcript| transcript.started_at)
            .min()
            .unwrap_or(0);
        let request = build_render_request(export, renderable);
        paragraphs.extend(
            render_transcript_segments(request)
                .into_iter()
                .map(|segment| {
                    (
                        base_started_at + segment.start_ms,
                        format!("{}: {}", segment.speaker_label, segment.text),
                    )
                }),
        );
    }

    paragraphs.sort_by_key(|(started_at, _)| *started_at);
    paragraphs
        .into_iter()
        .map(|(_, text)| text)
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

fn build_render_request(
    export: &MeetingExport,
    transcripts: Vec<RenderTranscriptInput>,
) -> RenderTranscriptRequest {
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

    RenderTranscriptRequest {
        speaker_context: export
            .speaker_context
            .as_ref()
            .and_then(parse_speaker_context),
        preview: None,
        transcripts,
        participant_human_ids,
        self_human_id,
        humans,
    }
}

fn parse_speaker_context(value: &Value) -> Option<SpeakerContext> {
    let context = SpeakerContext::from_stored_value(value);
    (!context.is_empty()).then_some(context)
}

fn normalize_transcript(transcript: &Transcript) -> Option<RenderTranscriptInput> {
    let (mut words, mut hints) = anlg_transcript::normalize_stored_transcript(
        &transcript.id,
        transcript.words.clone(),
        anlg_transcript::parse_stored_speaker_hints(
            &serde_json::to_string(&transcript.speaker_hints).ok()?,
        ),
    );
    if words.len() != transcript.words.len()
        || words
            .iter()
            .any(|word| word.text.is_none() || word.start_ms.is_none() || word.end_ms.is_none())
    {
        return None;
    }
    anlg_transcript::normalize_legacy_batch_channels(
        &transcript.source,
        &transcript.provider,
        &mut words,
        &mut hints,
    );
    anlg_transcript::render_input_from_stored(Some(transcript.started_at_ms), &words, &hints)
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
                folder_path: None,
                action_items: Vec::new(),
            },
            transcripts,
            speakers,
            speaker_context: None,
        }
    }

    #[test]
    fn keeps_flat_rows_when_only_some_rows_have_word_metadata() {
        let mut rendered = transcript(vec![word("w1", "Hello", 0, 0)], Vec::new());
        rendered.started_at_ms = 1_000;
        let mut flat = transcript(vec![json!({"text": "Decision approved"})], Vec::new());
        flat.started_at_ms = 5_000;
        let mut partial = transcript(
            vec![word("w2", "Later", 0, 0), json!({"text": "words"})],
            Vec::new(),
        );
        partial.started_at_ms = 9_000;

        let export = export(vec![flat, rendered, partial], Vec::new());
        assert_eq!(
            render_transcripts_markdown(&export),
            "Speaker 1: Hello\n\nDecision approved\n\nLater words"
        );
    }

    #[test]
    fn shared_microphone_context_disables_owner_inference() {
        let words = vec![word("w1", "Hello", 0, 0), word("w2", "Bob", 500, 0)];
        let speakers = vec![Speaker {
            human_id: "self-1".to_string(),
            name: "Me".to_string(),
            is_self: true,
        }];
        let mut export = export(vec![transcript(words, Vec::new())], speakers);
        assert_eq!(render_transcripts_markdown(&export), "Speaker 1: Hello Bob");

        export.speaker_context = Some(json!({
            "intervals": [{
                "start_ms": 0,
                "end_ms": 60_000,
                "active_call": false,
                "calendar_call": false,
                "mic_isolated": null,
                "shared_microphone": true,
                "title": "",
                "self_names": [],
                "participants": []
            }]
        }));
        assert_eq!(render_transcripts_markdown(&export), "Speaker 1: Hello Bob");
        let interval = &mut export.speaker_context.as_mut().unwrap()["intervals"][0];
        interval["shared_microphone"] = json!(false);
        interval["mic_isolated"] = json!(true);
        assert_eq!(render_transcripts_markdown(&export), "Me: Hello Bob");
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
            "Speaker 1: Hello there\n\nAlice: Hi Alice\n\nSpeaker 2: Anyone else?"
        );
    }
}
