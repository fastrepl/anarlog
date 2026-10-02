use sqlx::{Row, SqliteConnection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchTranscriptRow {
    pub id: String,
    pub started_at_ms: i64,
    pub words_json: String,
    pub speaker_hints_json: String,
    pub pending_delta_jsons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatchTranscriptInsert {
    pub id: String,
    pub session_id: String,
    pub owner_user_id: String,
    pub created_at: String,
    pub started_at_ms: f64,
    pub memo: String,
    pub provider: String,
    pub model: String,
    pub words_json: String,
    pub speaker_hints_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchTranscriptReplace {
    pub replace_session: bool,
    pub replace_transcript_id: Option<String>,
}

pub async fn load_batch_transcript_by_id(
    conn: &mut SqliteConnection,
    transcript_id: &str,
) -> Result<Option<BatchTranscriptRow>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, started_at_ms, words_json, speaker_hints_json
         FROM transcripts
         WHERE id = ? AND deleted_at IS NULL
         LIMIT 1",
    )
    .bind(transcript_id)
    .fetch_optional(&mut *conn)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };
    let id: String = row.try_get("id")?;
    let pending_delta_jsons = load_pending_delta_jsons(conn, &id).await?;

    Ok(Some(BatchTranscriptRow {
        id,
        started_at_ms: row.try_get("started_at_ms")?,
        words_json: row.try_get("words_json")?,
        speaker_hints_json: row.try_get("speaker_hints_json")?,
        pending_delta_jsons,
    }))
}

pub async fn load_session_batch_transcripts(
    conn: &mut SqliteConnection,
    session_id: &str,
) -> Result<Vec<BatchTranscriptRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, started_at_ms, words_json, speaker_hints_json
         FROM transcripts
         WHERE session_id = ? AND deleted_at IS NULL
         ORDER BY started_at_ms, created_at, id",
    )
    .bind(session_id)
    .fetch_all(&mut *conn)
    .await?;

    let mut transcripts = Vec::with_capacity(rows.len());
    for row in rows {
        let id: String = row.try_get("id")?;
        let pending_delta_jsons = load_pending_delta_jsons(conn, &id).await?;
        transcripts.push(BatchTranscriptRow {
            id,
            started_at_ms: row.try_get("started_at_ms")?,
            words_json: row.try_get("words_json")?,
            speaker_hints_json: row.try_get("speaker_hints_json")?,
            pending_delta_jsons,
        });
    }
    Ok(transcripts)
}

pub(crate) async fn load_pending_delta_jsons(
    conn: &mut SqliteConnection,
    transcript_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT delta_json
         FROM transcript_live_deltas
         WHERE transcript_id = ?
         ORDER BY sequence",
    )
    .bind(transcript_id)
    .fetch_all(&mut *conn)
    .await
}

pub async fn insert_batch_transcript(
    conn: &mut SqliteConnection,
    insert: &BatchTranscriptInsert,
    replace: &BatchTranscriptReplace,
) -> Result<bool, sqlx::Error> {
    if replace.replace_session {
        sqlx::query(
            "UPDATE transcripts
             SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE session_id = ? AND deleted_at IS NULL",
        )
        .bind(&insert.session_id)
        .execute(&mut *conn)
        .await?;
    } else if let Some(replace_transcript_id) = &replace.replace_transcript_id {
        sqlx::query(
            "UPDATE transcripts
             SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ? AND session_id = ? AND deleted_at IS NULL",
        )
        .bind(replace_transcript_id)
        .bind(&insert.session_id)
        .execute(&mut *conn)
        .await?;
    }

    let inserted = sqlx::query(
        "INSERT INTO transcripts (
            id, workspace_id, owner_user_id, session_id, source, provider,
            model, language, started_at_ms, ended_at_ms, audio_attachment_id,
            memo, words_json, speaker_hints_json, metadata_json, created_at,
            updated_at, deleted_at
         )
         SELECT ?, session.workspace_id,
                COALESCE(NULLIF(?, ''), session.owner_user_id),
                session.id, 'batch_transcription', ?, ?, '', ?, NULL, '',
                ?, ?, ?, '{}', ?,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), NULL
         FROM sessions AS session
         WHERE session.id = ? AND session.deleted_at IS NULL",
    )
    .bind(&insert.id)
    .bind(&insert.owner_user_id)
    .bind(&insert.provider)
    .bind(&insert.model)
    .bind(insert.started_at_ms)
    .bind(&insert.memo)
    .bind(&insert.words_json)
    .bind(&insert.speaker_hints_json)
    .bind(&insert.created_at)
    .bind(&insert.session_id)
    .execute(&mut *conn)
    .await?;

    Ok(inserted.rows_affected() > 0)
}

pub async fn clear_incomplete_capture_markers(
    conn: &mut SqliteConnection,
    session_id: &str,
) -> Result<(), sqlx::Error> {
    let prefix = format!("capture_incomplete:{session_id}:");
    sqlx::query(
        "DELETE FROM app_settings
         WHERE substr(id, 1, length(?)) = ?
           AND COALESCE(
               json_extract(
                   CASE WHEN json_valid(value_json) THEN value_json ELSE '{}' END,
                   '$.audioDeletionFailed'
               ),
               0
           ) != 1",
    )
    .bind(&prefix)
    .bind(&prefix)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub async fn mark_session_audio_transcription_complete(
    conn: &mut SqliteConnection,
    session_id: &str,
) -> Result<(), sqlx::Error> {
    let attachment_id = format!("session-audio:{session_id}");
    sqlx::query(
        "UPDATE session_attachments
         SET metadata_json = json_set(
                 CASE WHEN json_valid(metadata_json) THEN metadata_json ELSE '{}' END,
                 '$.transcript_status',
                 'complete'
             ),
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?
           AND session_id = ?
           AND source_type = 'session_audio'
           AND source_id = 'primary'
           AND deleted_at IS NULL",
    )
    .bind(attachment_id)
    .bind(session_id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prepare_schema;
    use anlg_db_core::Db;

    async fn test_db() -> Db {
        let db = Db::connect_memory_plain().await.unwrap();
        prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id)
             VALUES ('session-1', 'workspace-1', 'session-owner')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db
    }

    #[tokio::test]
    async fn loads_pending_deltas_and_inserts_a_replacement_transcript() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO transcripts (
                id, session_id, started_at_ms, words_json, speaker_hints_json
             ) VALUES ('transcript-old', 'session-1', 1000, '[]', '[]')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transcript_live_state (transcript_id, next_sequence)
             VALUES ('transcript-old', 2)",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transcript_live_deltas (id, transcript_id, sequence, delta_json)
             VALUES
                ('delta-1', 'transcript-old', 0, '{\"new_words\":[],\"replaced_ids\":[]}'),
                ('delta-2', 'transcript-old', 1, '{\"new_words\":[],\"replaced_ids\":[]}')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let insert = BatchTranscriptInsert {
            id: "transcript-new".to_string(),
            session_id: "session-1".to_string(),
            owner_user_id: String::new(),
            created_at: "2026-08-15T12:00:00.000Z".to_string(),
            started_at_ms: 2000.0,
            memo: "meeting memo".to_string(),
            provider: "soniox".to_string(),
            model: "stt-batch-v1".to_string(),
            words_json: "[]".to_string(),
            speaker_hints_json: "[]".to_string(),
        };

        let mut transaction = db.pool().begin_with("BEGIN IMMEDIATE").await.unwrap();
        let loaded = load_batch_transcript_by_id(&mut transaction, "transcript-old")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            loaded.pending_delta_jsons,
            [
                "{\"new_words\":[],\"replaced_ids\":[]}",
                "{\"new_words\":[],\"replaced_ids\":[]}"
            ]
        );
        assert!(
            insert_batch_transcript(
                &mut transaction,
                &insert,
                &BatchTranscriptReplace {
                    replace_session: true,
                    replace_transcript_id: None,
                },
            )
            .await
            .unwrap()
        );
        transaction.commit().await.unwrap();

        let rows = sqlx::query(
            "SELECT id, workspace_id, owner_user_id, source, provider, model,
                    language, started_at_ms, ended_at_ms, audio_attachment_id,
                    memo, words_json, speaker_hints_json, metadata_json, created_at,
                    updated_at, deleted_at
             FROM transcripts ORDER BY id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(rows.len(), 2);
        let old = rows
            .iter()
            .find(|row| row.get::<String, _>("id") == "transcript-old")
            .unwrap();
        let new = rows
            .iter()
            .find(|row| row.get::<String, _>("id") == "transcript-new")
            .unwrap();
        assert!(old.get::<Option<String>, _>("deleted_at").is_some());
        assert_eq!(new.get::<String, _>("workspace_id"), "workspace-1");
        assert_eq!(new.get::<String, _>("owner_user_id"), "session-owner");
        assert_eq!(new.get::<String, _>("source"), "batch_transcription");
        assert_eq!(new.get::<String, _>("language"), "");
        assert_eq!(new.get::<i64, _>("started_at_ms"), 2000);
        assert_eq!(new.get::<Option<i64>, _>("ended_at_ms"), None);
        assert_eq!(new.get::<String, _>("audio_attachment_id"), "");
        assert_eq!(new.get::<String, _>("memo"), "meeting memo");
        assert_eq!(new.get::<String, _>("metadata_json"), "{}");
        assert_eq!(new.get::<String, _>("created_at"), insert.created_at);
        assert!(new.get::<String, _>("updated_at").ends_with('Z'));
        assert_eq!(new.get::<Option<String>, _>("deleted_at"), None);
    }
}
