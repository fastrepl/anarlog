use std::collections::HashMap;
use std::path::PathBuf;

use hypr_db_parser::{SpeakerHint, Transcript, Word};
use serde_json::Value;

use crate::Result;

pub enum FileOp {
    Write {
        path: PathBuf,
        content: String,
        force: bool,
    },
}

pub fn group_by_session_id<T, F>(items: &[T], get_id: F) -> HashMap<&str, Vec<&T>>
where
    F: Fn(&T) -> &str,
{
    let mut map: HashMap<&str, Vec<&T>> = HashMap::new();
    for item in items {
        map.entry(get_id(item)).or_default().push(item);
    }
    map
}

pub fn word_to_json(w: &Word) -> Value {
    serde_json::json!({
        "id": w.id,
        "text": w.text,
        "start_ms": w.start_ms.unwrap_or(0.0) as i64,
        "end_ms": w.end_ms.unwrap_or(0.0) as i64,
        "channel": w.channel,
        "speaker": w.speaker,
    })
}

pub fn hint_to_json(h: &SpeakerHint) -> Value {
    let value: Value = serde_json::from_str(&h.value).unwrap_or(Value::String(h.value.clone()));
    serde_json::json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "word_id": h.word_id,
        "type": h.hint_type,
        "value": value,
    })
}

pub fn hints_to_json(hints: &[SpeakerHint]) -> Value {
    Value::Array(hints.iter().map(hint_to_json).collect())
}

pub fn transcript_to_json(transcript: &Transcript) -> Value {
    let words: Vec<Value> = transcript.words.iter().map(word_to_json).collect();
    let speaker_hints: Vec<Value> = transcript.speaker_hints.iter().map(hint_to_json).collect();

    serde_json::json!({
        "id": transcript.id,
        "user_id": transcript.user_id,
        "created_at": transcript.created_at,
        "session_id": transcript.session_id,
        "started_at": transcript.started_at as i64,
        "ended_at": transcript.ended_at.map(|v| v as i64),
        "words": words,
        "speaker_hints": speaker_hints,
    })
}

pub fn build_transcript_json_multi(transcripts: &[&Transcript]) -> String {
    let mut sorted: Vec<&Transcript> = transcripts.to_vec();
    sorted.sort_by(|a, b| {
        a.started_at
            .partial_cmp(&b.started_at)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let transcripts_json: Vec<Value> = sorted.iter().map(|t| transcript_to_json(t)).collect();

    let data = serde_json::json!({
        "transcripts": transcripts_json
    });

    serde_json::to_string_pretty(&data).unwrap()
}

pub fn apply_ops(ops: Vec<FileOp>) -> Result<()> {
    for op in ops {
        match op {
            FileOp::Write {
                path,
                content,
                force,
            } => {
                if path.exists() && !force {
                    continue;
                }
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, &content)?;
            }
        }
    }
    Ok(())
}
