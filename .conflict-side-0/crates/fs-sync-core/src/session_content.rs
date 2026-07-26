use std::str::FromStr;

use crate::frontmatter::ParsedDocument;
use crate::types::{SessionContentData, SessionMetaData, SessionNoteData, TranscriptJson};

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
            if let Ok(transcript) = serde_json::from_str::<TranscriptJson>(&file_content) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use assert_fs::fixture::PathChild;
    use assert_fs::prelude::*;

    #[test]
    fn load_session_content_preserves_full_transcript_shape() {
        let temp = TempDir::new().unwrap();
        let session_dir = temp.child("session-1");
        session_dir.create_dir_all().unwrap();
        session_dir
            .child(SESSION_TRANSCRIPT_FILE)
            .write_str(
                r#"{
  "transcripts": [
    {
      "id": "transcript-1",
      "user_id": "user-1",
      "created_at": "2024-01-01T00:00:00Z",
      "session_id": "session-1",
      "started_at": 0,
      "ended_at": 1200,
      "memo_md": "memo",
      "words": [
        {
          "id": "word-1",
          "text": "hello",
          "start_ms": 0,
          "end_ms": 400,
          "channel": 0,
          "speaker": "Speaker 1",
          "metadata": { "confidence": 0.98 }
        }
      ],
      "speaker_hints": [
        {
          "id": "hint-1",
          "word_id": "word-1",
          "type": "speaker_label",
          "value": { "label": "Speaker 1" }
        }
      ]
    }
  ]
}"#,
            )
            .unwrap();

        let content = load_session_content("session-1", session_dir.path());
        let transcript = content.transcript.expect("expected transcript");
        let entry = &transcript.transcripts[0];

        assert_eq!(entry.memo_md, "memo");
        assert_eq!(entry.words[0].speaker.as_deref(), Some("Speaker 1"));
        assert_eq!(
            entry.words[0]
                .metadata
                .as_ref()
                .and_then(|value| value.get("confidence"))
                .and_then(|value| value.as_f64()),
            Some(0.98)
        );
        assert_eq!(
            entry.speaker_hints[0]
                .value
                .get("label")
                .and_then(|value| value.as_str()),
            Some("Speaker 1")
        );
    }

    #[test]
    fn load_session_content_accepts_legacy_null_and_omitted_transcript_fields() {
        let temp = TempDir::new().unwrap();
        let session_dir = temp.child("session-1");
        session_dir.create_dir_all().unwrap();
        session_dir
            .child(SESSION_TRANSCRIPT_FILE)
            .write_str(
                r#"{
  "transcripts": [
    {
      "id": "transcript-1",
      "user_id": null,
      "created_at": null,
      "session_id": "session-1",
      "started_at": null,
      "ended_at": null,
      "memo_md": null,
      "words": [
        {
          "text": "hello",
          "start_ms": 0,
          "end_ms": 400,
          "channel": 0,
          "speaker": null,
          "metadata": null
        }
      ],
      "speaker_hints": null
    },
    {
      "id": "transcript-2",
      "session_id": "session-1",
      "words": [
        {
          "text": "world",
          "start_ms": 400,
          "end_ms": 800,
          "channel": 0
        }
      ],
      "speaker_hints": [
        {
          "word_id": "word-1",
          "type": "speaker_label"
        }
      ]
    }
  ]
}"#,
            )
            .unwrap();

        let content = load_session_content("session-1", session_dir.path());
        let transcript = content.transcript.expect("expected transcript");

        assert_eq!(transcript.transcripts.len(), 2);

        let first = &transcript.transcripts[0];
        assert_eq!(first.user_id, "");
        assert_eq!(first.created_at, "");
        assert_eq!(first.started_at, 0.0);
        assert_eq!(first.ended_at, None);
        assert_eq!(first.memo_md, "");
        assert_eq!(first.words[0].id, None);
        assert_eq!(first.words[0].speaker, None);
        assert_eq!(first.words[0].metadata, None);
        assert!(first.speaker_hints.is_empty());

        let second = &transcript.transcripts[1];
        assert_eq!(second.user_id, "");
        assert_eq!(second.created_at, "");
        assert_eq!(second.started_at, 0.0);
        assert_eq!(second.ended_at, None);
        assert_eq!(second.memo_md, "");
        assert_eq!(second.words[0].id, None);
        assert_eq!(second.speaker_hints[0].id, None);
        assert_eq!(second.speaker_hints[0].value, serde_json::Value::Null);
    }
}
