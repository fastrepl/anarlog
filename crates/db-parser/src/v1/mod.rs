use std::collections::HashMap;
use std::path::Path;

use hypr_db_core::libsql;
use serde_json::Value;

use crate::types::*;
use crate::{Error, Result};

mod cell;
mod parsers;
mod types;

use cell::is_tombstone;
use parsers::*;
use types::{SpeakerHintRaw, TranscriptRaw, WordWithTranscript};

pub async fn validate(path: &Path) -> Result<()> {
    let db = libsql::Builder::new_local(path).build().await?;
    let conn = db.connect()?;

    let mut rows = conn
        .query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            (),
        )
        .await?;

    let mut tables = Vec::new();
    while let Some(row) = rows.next().await? {
        tables.push(row.get::<String>(0)?);
    }

    if tables.len() != 1 || tables[0] != "main" {
        return Err(Error::InvalidData(format!(
            "v1 database expected single 'main' table, found: {:?}",
            tables
        )));
    }

    Ok(())
}

pub async fn parse_from_sqlite(path: &Path) -> Result<Collection> {
    validate(path).await?;

    let db = libsql::Builder::new_local(path).build().await?;
    let conn = db.connect()?;

    let mut rows = conn
        .query("SELECT store FROM main WHERE id = '_'", ())
        .await?;

    let row = rows
        .next()
        .await?
        .ok_or_else(|| Error::InvalidData("No store data found".to_string()))?;

    let store_json: String = row.get(0)?;
    let store: Value = serde_json::from_str(&store_json)?;

    parse_store(&store)
}

fn parse_store(store: &Value) -> Result<Collection> {
    let tables = store
        .get(0)
        .and_then(|v| v.get(0))
        .and_then(|v| v.as_object())
        .ok_or_else(|| Error::InvalidData("Invalid TinyBase store structure".to_string()))?;

    let sessions = extract_rows(tables, "sessions", parse_session);
    let humans = extract_rows(tables, "humans", parse_human);
    let organizations = extract_rows(tables, "organizations", parse_organization);
    let participants = extract_rows(tables, "mapping_session_participant", parse_participant);
    let templates = extract_rows(tables, "templates", parse_template);
    let enhanced_notes = extract_rows(tables, "enhanced_notes", parse_enhanced_note);
    let tags = extract_rows(tables, "tags", parse_tag);
    let tag_mappings = extract_rows(tables, "mapping_tag_session", parse_tag_mapping);

    let session_titles: HashMap<String, String> = sessions
        .iter()
        .map(|s| (s.id.clone(), s.title.clone()))
        .collect();

    let transcripts_raw = extract_rows(tables, "transcripts", parse_transcript_raw);
    let words_table = extract_rows(tables, "words", parse_word);
    let hints_table = extract_rows(tables, "speaker_hints", parse_speaker_hint_raw);

    let transcripts =
        merge_transcript_data(transcripts_raw, words_table, hints_table, &session_titles);

    Ok(Collection {
        sessions,
        transcripts,
        humans,
        organizations,
        participants,
        templates,
        enhanced_notes,
        tags,
        tag_mappings,
    })
}

fn extract_rows<T, F>(
    tables: &serde_json::Map<String, Value>,
    table_name: &str,
    parser: F,
) -> Vec<T>
where
    F: Fn(&str, &serde_json::Map<String, Value>) -> Option<T>,
{
    let Some(table) = tables.get(table_name) else {
        return vec![];
    };

    let Some(rows_obj) = table.get(0).and_then(|v| v.as_object()) else {
        return vec![];
    };

    rows_obj
        .iter()
        .filter_map(|(row_id, row_data)| {
            let cells = row_data.get(0)?.as_object()?;
            if is_tombstone(cells) {
                return None;
            }
            parser(row_id, cells)
        })
        .collect()
}

fn merge_transcript_data(
    raw: Vec<TranscriptRaw>,
    words_table: Vec<WordWithTranscript>,
    hints_table: Vec<SpeakerHintRaw>,
    session_titles: &HashMap<String, String>,
) -> Vec<Transcript> {
    let mut words_by_transcript: HashMap<String, Vec<WordWithTranscript>> = HashMap::new();

    // Build word_id -> speaker label mapping for resolving word.speaker
    let speaker_hints_for_labels: HashMap<String, String> = hints_table
        .iter()
        .filter_map(|h| {
            let label = resolve_speaker_hint(h)?;
            Some((h.word_id.clone(), label))
        })
        .collect();

    for w in &words_table {
        words_by_transcript
            .entry(w.transcript_id.clone())
            .or_default()
            .push(w.clone());
    }

    // Build word_id -> transcript_id mapping
    let word_to_transcript: HashMap<String, String> = words_table
        .iter()
        .map(|w| (w.word.id.clone(), w.transcript_id.clone()))
        .collect();

    // Group hints by transcript_id (via word_id -> transcript_id)
    let mut hints_by_transcript: HashMap<String, Vec<SpeakerHint>> = HashMap::new();
    for hint in &hints_table {
        if let Some(transcript_id) = word_to_transcript.get(&hint.word_id) {
            hints_by_transcript
                .entry(transcript_id.clone())
                .or_default()
                .push(SpeakerHint {
                    word_id: hint.word_id.clone(),
                    hint_type: hint.hint_type.clone(),
                    value: hint.value.clone(),
                });
        }
    }

    raw.into_iter()
        .filter_map(|t| {
            let transcript_id = t.id.clone();

            let (mut words, inline_hints) = if let Some(inline_words) = &t.inline_words {
                let mut words = parse_inline_words(inline_words, t.started_at);
                let inline_hints = if let Some(inline_hints_str) = &t.inline_hints {
                    let inline_speaker_hints = parse_inline_hints_to_map(inline_hints_str);
                    for word in &mut words {
                        if let Some(hint_speaker) = inline_speaker_hints.get(&word.id) {
                            word.speaker = Some(hint_speaker.clone());
                        }
                    }
                    parse_inline_hints_raw(inline_hints_str)
                } else {
                    vec![]
                };
                (words, inline_hints)
            } else {
                let mut raw_words = words_by_transcript.remove(&t.id).unwrap_or_default();
                raw_words.sort_by(|a, b| {
                    a.word
                        .start_ms
                        .partial_cmp(&b.word.start_ms)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                let words = raw_words
                    .into_iter()
                    .map(|w| {
                        let speaker = speaker_hints_for_labels
                            .get(&w.word.id)
                            .cloned()
                            .or(w.word.speaker)
                            .unwrap_or_else(|| format!("Speaker {}", w.word.channel));

                        Word {
                            id: w.word.id,
                            text: w.word.text,
                            start_ms: w.word.start_ms,
                            end_ms: w.word.end_ms,
                            channel: w.word.channel,
                            speaker: Some(speaker),
                        }
                    })
                    .collect();
                (words, vec![])
            };

            if words.is_empty() {
                return None;
            }

            for word in &mut words {
                if word.speaker.is_none() {
                    word.speaker = Some(format!("Speaker {}", word.channel));
                }
            }

            let start_ms = words.first().and_then(|w| w.start_ms);
            let end_ms = words.last().and_then(|w| w.end_ms);
            let title = session_titles
                .get(&t.session_id)
                .cloned()
                .unwrap_or_default();

            // Combine hints from table and inline hints
            let mut speaker_hints = hints_by_transcript
                .remove(&transcript_id)
                .unwrap_or_default();
            speaker_hints.extend(inline_hints);

            Some(Transcript {
                id: t.id,
                user_id: t.user_id,
                created_at: t.created_at,
                session_id: t.session_id,
                title,
                started_at: t.started_at,
                ended_at: t.ended_at,
                start_ms,
                end_ms,
                words,
                speaker_hints,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_store_empty() {
        let store: Value = serde_json::json!([[{}]]);
        let result = parse_store(&store).unwrap();
        assert!(result.sessions.is_empty());
    }

    #[test]
    fn parse_store_with_extra_fields() {
        let store: Value = serde_json::json!([[{
            "sessions": [{
                "test-id": [{
                    "title": ["Test"],
                    "unknown_field": ["ignored"],
                    "another_future_field": [123]
                }]
            }]
        }]]);
        let result = parse_store(&store).unwrap();
        assert_eq!(result.sessions.len(), 1);
        assert_eq!(result.sessions[0].title, "Test");
    }

    #[test]
    fn tombstone_rows_are_skipped() {
        let store: Value = serde_json::json!([[{
            "sessions": [{
                "deleted-id": [{
                    "title": ["\u{FFFC}"]
                }],
                "valid-id": [{
                    "title": ["Valid"]
                }]
            }]
        }]]);
        let result = parse_store(&store).unwrap();
        assert_eq!(result.sessions.len(), 1);
        assert_eq!(result.sessions[0].id, "valid-id");
    }
}
