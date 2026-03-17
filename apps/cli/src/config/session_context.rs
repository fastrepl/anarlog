use std::path::{Path, PathBuf};

use hypr_fs_sync_core::session_content::load_session_content;
use hypr_fs_sync_core::types::SessionContentData;

use crate::config::desktop;
use crate::error::{CliError, CliResult};

pub fn load_chat_system_message(session_id: &str) -> CliResult<String> {
    let paths = desktop::resolve_paths();
    let sessions_base = paths.vault_base.join("sessions");
    let session_dir = find_session_dir(&sessions_base, session_id).ok_or_else(|| {
        CliError::not_found(
            format!("session '{session_id}'"),
            Some(format!(
                "Expected a session folder named '{session_id}' under {}",
                sessions_base.display()
            )),
        )
    })?;

    let content = load_session_content(session_id, &session_dir);
    if session_content_is_empty(&content) {
        return Err(CliError::operation_failed(
            "load session context",
            format!("session '{session_id}' has no transcript, memo, or notes"),
        ));
    }

    Ok(render_session_context(&content))
}

fn render_session_context(content: &SessionContentData) -> String {
    let mut sections = vec![
        "You are continuing a chat with context from a local Char session.".to_string(),
        format!("Session ID: {}", content.session_id),
    ];

    if let Some(meta) = &content.meta {
        if let Some(title) = meta.title.as_deref().filter(|value| !value.is_empty()) {
            sections.push(format!("Title: {title}"));
        }
        if let Some(created_at) = meta.created_at.as_deref().filter(|value| !value.is_empty()) {
            sections.push(format!("Created At: {created_at}"));
        }
        if let Some(event_name) = extract_event_name(meta.event.as_ref()) {
            sections.push(format!("Event: {event_name}"));
        }
        if !meta.participants.is_empty() {
            let participants = meta
                .participants
                .iter()
                .map(|participant| participant.human_id.as_str())
                .filter(|human_id| !human_id.is_empty())
                .collect::<Vec<_>>();
            if !participants.is_empty() {
                sections.push(format!("Participants: {}", participants.join(", ")));
            }
        }
        if !meta.tags.is_empty() {
            sections.push(format!("Tags: {}", meta.tags.join(", ")));
        }
    }

    if let Some(raw_memo) = content
        .raw_memo_markdown
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        sections.push(format!("Raw Memo:\n{raw_memo}"));
    }

    let enhanced_notes = content
        .notes
        .iter()
        .filter_map(|note| {
            note.markdown
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|markdown| {
                    if let Some(title) = note.title.as_deref().filter(|value| !value.is_empty()) {
                        format!("{title}:\n{markdown}")
                    } else {
                        markdown.to_string()
                    }
                })
        })
        .collect::<Vec<_>>();
    if !enhanced_notes.is_empty() {
        sections.push(format!(
            "Enhanced Notes:\n{}",
            enhanced_notes.join("\n\n---\n\n")
        ));
    }

    if let Some(transcript) = flatten_transcript(content).filter(|value| !value.is_empty()) {
        sections.push(format!("Transcript:\n{transcript}"));
    }

    sections.join("\n\n")
}

fn extract_event_name(event: Option<&serde_json::Value>) -> Option<&str> {
    let event = event?.as_object()?;
    event
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            event
                .get("title")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
        })
}

fn flatten_transcript(content: &SessionContentData) -> Option<String> {
    let transcript = content.transcript.as_ref()?;
    let mut words = transcript
        .transcripts
        .iter()
        .flat_map(|entry| entry.words.iter())
        .collect::<Vec<_>>();

    words.sort_by(|left, right| {
        left.start_ms
            .partial_cmp(&right.start_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let text = words
        .into_iter()
        .map(|word| word.text.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if text.is_empty() { None } else { Some(text) }
}

fn session_content_is_empty(content: &SessionContentData) -> bool {
    let has_meta = content.meta.is_some();
    let has_raw_memo = content
        .raw_memo_markdown
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_notes = content.notes.iter().any(|note| {
        note.markdown
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    });
    let has_transcript = flatten_transcript(content).is_some();

    !(has_meta || has_raw_memo || has_notes || has_transcript)
}

fn find_session_dir(sessions_base: &Path, session_id: &str) -> Option<PathBuf> {
    if !sessions_base.is_dir() {
        return None;
    }

    let direct = sessions_base.join(session_id);
    if direct.is_dir() {
        return Some(direct);
    }

    let mut stack = vec![sessions_base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).ok()?.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if path.file_name().and_then(|value| value.to_str()) == Some(session_id) {
                return Some(path);
            }
            stack.push(path);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_session_context_includes_notes_and_transcript() {
        let content = SessionContentData {
            session_id: "session-1".to_string(),
            meta: None,
            raw_memo_tiptap_json: None,
            raw_memo_markdown: Some("memo text".to_string()),
            transcript: Some(hypr_fs_sync_core::types::TranscriptJson {
                transcripts: vec![hypr_fs_sync_core::types::TranscriptWithData {
                    id: "t1".to_string(),
                    user_id: String::new(),
                    created_at: String::new(),
                    session_id: "session-1".to_string(),
                    started_at: 0.0,
                    ended_at: None,
                    memo_md: String::new(),
                    words: vec![
                        hypr_fs_sync_core::types::TranscriptWord {
                            id: Some("w2".to_string()),
                            text: "world".to_string(),
                            start_ms: 100.0,
                            end_ms: 200.0,
                            channel: 0.0,
                            speaker: None,
                            metadata: None,
                        },
                        hypr_fs_sync_core::types::TranscriptWord {
                            id: Some("w1".to_string()),
                            text: "hello".to_string(),
                            start_ms: 0.0,
                            end_ms: 100.0,
                            channel: 0.0,
                            speaker: None,
                            metadata: None,
                        },
                    ],
                    speaker_hints: vec![],
                }],
            }),
            notes: vec![hypr_fs_sync_core::types::SessionNoteData {
                id: "note-1".to_string(),
                session_id: "session-1".to_string(),
                template_id: None,
                position: Some(0),
                title: Some("Summary".to_string()),
                tiptap_json: serde_json::Value::Null,
                markdown: Some("enhanced".to_string()),
            }],
        };

        let rendered = render_session_context(&content);

        assert!(rendered.contains("Raw Memo:\nmemo text"));
        assert!(rendered.contains("Enhanced Notes:\nSummary:\nenhanced"));
        assert!(rendered.contains("Transcript:\nhello world"));
    }
}
