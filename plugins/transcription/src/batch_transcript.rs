use anlg_db_app::{
    BatchTranscriptInsert, BatchTranscriptReplace, BatchTranscriptRow,
    clear_incomplete_capture_markers, insert_batch_transcript, load_batch_transcript_by_id,
    load_session_batch_transcripts, mark_session_audio_transcription_complete,
};
use anlg_transcript::{
    BatchRefinementOutcome, BatchRefinementRequest, BatchRefinementSource,
    BatchTranscriptPromotion, StoredLiveTranscriptDelta, StoredSpeakerHint, StoredTranscriptWord,
    materialize_live_transcript, parse_stored_speaker_hints, parse_stored_transcript_words,
    refine_batch_transcript, serialize_batch_transcript_hints, serialize_batch_transcript_words,
};
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
pub struct SaveBatchTranscriptRequest {
    pub session_id: String,
    pub transcript_id: Option<String>,
    pub owner_user_id: String,
    pub created_at: String,
    pub started_at: f64,
    pub memo: String,
    pub provider: String,
    pub model: String,
    pub words: Vec<StoredTranscriptWord>,
    pub hints: Vec<StoredSpeakerHint>,
    pub promotion: BatchTranscriptPromotion,
    pub mark_audio_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, specta::Type)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SaveBatchTranscriptOutcome {
    Saved { transcript_id: Option<String> },
    EmptyCurrentCapture,
    Truncated,
}

pub async fn save_batch_transcript(
    pool: &SqlitePool,
    request: SaveBatchTranscriptRequest,
) -> Result<SaveBatchTranscriptOutcome, String> {
    let SaveBatchTranscriptRequest {
        session_id,
        transcript_id,
        owner_user_id,
        created_at,
        started_at,
        memo,
        provider,
        model,
        words,
        hints,
        promotion,
        mark_audio_complete,
    } = request;
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    let source_rows = match &promotion {
        BatchTranscriptPromotion::WholeSession => {
            load_session_batch_transcripts(&mut transaction, &session_id)
                .await
                .map_err(|error| error.to_string())?
        }
        BatchTranscriptPromotion::CurrentCapture {
            replace_transcript_id: Some(replace_transcript_id),
            ..
        } => load_batch_transcript_by_id(&mut transaction, replace_transcript_id)
            .await
            .map_err(|error| error.to_string())?
            .into_iter()
            .collect(),
        BatchTranscriptPromotion::PreserveExisting
        | BatchTranscriptPromotion::CurrentCapture {
            replace_transcript_id: None,
            ..
        } => Vec::new(),
    };
    let previous_transcripts = source_rows
        .into_iter()
        .map(materialize_source_row)
        .collect::<Vec<_>>();

    let refinement = refine_batch_transcript(BatchRefinementRequest {
        words,
        hints,
        promotion,
        previous_transcripts,
    });

    let (words, speaker_hints, replace_session, replace_transcript_id, refined_started_at) =
        match refinement {
            BatchRefinementOutcome::Ready {
                words,
                speaker_hints,
                replace_session,
                replace_transcript_id,
                started_at,
            } => (
                words,
                speaker_hints,
                replace_session,
                replace_transcript_id,
                started_at,
            ),
            BatchRefinementOutcome::EmptyCurrentCapture => {
                transaction
                    .rollback()
                    .await
                    .map_err(|error| error.to_string())?;
                return Ok(SaveBatchTranscriptOutcome::EmptyCurrentCapture);
            }
            BatchRefinementOutcome::Truncated => {
                transaction
                    .rollback()
                    .await
                    .map_err(|error| error.to_string())?;
                return Ok(SaveBatchTranscriptOutcome::Truncated);
            }
        };

    let inserted_transcript_id =
        if let Some(transcript_id) = transcript_id.filter(|_| !words.is_empty()) {
            let insert = BatchTranscriptInsert {
                id: transcript_id.clone(),
                session_id: session_id.clone(),
                owner_user_id,
                created_at,
                started_at_ms: refined_started_at.unwrap_or(started_at),
                memo,
                provider,
                model,
                words_json: serialize_batch_transcript_words(&words)
                    .map_err(|error| error.to_string())?,
                speaker_hints_json: serialize_batch_transcript_hints(&speaker_hints)
                    .map_err(|error| error.to_string())?,
            };
            let inserted = insert_batch_transcript(
                &mut transaction,
                &insert,
                &BatchTranscriptReplace {
                    replace_session,
                    replace_transcript_id,
                },
            )
            .await
            .map_err(|error| error.to_string())?;
            inserted.then_some(transcript_id)
        } else {
            None
        };

    if replace_session {
        clear_incomplete_capture_markers(&mut transaction, &session_id)
            .await
            .map_err(|error| error.to_string())?;
    }
    if mark_audio_complete {
        mark_session_audio_transcription_complete(&mut transaction, &session_id)
            .await
            .map_err(|error| error.to_string())?;
    }
    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())?;

    Ok(SaveBatchTranscriptOutcome::Saved {
        transcript_id: inserted_transcript_id,
    })
}

fn materialize_source_row(row: BatchTranscriptRow) -> BatchRefinementSource {
    let words = parse_stored_transcript_words(&row.words_json);
    let hints = parse_stored_speaker_hints(&row.speaker_hints_json);
    let deltas: Vec<StoredLiveTranscriptDelta> = row
        .pending_delta_jsons
        .iter()
        .filter_map(|delta_json| serde_json::from_str(delta_json).ok())
        .collect();
    let (words, speaker_hints) = materialize_live_transcript(words, hints, &deltas);

    BatchRefinementSource {
        id: row.id,
        started_at: row.started_at_ms as f64,
        words,
        speaker_hints,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_db_core::Db;
    use anlg_transcript::WordState;
    use serde_json::Value;
    use sqlx::Row;

    async fn test_db() -> Db {
        let db = Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id)
             VALUES
                ('session-1', 'workspace-1', 'session-owner'),
                ('session-2', 'workspace-2', 'other-owner')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_attachments (
                id, workspace_id, session_id, source_type, source_id,
                metadata_json
             ) VALUES (
                'session-audio:session-1', 'workspace-1', 'session-1',
                'session_audio', 'primary',
                '{\"transcript_status\":\"pending\"}'
             )",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db
    }

    fn stored_word(id: &str, text: String, start_ms: f64, end_ms: f64) -> StoredTranscriptWord {
        StoredTranscriptWord {
            id: id.to_string(),
            text: Some(text),
            start_ms: Some(start_ms),
            end_ms: Some(end_ms),
            channel: Some(0.0),
            speaker: None,
            metadata: None,
        }
    }

    fn request(promotion: BatchTranscriptPromotion) -> SaveBatchTranscriptRequest {
        SaveBatchTranscriptRequest {
            session_id: "session-1".to_string(),
            transcript_id: Some("transcript-new".to_string()),
            owner_user_id: String::new(),
            created_at: "2026-08-15T12:00:00.000Z".to_string(),
            started_at: 9000.0,
            memo: "new memo".to_string(),
            provider: "soniox".to_string(),
            model: "batch-v1".to_string(),
            words: vec![stored_word(
                "new-word",
                "replacement".to_string(),
                100.0,
                200.0,
            )],
            hints: vec![],
            promotion,
            mark_audio_complete: true,
        }
    }

    async fn insert_transcript(
        db: &Db,
        id: &str,
        session_id: &str,
        started_at_ms: i64,
        words: &[StoredTranscriptWord],
    ) {
        let words_json = serde_json::to_string(words).unwrap();
        sqlx::query(
            "INSERT INTO transcripts (
                id, session_id, started_at_ms, words_json, speaker_hints_json
             ) VALUES (?, ?, ?, ?, '[]')",
        )
        .bind(id)
        .bind(session_id)
        .bind(started_at_ms)
        .bind(words_json)
        .execute(db.pool())
        .await
        .unwrap();
    }

    async fn insert_capture_marker(db: &Db, id: &str, value_json: &str) {
        sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
            .bind(id)
            .bind(value_json)
            .execute(db.pool())
            .await
            .unwrap();
    }

    async fn audio_metadata(db: &Db) -> String {
        sqlx::query_scalar(
            "SELECT metadata_json FROM session_attachments
             WHERE id = 'session-audio:session-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn whole_session_replacement_updates_transcripts_markers_and_audio_atomically() {
        let db = test_db().await;
        insert_transcript(&db, "old-1", "session-1", 1000, &[]).await;
        insert_transcript(&db, "old-2", "session-1", 2000, &[]).await;
        insert_transcript(&db, "other-session", "session-2", 3000, &[]).await;
        insert_capture_marker(
            &db,
            "capture_incomplete:session-1:old-1",
            r#"{"audioDeletionFailed":false}"#,
        )
        .await;
        insert_capture_marker(
            &db,
            "capture_incomplete:session-1:old-2",
            r#"{"audioDeletionFailed":1}"#,
        )
        .await;
        insert_capture_marker(
            &db,
            "capture_incomplete:session-2:old",
            r#"{"audioDeletionFailed":false}"#,
        )
        .await;

        let outcome =
            save_batch_transcript(db.pool(), request(BatchTranscriptPromotion::WholeSession))
                .await
                .unwrap();

        assert_eq!(
            outcome,
            SaveBatchTranscriptOutcome::Saved {
                transcript_id: Some("transcript-new".to_string())
            }
        );
        let transcripts = sqlx::query(
            "SELECT id, workspace_id, owner_user_id, source, provider, model,
                    language, started_at_ms, ended_at_ms, audio_attachment_id, memo,
                    words_json, speaker_hints_json, metadata_json, created_at, deleted_at
             FROM transcripts WHERE session_id = 'session-1' ORDER BY id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(transcripts.len(), 3);
        assert!(
            transcripts[0]
                .get::<Option<String>, _>("deleted_at")
                .is_some()
        );
        assert!(
            transcripts[1]
                .get::<Option<String>, _>("deleted_at")
                .is_some()
        );
        let inserted = transcripts
            .iter()
            .find(|row| row.get::<String, _>("id") == "transcript-new")
            .unwrap();
        assert_eq!(inserted.get::<String, _>("workspace_id"), "workspace-1");
        assert_eq!(inserted.get::<String, _>("owner_user_id"), "session-owner");
        assert_eq!(inserted.get::<String, _>("source"), "batch_transcription");
        assert_eq!(inserted.get::<String, _>("provider"), "soniox");
        assert_eq!(inserted.get::<String, _>("model"), "batch-v1");
        assert_eq!(inserted.get::<String, _>("language"), "");
        assert_eq!(inserted.get::<i64, _>("started_at_ms"), 9000);
        assert_eq!(inserted.get::<Option<i64>, _>("ended_at_ms"), None);
        assert_eq!(inserted.get::<String, _>("audio_attachment_id"), "");
        assert_eq!(inserted.get::<String, _>("memo"), "new memo");
        assert_eq!(
            inserted.get::<String, _>("words_json"),
            r#"[{"id":"new-word","text":"replacement","start_ms":100,"end_ms":200,"channel":0}]"#
        );
        assert_eq!(inserted.get::<String, _>("speaker_hints_json"), "[]");
        assert_eq!(inserted.get::<String, _>("metadata_json"), "{}");
        assert_eq!(
            inserted.get::<String, _>("created_at"),
            "2026-08-15T12:00:00.000Z"
        );
        assert_eq!(inserted.get::<Option<String>, _>("deleted_at"), None);

        let markers: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM app_settings
             WHERE substr(id, 1, length('capture_incomplete:')) = 'capture_incomplete:'
             ORDER BY id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(
            markers,
            [
                "capture_incomplete:session-1:old-2",
                "capture_incomplete:session-2:old"
            ]
        );
        assert_eq!(
            serde_json::from_str::<Value>(&audio_metadata(&db).await).unwrap()["transcript_status"],
            "complete"
        );
    }

    #[tokio::test]
    async fn current_capture_replacement_keeps_assignment_without_hint_id() {
        let db = test_db().await;
        let mut source_word = stored_word("source-word", "hello".to_string(), 100.0, 200.0);
        source_word.channel = Some(1.0);
        insert_transcript(&db, "source", "session-1", 1000, &[source_word]).await;
        let source_hints = serde_json::json!([
            {
                "word_id": "source-word",
                "type": "user_speaker_assignment",
                "value": r#"{"human_id":"alice","scope":"speaker","channel":1,"speaker_index":0}"#
            },
            {
                "id": "source-word:provider_speaker_index",
                "word_id": "source-word",
                "type": "provider_speaker_index",
                "value": r#"{"channel":1,"speaker_index":0}"#
            }
        ]);
        let parsed_hints = parse_stored_speaker_hints(&source_hints.to_string());
        assert_eq!(parsed_hints[0].id, "source-word:user_speaker_assignment");
        sqlx::query("UPDATE transcripts SET speaker_hints_json = ? WHERE id = 'source'")
            .bind(source_hints.to_string())
            .execute(db.pool())
            .await
            .unwrap();

        let mut batch_word = stored_word("new-word", "hello".to_string(), 100.0, 200.0);
        batch_word.channel = Some(1.0);
        let mut request = request(BatchTranscriptPromotion::CurrentCapture {
            audio_offset_ms: 0.0,
            replace_transcript_id: Some("source".to_string()),
            started_at: 1000.0,
        });
        request.started_at = 1000.0;
        request.words = vec![batch_word];
        request.hints = vec![StoredSpeakerHint {
            id: "new-word:provider_speaker_index".to_string(),
            word_id: Some("new-word".to_string()),
            hint_type: "provider_speaker_index".to_string(),
            value: Value::String(r#"{"channel":1,"speaker_index":7}"#.to_string()),
        }];
        request.mark_audio_complete = false;

        assert_eq!(
            save_batch_transcript(db.pool(), request).await.unwrap(),
            SaveBatchTranscriptOutcome::Saved {
                transcript_id: Some("transcript-new".to_string())
            }
        );
        let hints_json: String = sqlx::query_scalar(
            "SELECT speaker_hints_json FROM transcripts WHERE id = 'transcript-new'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        let saved_hints = serde_json::from_str::<Value>(&hints_json).unwrap();
        let assignment = saved_hints.as_array().unwrap().iter().find(|hint| {
            hint["type"] == "user_speaker_assignment" && hint["word_id"] == "new-word"
        });
        assert!(
            assignment.is_some(),
            "expected assignment on new-word in saved hints: {saved_hints}"
        );
        let assignment_value = match &assignment.unwrap()["value"] {
            Value::String(value) => serde_json::from_str::<Value>(value).unwrap(),
            value => value.clone(),
        };
        assert_eq!(assignment_value["human_id"], "alice");
    }

    #[tokio::test]
    async fn truncated_refinement_leaves_transcripts_and_audio_unchanged() {
        let db = test_db().await;
        insert_transcript(
            &db,
            "saved",
            "session-1",
            1000,
            &[stored_word("old-word", "a".repeat(500), 0.0, 500.0)],
        )
        .await;
        let mut request = request(BatchTranscriptPromotion::WholeSession);
        request.words = vec![stored_word("replacement", "a".repeat(200), 0.0, 200.0)];

        assert_eq!(
            save_batch_transcript(db.pool(), request).await.unwrap(),
            SaveBatchTranscriptOutcome::Truncated
        );
        let saved =
            sqlx::query("SELECT deleted_at, words_json FROM transcripts WHERE id = 'saved'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(saved.get::<Option<String>, _>("deleted_at"), None);
        assert!(
            saved
                .get::<String, _>("words_json")
                .contains(&"a".repeat(500))
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM transcripts WHERE id = 'transcript-new'"
            )
            .fetch_one(db.pool())
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            serde_json::from_str::<Value>(&audio_metadata(&db).await).unwrap()["transcript_status"],
            "pending"
        );
    }

    #[tokio::test]
    async fn pending_live_deltas_are_materialized_before_truncation_refinement() {
        let db = test_db().await;
        insert_transcript(&db, "saved", "session-1", 1000, &[]).await;
        sqlx::query(
            "INSERT INTO transcript_live_state (transcript_id, next_sequence)
             VALUES ('saved', 1)",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let delta_json = serde_json::json!({
            "new_words": [{
                "id": "pending-old-word",
                "text": "a".repeat(500),
                "start_ms": 0,
                "end_ms": 500,
                "channel": 0,
                "state": "final"
            }],
            "replaced_ids": [],
            "partials": []
        })
        .to_string();
        sqlx::query(
            "INSERT INTO transcript_live_deltas (id, transcript_id, sequence, delta_json)
             VALUES ('pending-delta', 'saved', 0, ?)",
        )
        .bind(delta_json)
        .execute(db.pool())
        .await
        .unwrap();
        let mut request = request(BatchTranscriptPromotion::WholeSession);
        request.words = vec![stored_word("replacement", "a".repeat(200), 0.0, 200.0)];

        assert_eq!(
            save_batch_transcript(db.pool(), request).await.unwrap(),
            SaveBatchTranscriptOutcome::Truncated
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM transcripts WHERE id = 'transcript-new'"
            )
            .fetch_one(db.pool())
            .await
            .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM transcript_live_deltas WHERE transcript_id = 'saved'"
            )
            .fetch_one(db.pool())
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            serde_json::from_str::<Value>(&audio_metadata(&db).await).unwrap()["transcript_status"],
            "pending"
        );
    }

    #[tokio::test]
    async fn current_capture_replacement_soft_deletes_only_the_requested_transcript() {
        let db = test_db().await;
        insert_transcript(
            &db,
            "replace-this",
            "session-1",
            1000,
            &[stored_word("old-word", "old".to_string(), 0.0, 100.0)],
        )
        .await;
        insert_transcript(&db, "keep-this", "session-1", 2000, &[]).await;
        let mut request = request(BatchTranscriptPromotion::CurrentCapture {
            audio_offset_ms: 60_000.0,
            replace_transcript_id: Some("replace-this".to_string()),
            started_at: 7000.0,
        });
        request.words = vec![stored_word(
            "new-word",
            "replacement".to_string(),
            60_100.0,
            60_200.0,
        )];
        request.mark_audio_complete = false;

        assert_eq!(
            save_batch_transcript(db.pool(), request).await.unwrap(),
            SaveBatchTranscriptOutcome::Saved {
                transcript_id: Some("transcript-new".to_string())
            }
        );
        let replaced: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM transcripts WHERE id = 'replace-this'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let kept: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM transcripts WHERE id = 'keep-this'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let inserted_started_at: i64 =
            sqlx::query_scalar("SELECT started_at_ms FROM transcripts WHERE id = 'transcript-new'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(replaced.is_some());
        assert_eq!(kept, None);
        assert_eq!(inserted_started_at, 7000);
        assert_eq!(
            serde_json::from_str::<Value>(&audio_metadata(&db).await).unwrap()["transcript_status"],
            "pending"
        );
    }

    #[tokio::test]
    async fn current_capture_replacement_keeps_speaker_identity_from_pending_live_words() {
        let db = test_db().await;
        insert_transcript(&db, "replace-this", "session-1", 1000, &[]).await;
        let source_hints = vec![StoredSpeakerHint {
            id: "pending-word:user_speaker_assignment".to_string(),
            word_id: Some("pending-word".to_string()),
            hint_type: "user_speaker_assignment".to_string(),
            value: serde_json::Value::String(
                r#"{"human_id":"alice","scope":"speaker","channel":1,"speaker_index":0}"#
                    .to_string(),
            ),
        }];
        sqlx::query("UPDATE transcripts SET speaker_hints_json = ? WHERE id = 'replace-this'")
            .bind(serde_json::to_string(&source_hints).unwrap())
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO transcript_live_state (transcript_id, next_sequence)
             VALUES ('replace-this', 1)",
        )
        .execute(db.pool())
        .await
        .unwrap();
        let delta_json = serde_json::json!({
            "new_words": [{
                "id": "pending-word",
                "text": "hello",
                "start_ms": 0,
                "end_ms": 500,
                "channel": 1,
                "speaker_index": 0,
                "state": "final"
            }],
            "replaced_ids": [],
            "partials": []
        })
        .to_string();
        sqlx::query(
            "INSERT INTO transcript_live_deltas (id, transcript_id, sequence, delta_json)
             VALUES ('pending-delta', 'replace-this', 0, ?)",
        )
        .bind(delta_json)
        .execute(db.pool())
        .await
        .unwrap();

        let mut batch_word = stored_word("new-word", "hello".to_string(), 0.0, 500.0);
        batch_word.channel = Some(1.0);
        let mut request = request(BatchTranscriptPromotion::CurrentCapture {
            audio_offset_ms: 0.0,
            replace_transcript_id: Some("replace-this".to_string()),
            started_at: 1000.0,
        });
        request.started_at = 1000.0;
        request.words = vec![batch_word];
        request.hints = vec![StoredSpeakerHint {
            id: "new-word:provider_speaker_index".to_string(),
            word_id: Some("new-word".to_string()),
            hint_type: "provider_speaker_index".to_string(),
            value: serde_json::Value::String(r#"{"channel":1,"speaker_index":7}"#.to_string()),
        }];
        request.mark_audio_complete = false;

        assert_eq!(
            save_batch_transcript(db.pool(), request).await.unwrap(),
            SaveBatchTranscriptOutcome::Saved {
                transcript_id: Some("transcript-new".to_string())
            }
        );
        let replaced = sqlx::query("SELECT deleted_at FROM transcripts WHERE id = 'replace-this'")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert!(replaced.get::<Option<String>, _>("deleted_at").is_some());

        let hints_json: String = sqlx::query_scalar(
            "SELECT speaker_hints_json FROM transcripts WHERE id = 'transcript-new'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        let saved_hints = serde_json::from_str::<Value>(&hints_json).unwrap();
        let assignment = saved_hints.as_array().unwrap().iter().find(|hint| {
            hint["type"] == "user_speaker_assignment" && hint["word_id"] == "new-word"
        });
        assert!(
            assignment.is_some(),
            "expected assignment on new-word in saved hints: {saved_hints}"
        );
        let assignment_value = match &assignment.unwrap()["value"] {
            Value::String(value) => serde_json::from_str::<Value>(value).unwrap(),
            value => value.clone(),
        };
        assert_eq!(assignment_value["human_id"], "alice");
    }

    #[tokio::test]
    async fn current_capture_without_remaining_words_rolls_back_without_finalizing_audio() {
        let db = test_db().await;
        let mut request = request(BatchTranscriptPromotion::CurrentCapture {
            audio_offset_ms: 60_000.0,
            replace_transcript_id: None,
            started_at: 7000.0,
        });
        request.words = vec![stored_word("old", "old".to_string(), 0.0, 100.0)];

        assert_eq!(
            save_batch_transcript(db.pool(), request).await.unwrap(),
            SaveBatchTranscriptOutcome::EmptyCurrentCapture
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM transcripts")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            serde_json::from_str::<Value>(&audio_metadata(&db).await).unwrap()["transcript_status"],
            "pending"
        );
    }

    #[test]
    fn stored_live_delta_ignores_partials() {
        let delta: StoredLiveTranscriptDelta = serde_json::from_str(
            r#"{"new_words":[{"id":"word","text":"word","start_ms":0,"end_ms":1,"channel":0,"state":"final"}],"replaced_ids":[],"partials":[{"text":"partial"}]}"#,
        )
        .unwrap();
        assert_eq!(delta.new_words.len(), 1);
        assert_eq!(delta.new_words[0].id, "word");
        assert_eq!(delta.new_words[0].state, WordState::Final);
    }
}
