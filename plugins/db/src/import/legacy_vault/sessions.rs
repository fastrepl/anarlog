use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

use anlg_db_app::{
    LegacyAttachment, LegacyDocument, LegacyImportBatch, LegacyImportRow, LegacyParticipant,
    LegacySession, LegacySessionTag, LegacyTag, LegacyTranscript,
};
use anlg_fs_sync_core::frontmatter::ParsedDocument;
use serde_json::Value;

use super::discovery::SourceFile;
use super::source_format::{
    append_warning, content_type_for_path, infer_session_id_and_folder, normalized_relative_path,
    parse_json_object, stable_id, value_i64, value_string,
};

pub(super) const RECOVERED_MISSING_SESSION_METADATA: &str = "recovered_missing_session_metadata";

pub(super) fn parse_session_meta(
    vault_base: &Path,
    path: &Path,
    content: &str,
) -> Result<LegacyImportBatch, String> {
    let meta = parse_json_object(content)?;
    let (session_id, folder_path) = infer_session_id_and_folder(vault_base, path)?;
    let mut batch = LegacyImportBatch::default();

    if let Some(meta_id) = value_string(meta.get("id"))
        && meta_id != session_id
    {
        append_warning(
            &mut batch,
            format!("metadata id {meta_id} differs from folder id {session_id}"),
        );
    }

    let event = meta.get("event").and_then(Value::as_object);
    let event_json = event
        .map(|event| Value::Object(event.clone()).to_string())
        .unwrap_or_default();
    let event_id = value_string(meta.get("event_id")).unwrap_or_default();
    let external_event_id = event
        .and_then(|event| value_string(event.get("tracking_id")))
        .unwrap_or_default();
    let started_at = event
        .and_then(|event| value_string(event.get("started_at")))
        .unwrap_or_default();
    let ended_at = event
        .and_then(|event| value_string(event.get("ended_at")))
        .unwrap_or_default();
    let series_id = event
        .and_then(|event| value_string(event.get("recurrence_series_id")))
        .unwrap_or_default();
    let stored_title = value_string(meta.get("title")).unwrap_or_default();
    let title = if stored_title.trim().is_empty() {
        recover_title_from_summary(path).unwrap_or(stored_title)
    } else {
        stored_title
    };

    batch.rows.push(LegacyImportRow::Session(LegacySession {
        id: session_id.clone(),
        owner_user_id: value_string(meta.get("user_id")).unwrap_or_default(),
        title,
        created_at: value_string(meta.get("created_at")).unwrap_or_default(),
        started_at,
        ended_at,
        event_id,
        external_event_id,
        external_provider: String::new(),
        series_id,
        event_json,
        metadata_json: "{}".to_string(),
        folder_path,
        recovery_status: None,
    }));

    if let Some(participants) = meta.get("participants").and_then(Value::as_array) {
        for (index, participant) in participants.iter().enumerate() {
            let Some(participant) = participant.as_object() else {
                batch.skipped_count += 1;
                append_warning(&mut batch, format!("participant {index} is not an object"));
                continue;
            };
            let human_id = value_string(participant.get("human_id")).unwrap_or_default();
            if human_id.is_empty() {
                batch.skipped_count += 1;
                append_warning(&mut batch, format!("participant {index} has no human_id"));
                continue;
            }
            let id = value_string(participant.get("id")).unwrap_or_else(|| {
                stable_id(&format!("{session_id}:participant:{human_id}:{index}"))
            });
            batch
                .rows
                .push(LegacyImportRow::Participant(LegacyParticipant {
                    id,
                    owner_user_id: value_string(participant.get("user_id")).unwrap_or_default(),
                    session_id: session_id.clone(),
                    human_id,
                    source: value_string(participant.get("source")).unwrap_or_default(),
                }));
        }
    }

    if let Some(key_facts) = meta.get("key_facts").and_then(Value::as_object) {
        let body = value_string(key_facts.get("content")).unwrap_or_default();
        let source_hash = value_string(key_facts.get("source_hash")).unwrap_or_default();
        if !body.is_empty() || !source_hash.is_empty() {
            batch.rows.push(LegacyImportRow::Document(LegacyDocument {
                id: format!("{session_id}:key_facts"),
                session_id: session_id.clone(),
                kind: "key_facts".to_string(),
                template_id: String::new(),
                title: "Key facts".to_string(),
                body_format: "markdown".to_string(),
                body,
                source_hash,
                sort_order: 0,
                created_by: value_string(key_facts.get("user_id"))
                    .or_else(|| value_string(meta.get("user_id")))
                    .unwrap_or_default(),
                created_at: value_string(key_facts.get("created_at")).unwrap_or_default(),
                updated_at: value_string(key_facts.get("updated_at")).unwrap_or_default(),
                generation_metadata_json: "{}".to_string(),
                recovery_status: None,
            }));
        }
    }

    if let Some(tags) = meta.get("tags").and_then(Value::as_array) {
        let owner_user_id = value_string(meta.get("user_id")).unwrap_or_default();
        for (index, tag) in tags.iter().enumerate() {
            let Some(name) = value_string(Some(tag)) else {
                batch.skipped_count += 1;
                append_warning(&mut batch, format!("tag {index} is not a string"));
                continue;
            };
            if name.is_empty() {
                continue;
            }
            batch.rows.push(LegacyImportRow::Tag(LegacyTag {
                id: name.clone(),
                owner_user_id: owner_user_id.clone(),
                name: name.clone(),
            }));
            batch
                .rows
                .push(LegacyImportRow::SessionTag(LegacySessionTag {
                    id: format!("{session_id}:{name}"),
                    owner_user_id: owner_user_id.clone(),
                    session_id: session_id.clone(),
                    tag_id: name,
                }));
        }
    }

    Ok(batch)
}

fn recover_title_from_summary(meta_path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(meta_path.with_file_name("_summary.md")).ok()?;
    let document = ParsedDocument::from_str(&content).ok()?;
    let title = document
        .content
        .trim_start()
        .lines()
        .next()?
        .strip_prefix("# ")?
        .trim();

    if title.is_empty()
        || matches!(
            title.to_ascii_lowercase().as_str(),
            "summary" | "untitled" | "untitled note"
        )
    {
        return None;
    }

    Some(title.to_string())
}

pub(super) fn parse_session_document(
    vault_base: &Path,
    path: &Path,
    content: &str,
    source_sha256: &str,
) -> Result<LegacyImportBatch, String> {
    let document = ParsedDocument::from_str(content).map_err(|error| error.to_string())?;
    let inferred_session_id = infer_session_id_and_folder(vault_base, path)?.0;
    let session_id = value_string(document.frontmatter.get("session_id"))
        .filter(|id| !id.is_empty())
        .unwrap_or(inferred_session_id);
    if session_id.is_empty() {
        return Err("document has no session id".to_string());
    }

    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let template_id = value_string(document.frontmatter.get("template_id")).unwrap_or_default();
    let kind = if filename == "_memo.md" {
        "note"
    } else if filename == "_summary.md" {
        "summary"
    } else if !template_id.is_empty() {
        "template_output"
    } else {
        "summary"
    };
    let id = value_string(document.frontmatter.get("id"))
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| stable_id(&format!("{session_id}:document:{filename}")));

    Ok(LegacyImportBatch {
        rows: vec![LegacyImportRow::Document(LegacyDocument {
            id,
            session_id,
            kind: kind.to_string(),
            template_id,
            title: value_string(document.frontmatter.get("title")).unwrap_or_default(),
            body_format: "markdown".to_string(),
            body: document.content,
            source_hash: source_sha256.to_string(),
            sort_order: value_i64(document.frontmatter.get("position")).unwrap_or(0),
            created_by: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            generation_metadata_json: "{}".to_string(),
            recovery_status: None,
        })],
        ..Default::default()
    })
}

pub(super) fn parse_transcript(
    vault_base: &Path,
    path: &Path,
    content: &str,
    source_sha256: &str,
) -> Result<LegacyImportBatch, String> {
    let root = parse_json_object(content)?;
    let transcripts = root
        .get("transcripts")
        .and_then(Value::as_array)
        .ok_or_else(|| "transcript file has no transcripts array".to_string())?;
    let (inferred_session_id, folder_path) = infer_session_id_and_folder(vault_base, path)?;
    let source_path = normalized_relative_path(vault_base, path);
    let missing_session_metadata = !path.with_file_name("_meta.json").is_file();
    let recovery_metadata_json = missing_session_metadata.then(|| {
        serde_json::json!({
            "legacy_recovery": {
                "reason": "missing_session_metadata",
                "source_path": source_path,
                "source_sha256": source_sha256,
            }
        })
        .to_string()
    });
    let mut batch = LegacyImportBatch::default();
    let mut recovered_sessions = HashMap::<String, (String, String)>::new();

    for (index, transcript) in transcripts.iter().enumerate() {
        let Some(transcript) = transcript.as_object() else {
            batch.skipped_count += 1;
            append_warning(&mut batch, format!("transcript {index} is not an object"));
            continue;
        };
        let session_id = value_string(transcript.get("session_id"))
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| inferred_session_id.clone());
        if session_id.is_empty() {
            batch.skipped_count += 1;
            append_warning(&mut batch, format!("transcript {index} has no session id"));
            continue;
        }
        let id = value_string(transcript.get("id"))
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| stable_id(&format!("{session_id}:transcript:{index}")));
        let mut words = transcript
            .get("words")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for (word_index, word) in words.iter_mut().enumerate() {
            let Some(word) = word.as_object_mut() else {
                append_warning(
                    &mut batch,
                    format!("transcript {index} word {word_index} is not an object"),
                );
                continue;
            };
            let has_id = word
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| !id.is_empty());
            if !has_id {
                word.insert(
                    "id".to_string(),
                    Value::String(format!("{id}:word:{word_index}")),
                );
            }
        }

        let speaker_hints = transcript
            .get("speaker_hints")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let owner_user_id = value_string(transcript.get("user_id")).unwrap_or_default();
        let created_at = value_string(transcript.get("created_at")).unwrap_or_default();
        if missing_session_metadata {
            let recovered = recovered_sessions
                .entry(session_id.clone())
                .or_insert_with(|| (owner_user_id.clone(), created_at.clone()));
            if recovered.0.is_empty() && !owner_user_id.is_empty() {
                recovered.0.clone_from(&owner_user_id);
            }
            if !created_at.is_empty() && (recovered.1.is_empty() || created_at < recovered.1) {
                recovered.1.clone_from(&created_at);
            }
        }
        batch
            .rows
            .push(LegacyImportRow::Transcript(LegacyTranscript {
                id,
                owner_user_id,
                session_id: session_id.clone(),
                started_at_ms: value_i64(transcript.get("started_at")).unwrap_or(0),
                ended_at_ms: value_i64(transcript.get("ended_at")),
                memo: value_string(transcript.get("memo_md")).unwrap_or_default(),
                words_json: Value::Array(words).to_string(),
                speaker_hints_json: Value::Array(speaker_hints).to_string(),
                created_at,
                metadata_json: recovery_metadata_json
                    .clone()
                    .unwrap_or_else(|| "{}".to_string()),
                recovery_status: missing_session_metadata
                    .then(|| RECOVERED_MISSING_SESSION_METADATA.to_string()),
            }));
    }

    if let Some(metadata_json) = recovery_metadata_json {
        let mut sessions = recovered_sessions.into_iter().collect::<Vec<_>>();
        sessions.sort_by(|left, right| left.0.cmp(&right.0));
        let mut rows = sessions
            .into_iter()
            .map(|(session_id, (owner_user_id, created_at))| {
                LegacyImportRow::Session(LegacySession {
                    id: session_id.clone(),
                    owner_user_id,
                    title: recovered_transcript_title(&session_id, &created_at),
                    created_at,
                    started_at: String::new(),
                    ended_at: String::new(),
                    event_id: String::new(),
                    external_event_id: String::new(),
                    external_provider: String::new(),
                    series_id: String::new(),
                    event_json: String::new(),
                    metadata_json: metadata_json.clone(),
                    folder_path: folder_path.clone(),
                    recovery_status: Some(RECOVERED_MISSING_SESSION_METADATA.to_string()),
                })
            })
            .collect::<Vec<_>>();
        rows.append(&mut batch.rows);
        batch.rows = rows;
    }

    Ok(batch)
}

fn recovered_transcript_title(session_id: &str, created_at: &str) -> String {
    let date = created_at.get(..10).filter(|date| {
        date.as_bytes().iter().enumerate().all(|(index, byte)| {
            matches!(index, 4 | 7) && *byte == b'-'
                || !matches!(index, 4 | 7) && byte.is_ascii_digit()
        })
    });
    match date {
        Some(date) => format!("Recovered transcript — {date}"),
        None => format!("Recovered transcript — {session_id}"),
    }
}

pub(super) fn parse_attachment(
    vault_base: &Path,
    source: &SourceFile,
    bytes: &[u8],
    source_sha256: &str,
) -> Result<LegacyImportBatch, String> {
    let parts = source.relative_path.split('/').collect::<Vec<_>>();
    let attachment_index = parts
        .iter()
        .position(|part| *part == "attachments")
        .ok_or_else(|| "attachment path has no attachments directory".to_string())?;
    if attachment_index < 2 {
        return Err("attachment path has no session directory".to_string());
    }
    let session_id = parts[attachment_index - 1].to_string();
    let filename = source
        .path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "attachment filename is not valid UTF-8".to_string())?
        .to_string();
    let relative_path = normalized_relative_path(vault_base, &source.path);

    Ok(LegacyImportBatch {
        rows: vec![LegacyImportRow::Attachment(LegacyAttachment {
            id: stable_id(&format!("{session_id}:attachment:{relative_path}")),
            session_id,
            filename: filename.clone(),
            relative_path,
            content_type: content_type_for_path(&source.path).to_string(),
            size_bytes: i64::try_from(bytes.len()).unwrap_or(i64::MAX),
            sha256: source_sha256.to_string(),
            source_id: filename,
        })],
        ..Default::default()
    })
}
