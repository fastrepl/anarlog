use crate::{Args, Error, Result, db, output};
use sha2::{Digest, Sha256};
use sqlx::Connection;
use std::path::{Component, Path, PathBuf};

fn invalid(reason: &'static str) -> Error {
    Error::operation("delete meeting", reason)
}

/// Call only while the desktop app is closed. The agent enforces this too.
fn closed(base: &Path) -> Result<Vec<std::fs::File>> {
    let mut locks = Vec::new();
    for name in [
        "launch.lock",
        "com.anarlog.stable.running.lock",
        "com.anarlog.nightly.running.lock",
        "com.hyprnote.stable.running.lock",
        "com.hyprnote.nightly.running.lock",
    ] {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(base.join(name))
            .map_err(|_| invalid("cannot lock recorder"))?;
        file.try_lock()
            .map_err(|_| invalid("close Anarlog before deletion"))?;
        locks.push(file);
    }
    Ok(locks)
}

fn contained(base: &Path, relative: &str) -> Result<PathBuf> {
    let relative = Path::new(relative);
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(invalid("invalid recording path"));
    }
    let path = base.join(relative);
    let mut existing = path.as_path();
    while !existing.exists() {
        existing = existing
            .parent()
            .ok_or_else(|| invalid("invalid recording path"))?;
    }
    if !existing
        .canonicalize()
        .map_err(|_| invalid("recording path unavailable"))?
        .starts_with(base)
    {
        return Err(invalid("recording path escapes storage"));
    }
    Ok(path)
}
fn find_sessions(base: &Path, id: &str, found: &mut Vec<PathBuf>) -> Result<()> {
    let entries = match std::fs::read_dir(base) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(invalid("recording directory unavailable")),
    };
    for entry in entries {
        let entry = entry.map_err(|_| invalid("recording directory unavailable"))?;
        if !entry
            .file_type()
            .map_err(|_| invalid("recording metadata unavailable"))?
            .is_dir()
        {
            continue;
        }
        if entry.file_name() == id {
            found.push(entry.path());
        } else {
            find_sessions(&entry.path(), id, found)?;
        }
    }
    Ok(())
}

pub async fn initialize_managed(args: &Args, json: bool) -> Result<()> {
    let path = db::resolve_path(args)?;
    let base = path
        .parent()
        .ok_or_else(|| invalid("storage unavailable"))?;
    if !base.join("managed_settings.json").is_file() {
        return Err(invalid("managed policy required before initialization"));
    }
    let _locks = closed(base)?;
    let db = anlg_db_core::Db::connect_local_plain(&path)
        .await
        .map_err(|_| invalid("cannot initialize recorder database"))?;
    anlg_db_app::prepare_schema(&db)
        .await
        .map_err(|_| invalid("cannot apply managed recorder policy"))?;
    let protected:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM managed_app_settings WHERE id='intelligence_disabled' AND value_json='true')").fetch_one(db.pool()).await.map_err(|_|invalid("cannot verify managed policy"))?;
    db.pool().close().await;
    if !protected {
        return Err(invalid(
            "managed capture policy requires intelligence disabled",
        ));
    }
    output::emit(&if json {
        output::json(
            "meetings.initialize-managed",
            &serde_json::json!({"managed":true}),
            None,
        )?
    } else {
        "Managed recorder initialized.".into()
    });
    Ok(())
}

pub async fn run(args: &Args, id: &str, expected: &str, json: bool) -> Result<()> {
    if expected.len() != 64
        || !expected.bytes().all(|b| b.is_ascii_hexdigit())
        || id.is_empty()
        || id == "."
        || id == ".."
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(invalid("invalid snapshot identity"));
    }
    let path = db::resolve_path(args)?;
    let base = path
        .parent()
        .ok_or_else(|| invalid("storage unavailable"))?
        .canonicalize()
        .map_err(|_| invalid("storage unavailable"))?;
    let _locks = closed(&base)?;
    let writer = db::open_write(args).await?;
    let mut connection = writer
        .pool()
        .acquire()
        .await
        .map_err(|_| invalid("cannot open meeting"))?;
    sqlx::query("PRAGMA secure_delete=ON")
        .execute(&mut *connection)
        .await
        .map_err(|_| invalid("cannot secure local purge"))?;
    let mut tx = connection
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|_| invalid("cannot lock meeting"))?;
    let pending: Option<(String, String)> = sqlx::query_as(
        "SELECT export_sha256,paths_json FROM local_meeting_purges WHERE meeting_id=?",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| invalid("managed capture migration is required"))?;
    let paths = if let Some((hash, paths)) = pending {
        if hash != expected {
            return Err(invalid("snapshot changed"));
        }
        serde_json::from_str::<Vec<String>>(&paths).map_err(|_| invalid("invalid purge journal"))?
    } else {
        // A separate read-only pool observes the committed snapshot while our
        // IMMEDIATE transaction prevents all competing writers.
        let reader = db::open(args).await?;
        let exported =
            anlg_agent_access::get_local_meeting_export(reader.pool(), id.to_owned()).await?;
        reader.pool().close().await;
        let bytes = format!("{}\n", output::json("meetings.export", &exported, None)?);
        let actual = Sha256::digest(bytes.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        if actual != expected {
            return Err(invalid("snapshot changed"));
        }
        if exported.capture_pending
            || exported.export.meeting.started_at.is_empty()
            || exported.export.meeting.ended_at.is_empty()
            || exported.export.transcripts.is_empty()
            || exported.export.transcripts.iter().any(|t| {
                t.ended_at_ms.is_none()
                    || t.words.is_empty()
                    || t.words.iter().any(|w| w["state"] != "final")
            })
        {
            return Err(invalid("transcription is incomplete"));
        }
        let mut paths = vec!["search_index".to_string()];
        let mut sessions = Vec::new();
        find_sessions(&base.join("sessions"), id, &mut sessions)?;
        for session in sessions {
            paths.push(
                session
                    .strip_prefix(&base)
                    .map_err(|_| invalid("recording path escapes storage"))?
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        for relative in &paths {
            contained(&base, relative)?;
        }
        anlg_db_app::purge_local_session(&mut tx, id)
            .await
            .map_err(|_| invalid("cannot purge local meeting"))?;
        let encoded =
            serde_json::to_string(&paths).map_err(|_| invalid("cannot persist purge journal"))?;
        sqlx::query(
            "INSERT INTO local_meeting_purges(meeting_id,export_sha256,paths_json) VALUES(?,?,?)",
        )
        .bind(id)
        .bind(expected)
        .bind(encoded)
        .execute(&mut *tx)
        .await
        .map_err(|_| invalid("cannot persist purge journal"))?;
        paths
    };
    tx.commit()
        .await
        .map_err(|_| invalid("cannot commit local purge"))?;
    let (busy, _, _): (i64, i64, i64) = sqlx::query_as("PRAGMA wal_checkpoint(TRUNCATE)")
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| invalid("database cleanup awaits retry"))?;
    if busy != 0 {
        return Err(invalid("database cleanup awaits retry"));
    }
    sqlx::query("VACUUM")
        .execute(&mut *connection)
        .await
        .map_err(|_| invalid("database cleanup awaits retry"))?;
    // Keep the journal until every recording and derived search artifact has
    // gone. Retrying the same snapshot resumes after an interrupted removal.
    for relative in paths {
        let target = contained(&base, &relative)?;
        match std::fs::remove_dir_all(&target) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err(invalid("recording cleanup awaits retry")),
        }
    }
    sqlx::query("DELETE FROM local_meeting_purges WHERE meeting_id=? AND export_sha256=?")
        .bind(id)
        .bind(expected)
        .execute(&mut *connection)
        .await
        .map_err(|_| invalid("purge cleanup awaits retry"))?;
    let rendered = if json {
        output::json(
            "meetings.delete",
            &serde_json::json!({"meeting_id":id,"deleted":true,"export_sha256":expected}),
            None,
        )?
    } else {
        "Meeting permanently deleted.".to_string()
    };
    output::emit(&rendered);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    async fn fixture() -> (tempfile::TempDir, Args, String) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.db");
        let db = anlg_db_core::Db::connect_local_plain(&path).await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        for statement in [
            "INSERT INTO app_settings(id,value_json) VALUES('cloud_sync_enabled','false')",
            "INSERT INTO sessions(id,title,started_at,ended_at) VALUES('meeting-1','Raw title','2026-10-01T10:00:00Z','2026-10-01T10:30:00Z')",
            "INSERT INTO sessions(id,title) VALUES('other','Keep this meeting')",
            "INSERT INTO session_documents(id,session_id,kind,body_format,body) VALUES('note-1','meeting-1','note','markdown','Raw historical note')",
            "UPDATE session_documents SET body='Raw current note' WHERE id='note-1'",
            r#"INSERT INTO transcripts(id,session_id,ended_at_ms,words_json) VALUES('transcript-1','meeting-1',1000,'[{"text":"Raw private word","start_ms":0,"end_ms":1000,"channel":0,"state":"final"}]')"#,
            "INSERT INTO transcript_live_state(transcript_id) VALUES('transcript-1')",
            r#"INSERT INTO transcript_live_deltas(id,transcript_id,sequence,delta_json) VALUES('delta-1','transcript-1',0,'{"new_words":["Raw partial"],"replaced_ids":[],"partials":[]}')"#,
        ] {
            sqlx::query(statement).execute(db.pool()).await.unwrap();
        }
        let exported = anlg_agent_access::get_local_meeting_export(db.pool(), "meeting-1".into())
            .await
            .unwrap();
        let bytes = format!(
            "{}\n",
            output::json("meetings.export", &exported, None).unwrap()
        );
        let hash = Sha256::digest(bytes.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        db.pool().close().await;
        for folder in [
            "sessions/project/meeting-1",
            "sessions/old/meeting-1",
            "search_index",
        ] {
            std::fs::create_dir_all(dir.path().join(folder)).unwrap();
            std::fs::write(dir.path().join(folder).join("audio.wav"), b"Raw audio").unwrap();
        }
        let args = Args::try_parse_from([
            "anarlog",
            "--json",
            "--db-path",
            path.to_str().unwrap(),
            "meetings",
            "--source",
            "local",
            "list",
        ])
        .unwrap();
        (dir, args, hash)
    }

    #[tokio::test]
    async fn conditional_purge_removes_history_partials_audio_and_index() {
        let (dir, args, hash) = fixture().await;
        assert!(
            run(&args, "meeting-1", &"0".repeat(64), true)
                .await
                .is_err()
        );
        assert!(
            dir.path()
                .join("sessions/project/meeting-1/audio.wav")
                .exists()
        );
        run(&args, "meeting-1", &hash, true).await.unwrap();
        let db = db::open(&args).await.unwrap();
        for table in [
            "session_documents",
            "session_document_versions",
            "transcripts",
            "transcript_live_state",
            "transcript_live_deltas",
            "local_meeting_purges",
        ] {
            let count: i64 =
                sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
                    .fetch_one(db.pool())
                    .await
                    .unwrap();
            assert_eq!(count, 0, "{table}");
        }
        let remaining: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id='other'")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(remaining, "Keep this meeting");
        db.pool().close().await;
        assert!(!dir.path().join("sessions/project/meeting-1").exists());
        assert!(!dir.path().join("sessions/old/meeting-1").exists());
        assert!(!dir.path().join("search_index").exists());
        let bytes = std::fs::read(dir.path().join("app.db")).unwrap();
        assert!(!bytes.windows(11).any(|w| w == b"Raw private"));
        assert!(!bytes.windows(10).any(|w| w == b"Raw histor"));
    }

    #[tokio::test]
    async fn incomplete_and_running_recordings_are_kept_and_cleanup_can_resume() {
        let (dir, args, hash) = fixture().await;
        let lock = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.path().join("com.hyprnote.stable.running.lock"))
            .unwrap();
        lock.try_lock().unwrap();
        assert!(run(&args, "meeting-1", &hash, true).await.is_err());
        drop(lock);
        let db = db::open_write(&args).await.unwrap();
        sqlx::query("UPDATE transcripts SET ended_at_ms=NULL WHERE id='transcript-1'")
            .execute(db.pool())
            .await
            .unwrap();
        let exported = anlg_agent_access::get_local_meeting_export(db.pool(), "meeting-1".into())
            .await
            .unwrap();
        let bytes = format!(
            "{}\n",
            output::json("meetings.export", &exported, None).unwrap()
        );
        let incomplete = Sha256::digest(bytes.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        db.pool().close().await;
        assert!(run(&args, "meeting-1", &incomplete, true).await.is_err());
        let db = db::open_write(&args).await.unwrap();
        let mut tx = db.pool().begin().await.unwrap();
        anlg_db_app::purge_local_session(&mut tx, "meeting-1")
            .await
            .unwrap();
        sqlx::query("INSERT INTO local_meeting_purges VALUES('meeting-1',?,'[\"sessions/project/meeting-1\",\"sessions/old/meeting-1\",\"search_index\"]')").bind(&hash).execute(&mut *tx).await.unwrap();
        tx.commit().await.unwrap();
        db.pool().close().await;
        // A journal is replayed by the exact original snapshot only.
        assert!(run(&args, "meeting-1", &incomplete, true).await.is_err());
        run(&args, "meeting-1", &hash, true).await.unwrap();
        assert!(!dir.path().join("sessions/project/meeting-1").exists());
    }
    #[tokio::test]
    async fn first_launch_initialization_requires_managed_policy_and_never_enables_intelligence() {
        let dir = tempfile::tempdir().unwrap();
        let args = Args::try_parse_from([
            "anarlog",
            "--json",
            "--base",
            dir.path().to_str().unwrap(),
            "meetings",
            "--source",
            "local",
            "initialize-managed",
        ])
        .unwrap();
        assert!(initialize_managed(&args, true).await.is_err());
        assert!(!dir.path().join("app.db").exists());
        std::fs::write(dir.path().join("managed_settings.json"),br#"{"schema_version":1,"settings":{"intelligence_disabled":true,"telemetry_consent":false,"crash_reporting_consent":false,"cloud_sync_enabled":false,"automatic_updates":false}}"#).unwrap();
        initialize_managed(&args, true).await.unwrap();
        let db = db::open(&args).await.unwrap();
        let values: Vec<(String, String)> =
            sqlx::query_as("SELECT id,value_json FROM managed_app_settings ORDER BY id")
                .fetch_all(db.pool())
                .await
                .unwrap();
        assert_eq!(values.len(), 5);
        assert!(values.iter().all(|(id, value)| value
            == if id == "intelligence_disabled" {
                "true"
            } else {
                "false"
            }));
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM sessions")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            0
        );
        db.pool().close().await;
    }
}
