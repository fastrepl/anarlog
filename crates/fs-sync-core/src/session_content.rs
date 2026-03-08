use std::str::FromStr;

use crate::frontmatter::ParsedDocument;
use crate::types::{SessionContentData, SessionMetaData, SessionNoteData, TranscriptData};

const SESSION_META_FILE: &str = "_meta.json";
const SESSION_MEMO_FILE: &str = "_memo.md";
const SESSION_TRANSCRIPT_FILE: &str = "transcript.json";

pub fn load_session_content(session_id: &str, session_dir: &std::path::Path) -> SessionContentData {
    let mut content = SessionContentData {
        session_id: session_id.to_string(),
        meta: None,
        raw_memo_tiptap_json: None,
        raw_memo_markdown: None,
        transcript: None,
        notes: vec![],
    };

    let entries = match std::fs::read_dir(session_dir) {
        Ok(entries) => entries,
        Err(_) => return content,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let name = match path.file_name().and_then(|v| v.to_str()) {
            Some(name) => name,
            None => continue,
        };

        let file_content = match std::fs::read_to_string(&path) {
            Ok(value) => value,
            Err(_) => continue,
        };

        if name == SESSION_META_FILE {
            if let Ok(meta) = serde_json::from_str::<SessionMetaData>(&file_content) {
                content.meta = Some(meta);
            }
            continue;
        }

        if name == SESSION_TRANSCRIPT_FILE {
            if let Ok(transcript) = serde_json::from_str::<TranscriptData>(&file_content) {
                content.transcript = Some(transcript);
            }
            continue;
        }

        if !name.ends_with(".md") {
            continue;
        }

        let parsed = match ParsedDocument::from_str(&file_content) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };

        let tiptap_json = match hypr_tiptap::md_to_tiptap_json(&parsed.content) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let frontmatter = parsed.frontmatter;
        let id = frontmatter
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let frontmatter_session_id = frontmatter
            .get("session_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if frontmatter_session_id != session_id {
            continue;
        }

        if name == SESSION_MEMO_FILE {
            content.raw_memo_tiptap_json = Some(tiptap_json);
            let trimmed = parsed.content.trim();
            if !trimmed.is_empty() {
                content.raw_memo_markdown = Some(trimmed.to_string());
            }
            continue;
        }

        if id.is_empty() {
            continue;
        }

        let markdown = {
            let trimmed = parsed.content.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        };

        content.notes.push(SessionNoteData {
            id,
            session_id: frontmatter_session_id,
            template_id: frontmatter
                .get("template_id")
                .and_then(|v| v.as_str())
                .map(|v| v.to_string()),
            position: frontmatter.get("position").and_then(|v| v.as_i64()),
            title: frontmatter
                .get("title")
                .and_then(|v| v.as_str())
                .map(|v| v.to_string()),
            tiptap_json,
            markdown,
        });
    }

    content
}
