use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use hypr_db_parser::{Collection, Transcript};
use hypr_version::Version;
use serde_json::Value;

use super::utils::{FileOp, apply_ops, group_by_session_id, hints_to_json, transcript_to_json};
use super::version_from_name;
use crate::Result;

mod files {
    // just to be safe
    pub const TRANSCRIPT_LEGACY: &str = "_transcript.json";
    pub const TRANSCRIPT: &str = "transcript.json";
}

pub struct Migrate;

impl super::Migration for Migrate {
    fn introduced_in(&self) -> &'static Version {
        version_from_name!()
    }

    fn run<'a>(&self, base_dir: &'a Path) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(run_inner(base_dir))
    }
}

async fn run_inner(base_dir: &Path) -> Result<()> {
    let sqlite_path = base_dir.join("db.sqlite");
    if !sqlite_path.exists() {
        return Ok(());
    }

    let data = match try_parse_sqlite(&sqlite_path).await {
        Some(data) => data,
        None => return Ok(()),
    };

    let ops = collect_repair_ops(base_dir, &data)?;
    apply_ops(ops)?;

    Ok(())
}

async fn try_parse_sqlite(path: &Path) -> Option<Collection> {
    if hypr_db_parser::v1::validate(path).await.is_ok() {
        return hypr_db_parser::v1::parse_from_sqlite(path).await.ok();
    }

    if hypr_db_parser::v0::validate(path).await.is_ok() {
        return hypr_db_parser::v0::parse_from_sqlite(path).await.ok();
    }

    None
}

fn collect_repair_ops(base_dir: &Path, data: &Collection) -> Result<Vec<FileOp>> {
    let sessions_dir = base_dir.join("sessions");
    let transcripts_by_session = group_by_session_id(&data.transcripts, |t| &t.session_id);

    let mut ops = vec![];

    for session in &data.sessions {
        let sid = session.id.as_str();
        let session_dir = sessions_dir.join(sid);
        let transcript_path = session_dir.join(files::TRANSCRIPT);
        let transcript_path_legacy = session_dir.join(files::TRANSCRIPT_LEGACY);

        let transcript_path = if transcript_path.exists() {
            transcript_path
        } else if transcript_path_legacy.exists() {
            transcript_path_legacy
        } else {
            continue;
        };

        let sqlite_transcripts = transcripts_by_session
            .get(sid)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        if sqlite_transcripts.is_empty() {
            continue;
        }

        if let Some(merged_content) = merge_transcripts(&transcript_path, sqlite_transcripts) {
            ops.push(FileOp::Write {
                path: transcript_path,
                content: merged_content,
                force: true,
            });
        }
    }

    Ok(ops)
}

fn merge_transcripts(path: &Path, sqlite_transcripts: &[&Transcript]) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let json: Value = serde_json::from_str(&content).ok()?;
    let file_transcripts = json.get("transcripts")?.as_array()?;

    let file_ids: HashSet<&str> = file_transcripts
        .iter()
        .filter_map(|t| t.get("id")?.as_str())
        .collect();

    let missing_transcripts: Vec<_> = sqlite_transcripts
        .iter()
        .filter(|t| !file_ids.contains(t.id.as_str()))
        .collect();

    let sqlite_has_hints = sqlite_transcripts
        .iter()
        .any(|t| !t.speaker_hints.is_empty());

    let file_has_hints = file_transcripts.iter().any(|t| {
        t.get("speaker_hints")
            .and_then(|h| h.as_array())
            .map(|arr| !arr.is_empty())
            .unwrap_or(false)
    });

    let needs_hint_repair = sqlite_has_hints && !file_has_hints;

    if missing_transcripts.is_empty() && !needs_hint_repair {
        return None;
    }

    let mut merged: Vec<Value> = file_transcripts.clone();

    for transcript in missing_transcripts {
        merged.push(transcript_to_json(transcript));
    }

    if needs_hint_repair {
        let sqlite_by_id: HashMap<&str, &&Transcript> = sqlite_transcripts
            .iter()
            .map(|t| (t.id.as_str(), t))
            .collect();

        for file_t in &mut merged {
            let Some(id) = file_t.get("id").and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(sqlite_t) = sqlite_by_id.get(id) else {
                continue;
            };
            if sqlite_t.speaker_hints.is_empty() {
                continue;
            }
            let current_hints = file_t
                .get("speaker_hints")
                .and_then(|h| h.as_array())
                .map(|arr| arr.len())
                .unwrap_or(0);
            if current_hints == 0 {
                file_t["speaker_hints"] = hints_to_json(&sqlite_t.speaker_hints);
            }
        }
    }

    merged.sort_by(|a, b| {
        let a_started = a.get("started_at").and_then(|v| v.as_i64()).unwrap_or(0);
        let b_started = b.get("started_at").and_then(|v| v.as_i64()).unwrap_or(0);
        a_started.cmp(&b_started)
    });

    let result = serde_json::json!({ "transcripts": merged });
    Some(serde_json::to_string_pretty(&result).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_db_parser::{Session, SpeakerHint, Word};
    use tempfile::tempdir;

    fn make_word(id: &str, text: &str) -> Word {
        Word {
            id: id.to_string(),
            text: text.to_string(),
            start_ms: Some(0.0),
            end_ms: Some(100.0),
            channel: 0,
            speaker: None,
        }
    }

    fn make_hint(word_id: &str) -> SpeakerHint {
        SpeakerHint {
            word_id: word_id.to_string(),
            hint_type: "manual".to_string(),
            value: r#""Speaker A""#.to_string(),
        }
    }

    fn make_transcript(id: &str, session_id: &str, started_at: f64) -> Transcript {
        Transcript {
            id: id.to_string(),
            user_id: "user1".to_string(),
            created_at: "2024-01-01".to_string(),
            session_id: session_id.to_string(),
            title: "".to_string(),
            started_at,
            ended_at: Some(started_at + 1000.0),
            start_ms: None,
            end_ms: None,
            words: vec![make_word("w1", "hello")],
            speaker_hints: vec![],
        }
    }

    fn make_transcript_with_hints(
        id: &str,
        session_id: &str,
        started_at: f64,
        hints: Vec<SpeakerHint>,
    ) -> Transcript {
        let mut t = make_transcript(id, session_id, started_at);
        t.speaker_hints = hints;
        t
    }

    fn make_session(id: &str) -> Session {
        Session {
            id: id.to_string(),
            user_id: "user1".to_string(),
            created_at: "2024-01-01".to_string(),
            title: "Test Session".to_string(),
            raw_md: None,
            enhanced_content: None,
            folder_id: None,
            event_id: None,
        }
    }

    fn write_transcript_file(dir: &Path, transcripts: &[Value]) {
        let content = serde_json::json!({ "transcripts": transcripts });
        std::fs::write(dir, serde_json::to_string_pretty(&content).unwrap()).unwrap();
    }

    #[test]
    fn test_merge_adds_missing_transcripts() {
        let temp = tempdir().unwrap();
        let transcript_path = temp.path().join("transcript.json");

        let file_transcript = serde_json::json!({
            "id": "t1",
            "user_id": "user1",
            "created_at": "2024-01-01",
            "session_id": "s1",
            "started_at": 1000,
            "ended_at": 2000,
            "words": [],
            "speaker_hints": []
        });
        write_transcript_file(&transcript_path, &[file_transcript]);

        let sqlite_t1 = make_transcript("t1", "s1", 1000.0);
        let sqlite_t2 = make_transcript("t2", "s1", 3000.0);
        let sqlite_transcripts: Vec<&Transcript> = vec![&sqlite_t1, &sqlite_t2];

        let result = merge_transcripts(&transcript_path, &sqlite_transcripts);
        assert!(result.is_some());

        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        let transcripts = merged["transcripts"].as_array().unwrap();

        assert_eq!(transcripts.len(), 2);
        assert_eq!(transcripts[0]["id"], "t1");
        assert_eq!(transcripts[1]["id"], "t2");
    }

    #[test]
    fn test_merge_preserves_existing_transcripts() {
        let temp = tempdir().unwrap();
        let transcript_path = temp.path().join("transcript.json");

        let file_transcript = serde_json::json!({
            "id": "t1",
            "user_id": "user1",
            "created_at": "2024-01-01",
            "session_id": "s1",
            "started_at": 1000,
            "ended_at": 2000,
            "words": [{"id": "original_word", "text": "original"}],
            "speaker_hints": []
        });
        write_transcript_file(&transcript_path, &[file_transcript]);

        let sqlite_t1 = make_transcript("t1", "s1", 1000.0);
        let sqlite_t2 = make_transcript("t2", "s1", 3000.0);
        let sqlite_transcripts: Vec<&Transcript> = vec![&sqlite_t1, &sqlite_t2];

        let result = merge_transcripts(&transcript_path, &sqlite_transcripts);
        assert!(result.is_some());

        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        let transcripts = merged["transcripts"].as_array().unwrap();

        let t1_words = transcripts[0]["words"].as_array().unwrap();
        assert_eq!(t1_words[0]["id"], "original_word");
    }

    #[test]
    fn test_merge_repairs_missing_speaker_hints() {
        let temp = tempdir().unwrap();
        let transcript_path = temp.path().join("transcript.json");

        let file_transcript = serde_json::json!({
            "id": "t1",
            "user_id": "user1",
            "created_at": "2024-01-01",
            "session_id": "s1",
            "started_at": 1000,
            "ended_at": 2000,
            "words": [],
            "speaker_hints": []
        });
        write_transcript_file(&transcript_path, &[file_transcript]);

        let sqlite_t1 = make_transcript_with_hints("t1", "s1", 1000.0, vec![make_hint("w1")]);
        let sqlite_transcripts: Vec<&Transcript> = vec![&sqlite_t1];

        let result = merge_transcripts(&transcript_path, &sqlite_transcripts);
        assert!(result.is_some());

        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        let transcripts = merged["transcripts"].as_array().unwrap();

        let hints = transcripts[0]["speaker_hints"].as_array().unwrap();
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0]["word_id"], "w1");
    }

    #[test]
    fn test_merge_skips_when_all_present() {
        let temp = tempdir().unwrap();
        let transcript_path = temp.path().join("transcript.json");

        let file_transcript = serde_json::json!({
            "id": "t1",
            "user_id": "user1",
            "created_at": "2024-01-01",
            "session_id": "s1",
            "started_at": 1000,
            "ended_at": 2000,
            "words": [],
            "speaker_hints": []
        });
        write_transcript_file(&transcript_path, &[file_transcript]);

        let sqlite_t1 = make_transcript("t1", "s1", 1000.0);
        let sqlite_transcripts: Vec<&Transcript> = vec![&sqlite_t1];

        let result = merge_transcripts(&transcript_path, &sqlite_transcripts);
        assert!(result.is_none());
    }

    #[test]
    fn test_merge_sorts_by_started_at() {
        let temp = tempdir().unwrap();
        let transcript_path = temp.path().join("transcript.json");

        let file_transcript = serde_json::json!({
            "id": "t2",
            "user_id": "user1",
            "created_at": "2024-01-01",
            "session_id": "s1",
            "started_at": 3000,
            "ended_at": 4000,
            "words": [],
            "speaker_hints": []
        });
        write_transcript_file(&transcript_path, &[file_transcript]);

        let sqlite_t1 = make_transcript("t1", "s1", 1000.0);
        let sqlite_t2 = make_transcript("t2", "s1", 3000.0);
        let sqlite_transcripts: Vec<&Transcript> = vec![&sqlite_t1, &sqlite_t2];

        let result = merge_transcripts(&transcript_path, &sqlite_transcripts);
        assert!(result.is_some());

        let merged: Value = serde_json::from_str(&result.unwrap()).unwrap();
        let transcripts = merged["transcripts"].as_array().unwrap();

        assert_eq!(transcripts[0]["id"], "t1");
        assert_eq!(transcripts[1]["id"], "t2");
    }

    #[test]
    fn test_collect_repair_ops_skips_missing_sessions() {
        let temp = tempdir().unwrap();
        let sessions_dir = temp.path().join("sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();

        let data = Collection {
            sessions: vec![make_session("s1")],
            transcripts: vec![make_transcript("t1", "s1", 1000.0)],
            ..Default::default()
        };

        let ops = collect_repair_ops(temp.path(), &data).unwrap();
        assert!(ops.is_empty());
    }

    #[test]
    fn test_collect_repair_ops_generates_write_for_missing_transcripts() {
        let temp = tempdir().unwrap();
        let session_dir = temp.path().join("sessions").join("s1");
        std::fs::create_dir_all(&session_dir).unwrap();

        let file_transcript = serde_json::json!({
            "id": "t1",
            "user_id": "user1",
            "created_at": "2024-01-01",
            "session_id": "s1",
            "started_at": 1000,
            "ended_at": 2000,
            "words": [],
            "speaker_hints": []
        });
        write_transcript_file(&session_dir.join("transcript.json"), &[file_transcript]);

        let t1 = make_transcript("t1", "s1", 1000.0);
        let t2 = make_transcript("t2", "s1", 3000.0);

        let data = Collection {
            sessions: vec![make_session("s1")],
            transcripts: vec![t1, t2],
            ..Default::default()
        };

        let ops = collect_repair_ops(temp.path(), &data).unwrap();
        assert_eq!(ops.len(), 1);

        match &ops[0] {
            FileOp::Write { path, force, .. } => {
                assert!(path.ends_with("transcript.json"));
                assert!(*force);
            }
        }
    }
}
