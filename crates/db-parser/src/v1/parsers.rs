use std::collections::HashMap;

use serde_json::Value;

use super::cell::{get_cell_f64, get_cell_i64, get_cell_str};
use super::types::{SpeakerHintRaw, TranscriptRaw, WordWithTranscript};
use crate::types::*;

pub(super) fn parse_session(id: &str, cells: &serde_json::Map<String, Value>) -> Option<Session> {
    let title = get_cell_str(cells, "title").unwrap_or_default();
    let raw_md = get_cell_str(cells, "raw_md");

    if title.is_empty() && raw_md.is_none() {
        return None;
    }

    Some(Session {
        id: id.to_string(),
        user_id: get_cell_str(cells, "user_id")
            .unwrap_or_default()
            .to_string(),
        created_at: get_cell_str(cells, "created_at")
            .unwrap_or_default()
            .to_string(),
        title: title.to_string(),
        raw_md: raw_md.map(String::from),
        enhanced_content: None,
        folder_id: get_cell_str(cells, "folder_id").map(String::from),
        event_id: get_cell_str(cells, "event_id").map(String::from),
    })
}

pub(super) fn parse_transcript_raw(
    id: &str,
    cells: &serde_json::Map<String, Value>,
) -> Option<TranscriptRaw> {
    Some(TranscriptRaw {
        id: id.to_string(),
        user_id: get_cell_str(cells, "user_id")
            .unwrap_or_default()
            .to_string(),
        created_at: get_cell_str(cells, "created_at")
            .unwrap_or_default()
            .to_string(),
        session_id: get_cell_str(cells, "session_id")
            .unwrap_or_default()
            .to_string(),
        started_at: get_cell_f64(cells, "started_at").unwrap_or_default(),
        ended_at: get_cell_f64(cells, "ended_at"),
        inline_words: get_cell_str(cells, "words").map(String::from),
        inline_hints: get_cell_str(cells, "speaker_hints").map(String::from),
    })
}

pub(super) fn parse_word(
    id: &str,
    cells: &serde_json::Map<String, Value>,
) -> Option<WordWithTranscript> {
    let transcript_id = get_cell_str(cells, "transcript_id")?.to_string();
    Some(WordWithTranscript {
        transcript_id,
        word: Word {
            id: id.to_string(),
            text: get_cell_str(cells, "text").unwrap_or_default().to_string(),
            start_ms: get_cell_f64(cells, "start_ms"),
            end_ms: get_cell_f64(cells, "end_ms"),
            channel: get_cell_i64(cells, "channel").unwrap_or_default(),
            speaker: get_cell_str(cells, "speaker").map(String::from),
        },
    })
}

pub(super) fn parse_speaker_hint_raw(
    _id: &str,
    cells: &serde_json::Map<String, Value>,
) -> Option<SpeakerHintRaw> {
    let word_id = get_cell_str(cells, "word_id")?.to_string();
    if word_id.is_empty() {
        return None;
    }
    Some(SpeakerHintRaw {
        word_id,
        hint_type: get_cell_str(cells, "type").unwrap_or_default().to_string(),
        value: get_cell_str(cells, "value").unwrap_or_default().to_string(),
    })
}

pub(super) fn resolve_speaker_hint(hint: &SpeakerHintRaw) -> Option<String> {
    match hint.hint_type.as_str() {
        "speaker_label" | "label" => {
            let parsed = serde_json::from_str::<Value>(&hint.value).ok()?;
            parsed
                .get("label")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or_else(|| Some(hint.value.clone()))
        }
        "provider_speaker_index" => {
            let parsed = serde_json::from_str::<Value>(&hint.value).ok()?;
            let speaker_index = parsed.get("speaker_index").and_then(|v| v.as_i64())?;
            Some(format!("Speaker {}", speaker_index))
        }
        _ => None,
    }
}

pub(super) fn parse_inline_words(json: &str, _started_at: f64) -> Vec<Word> {
    let Ok(arr) = serde_json::from_str::<Vec<Value>>(json) else {
        return vec![];
    };

    let mut words: Vec<Word> = arr
        .into_iter()
        .filter_map(|v| {
            let obj = v.as_object()?;
            let id = obj.get("id")?.as_str()?.to_string();
            Some(Word {
                id,
                text: obj
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                start_ms: obj.get("start_ms").and_then(|v| v.as_f64()),
                end_ms: obj.get("end_ms").and_then(|v| v.as_f64()),
                channel: obj
                    .get("channel")
                    .and_then(|v| v.as_i64())
                    .unwrap_or_default(),
                speaker: obj
                    .get("speaker")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(String::from),
            })
        })
        .collect();

    words.sort_by(|a, b| {
        let start_a = a.start_ms.unwrap_or(0.0);
        let start_b = b.start_ms.unwrap_or(0.0);
        start_a
            .partial_cmp(&start_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    words
}

pub(super) fn parse_inline_hints_to_map(json: &str) -> HashMap<String, String> {
    let Ok(arr) = serde_json::from_str::<Vec<Value>>(json) else {
        return HashMap::new();
    };

    arr.into_iter()
        .filter_map(|v| {
            let obj = v.as_object()?;
            let word_id = obj.get("word_id").and_then(|v| v.as_str())?;
            if word_id.is_empty() {
                return None;
            }
            let hint_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or_default();
            let value = obj
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            let hint = SpeakerHintRaw {
                word_id: word_id.to_string(),
                hint_type: hint_type.to_string(),
                value: value.to_string(),
            };
            let label = resolve_speaker_hint(&hint)?;
            Some((word_id.to_string(), label))
        })
        .collect()
}

pub(super) fn parse_inline_hints_raw(json: &str) -> Vec<SpeakerHint> {
    let Ok(arr) = serde_json::from_str::<Vec<Value>>(json) else {
        return vec![];
    };

    arr.into_iter()
        .filter_map(|v| {
            let obj = v.as_object()?;
            let word_id = obj.get("word_id").and_then(|v| v.as_str())?;
            if word_id.is_empty() {
                return None;
            }
            let hint_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or_default();
            let value = obj
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            Some(SpeakerHint {
                word_id: word_id.to_string(),
                hint_type: hint_type.to_string(),
                value: value.to_string(),
            })
        })
        .collect()
}

pub(super) fn parse_human(id: &str, cells: &serde_json::Map<String, Value>) -> Option<Human> {
    if id == "00000000-0000-0000-0000-000000000000" {
        return None;
    }
    Some(Human {
        id: id.to_string(),
        user_id: get_cell_str(cells, "user_id")
            .unwrap_or_default()
            .to_string(),
        created_at: get_cell_str(cells, "created_at")
            .unwrap_or_default()
            .to_string(),
        name: get_cell_str(cells, "name").unwrap_or_default().to_string(),
        email: get_cell_str(cells, "email").map(String::from),
        org_id: get_cell_str(cells, "org_id").map(String::from),
        job_title: get_cell_str(cells, "job_title").map(String::from),
        linkedin_username: get_cell_str(cells, "linkedin_username").map(String::from),
    })
}

pub(super) fn parse_organization(
    id: &str,
    cells: &serde_json::Map<String, Value>,
) -> Option<Organization> {
    if id == "0" {
        return None;
    }
    Some(Organization {
        id: id.to_string(),
        user_id: get_cell_str(cells, "user_id")
            .unwrap_or_default()
            .to_string(),
        created_at: get_cell_str(cells, "created_at")
            .unwrap_or_default()
            .to_string(),
        name: get_cell_str(cells, "name").unwrap_or_default().to_string(),
        description: get_cell_str(cells, "description").map(String::from),
    })
}

pub(super) fn parse_participant(
    id: &str,
    cells: &serde_json::Map<String, Value>,
) -> Option<SessionParticipant> {
    let session_id = get_cell_str(cells, "session_id")?;
    let human_id = get_cell_str(cells, "human_id")?;
    if session_id.is_empty() || human_id.is_empty() {
        return None;
    }
    Some(SessionParticipant {
        id: id.to_string(),
        user_id: get_cell_str(cells, "user_id")
            .unwrap_or_default()
            .to_string(),
        session_id: session_id.to_string(),
        human_id: human_id.to_string(),
        source: get_cell_str(cells, "source")
            .unwrap_or("imported")
            .to_string(),
    })
}

pub(super) fn parse_template(id: &str, cells: &serde_json::Map<String, Value>) -> Option<Template> {
    let title = get_cell_str(cells, "title")?;
    if title.is_empty() {
        return None;
    }

    let sections = get_cell_str(cells, "sections")
        .and_then(|s| serde_json::from_str::<Vec<TemplateSection>>(s).ok())
        .unwrap_or_default();

    let tags = get_cell_str(cells, "tags")
        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
        .unwrap_or_default();

    Some(Template {
        id: id.to_string(),
        user_id: get_cell_str(cells, "user_id")
            .unwrap_or_default()
            .to_string(),
        title: title.to_string(),
        description: get_cell_str(cells, "description")
            .unwrap_or_default()
            .to_string(),
        sections,
        tags,
        context_option: get_cell_str(cells, "context_option").map(String::from),
    })
}

pub(super) fn parse_enhanced_note(
    id: &str,
    cells: &serde_json::Map<String, Value>,
) -> Option<EnhancedNote> {
    let session_id = get_cell_str(cells, "session_id")?;
    let content = get_cell_str(cells, "content")?;
    if session_id.is_empty() || content.is_empty() {
        return None;
    }

    Some(EnhancedNote {
        id: id.to_string(),
        user_id: get_cell_str(cells, "user_id")
            .unwrap_or_default()
            .to_string(),
        session_id: session_id.to_string(),
        content: content.to_string(),
        template_id: get_cell_str(cells, "template_id").map(String::from),
        position: get_cell_i64(cells, "position").unwrap_or(1) as i32,
        title: get_cell_str(cells, "title").unwrap_or_default().to_string(),
    })
}

pub(super) fn parse_tag(id: &str, cells: &serde_json::Map<String, Value>) -> Option<Tag> {
    let name = get_cell_str(cells, "name").unwrap_or_default();
    if name.is_empty() {
        return None;
    }
    Some(Tag {
        id: id.to_string(),
        user_id: get_cell_str(cells, "user_id")
            .unwrap_or_default()
            .to_string(),
        name: name.to_string(),
    })
}

pub(super) fn parse_tag_mapping(
    id: &str,
    cells: &serde_json::Map<String, Value>,
) -> Option<TagMapping> {
    let tag_id = get_cell_str(cells, "tag_id")?;
    let session_id = get_cell_str(cells, "session_id")?;
    if tag_id.is_empty() || session_id.is_empty() {
        return None;
    }
    Some(TagMapping {
        id: id.to_string(),
        user_id: get_cell_str(cells, "user_id")
            .unwrap_or_default()
            .to_string(),
        tag_id: tag_id.to_string(),
        session_id: session_id.to_string(),
    })
}
