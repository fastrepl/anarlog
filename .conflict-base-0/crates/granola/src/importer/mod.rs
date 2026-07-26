use crate::api::{Document, GranolaClient};
use crate::cache::{CacheData, CacheDocument, TranscriptSegment, read_cache};
use crate::error::Result;
use crate::prosemirror::convert_to_plain_text;
use hypr_importer_core::ir::{Collection, Session, Tag, TagMapping, Transcript, Word};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

pub async fn import_all_from_path(path: &Path) -> Result<Collection> {
    let supabase_content = std::fs::read(path)?;

    let client = GranolaClient::new(&supabase_content, Duration::from_secs(30))?;
    let documents = client.get_documents().await?;

    let mut sessions = Vec::new();
    let mut tags: Vec<Tag> = Vec::new();
    let mut tag_mappings: Vec<TagMapping> = Vec::new();
    let mut tag_name_to_id: HashMap<String, String> = HashMap::new();

    for doc in documents {
        let session = document_to_session(&doc);

        for tag_name in &doc.tags {
            let tag_id = tag_name_to_id
                .entry(tag_name.to_string())
                .or_insert_with(|| {
                    let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, tag_name.as_bytes())
                        .to_string();
                    tags.push(Tag {
                        id: id.clone(),
                        user_id: String::new(),
                        name: tag_name.clone(),
                    });
                    id
                })
                .clone();

            tag_mappings.push(TagMapping {
                id: format!("{}_{}", tag_id, session.id),
                user_id: String::new(),
                tag_id,
                session_id: session.id.clone(),
            });
        }

        sessions.push(session);
    }

    let cache_path = path
        .parent()
        .map(|p| p.join("cache"))
        .unwrap_or_else(crate::cache::default_cache_path);
    let transcripts = if cache_path.exists() {
        let cache_data = read_cache(&cache_path)?;
        cache_data_to_transcripts(&cache_data)
    } else {
        vec![]
    };

    Ok(Collection {
        sessions,
        transcripts,
        humans: vec![],
        organizations: vec![],
        participants: vec![],
        templates: vec![],
        enhanced_notes: vec![],
        tags,
        tag_mappings,
    })
}

fn document_to_session(doc: &Document) -> Session {
    let content = get_document_content(doc);

    Session {
        id: doc.id.clone(),
        user_id: String::new(),
        created_at: doc.created_at.clone(),
        title: doc.title.clone(),
        raw_md: Some(content),
        enhanced_content: None,
        folder_id: None,
        event_id: None,
    }
}

fn get_document_content(doc: &Document) -> String {
    if let Some(ref notes) = doc.notes {
        let content = convert_to_plain_text(notes).trim().to_string();
        if !content.is_empty() {
            return content;
        }
    }

    if let Some(ref panel) = doc.last_viewed_panel {
        if let Some(ref content) = panel.content {
            let text = convert_to_plain_text(content).trim().to_string();
            if !text.is_empty() {
                return text;
            }
        }

        if !panel.original_content.is_empty() {
            return panel.original_content.clone();
        }
    }

    doc.content.clone()
}

fn cache_data_to_transcripts(cache_data: &CacheData) -> Vec<Transcript> {
    cache_data
        .transcripts
        .iter()
        .filter_map(|(doc_id, segments)| {
            if segments.is_empty() {
                return None;
            }

            let doc = cache_data
                .documents
                .get(doc_id)
                .cloned()
                .unwrap_or_else(|| CacheDocument {
                    id: doc_id.clone(),
                    title: doc_id.clone(),
                    created_at: String::new(),
                    updated_at: String::new(),
                });

            Some(cache_document_to_transcript(&doc, segments))
        })
        .collect()
}

fn cache_document_to_transcript(doc: &CacheDocument, segments: &[TranscriptSegment]) -> Transcript {
    let words: Vec<Word> = segments
        .iter()
        .map(|seg| Word {
            id: seg.id.clone(),
            text: seg.text.clone(),
            start_ms: parse_timestamp_to_ms(&seg.start_timestamp),
            end_ms: parse_timestamp_to_ms(&seg.end_timestamp),
            channel: 0,
            speaker: Some(match seg.source.as_str() {
                "microphone" => "You".to_string(),
                _ => "System".to_string(),
            }),
        })
        .collect();

    let start_ms = words.first().and_then(|w| w.start_ms);
    let end_ms = words.last().and_then(|w| w.end_ms);

    Transcript {
        id: doc.id.clone(),
        user_id: String::new(),
        created_at: doc.created_at.clone(),
        session_id: doc.id.clone(),
        title: doc.title.clone(),
        started_at: start_ms.unwrap_or(0.0),
        ended_at: end_ms,
        start_ms,
        end_ms,
        words,
        speaker_hints: vec![],
    }
}

fn parse_timestamp_to_ms(timestamp: &str) -> Option<f64> {
    let parts: Vec<&str> = timestamp.split(':').collect();
    if parts.len() != 3 {
        return None;
    }

    let hours: f64 = parts[0].parse().ok()?;
    let minutes: f64 = parts[1].parse().ok()?;

    let sec_parts: Vec<&str> = parts[2].split('.').collect();
    let seconds: f64 = sec_parts[0].parse().ok()?;
    let millis: f64 = if sec_parts.len() > 1 {
        let ms_str = sec_parts[1];
        let padded = format!("{:0<3}", ms_str);
        padded[..3].parse().unwrap_or(0.0)
    } else {
        0.0
    };

    Some((hours * 3600.0 + minutes * 60.0 + seconds) * 1000.0 + millis)
}
