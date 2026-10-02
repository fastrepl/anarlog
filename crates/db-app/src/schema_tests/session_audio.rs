use super::*;

async fn insert_sessions(db: &Db) {
    sqlx::raw_sql(
        r#"INSERT INTO sessions (id, created_at) VALUES
             ('idle', '2026-01-01T00:00:00.000Z'),
             ('processing', '2026-01-01T00:00:00.000Z'),
             ('capturing', '2026-01-01T00:00:00.000Z'),
             ('no-words', 'not a date');

           INSERT INTO transcripts (id, session_id, words_json) VALUES
             ('t-idle', 'idle', '[{"text":"hi"}]'),
             ('t-processing', 'processing', '[{"text":"hi"}]'),
             ('t-no-words', 'no-words', '[]');

           INSERT INTO session_attachments (
             id, session_id, source_type, source_id, metadata_json, deleted_at
           ) VALUES
             ('session-audio:processing', 'processing', 'session_audio', 'primary',
              '{"transcript_status":"processing"}', NULL),
             ('session-audio:idle', 'idle', 'session_audio', 'primary', '{}',
              '2026-01-02T00:00:00.000Z');

           INSERT INTO app_settings (id, value_json)
           VALUES ('capture_lifecycle_pending:capturing', '{}');"#,
    )
    .execute(db.pool())
    .await
    .unwrap();
}

#[tokio::test]
async fn session_audio_retention_skips_sessions_still_owned_by_capture_or_transcription() {
    let db = test_db().await;
    let pool = db.pool();
    insert_sessions(&db).await;

    assert_eq!(
        list_session_audio_retention_candidates(pool).await.unwrap(),
        vec![
            SessionAudioRetentionCandidate {
                session_id: "idle".into(),
                created_at_ms: Some(1_767_225_600_000),
                has_words: true,
            },
            SessionAudioRetentionCandidate {
                session_id: "no-words".into(),
                created_at_ms: None,
                has_words: false,
            },
        ]
    );
    assert!(
        get_session_audio_retention_candidate(pool, "idle")
            .await
            .unwrap()
            .is_some()
    );
    for claimed in ["processing", "capturing"] {
        assert_eq!(
            get_session_audio_retention_candidate(pool, claimed)
                .await
                .unwrap(),
            None
        );
    }
    assert!(session_audio_is_processed(pool, "idle").await.unwrap());
    assert!(
        !session_audio_is_processed(pool, "processing")
            .await
            .unwrap()
    );
    assert!(!session_audio_is_processed(pool, "no-words").await.unwrap());
}

#[tokio::test]
async fn logically_deleted_session_audio_is_listed_until_marked_absent() {
    let db = test_db().await;
    let pool = db.pool();
    insert_sessions(&db).await;

    assert_eq!(
        list_logically_deleted_session_audio(pool).await.unwrap(),
        vec!["idle".to_string()]
    );
    assert!(
        session_audio_is_logically_deleted(pool, "idle")
            .await
            .unwrap()
    );
    assert!(
        !session_audio_is_logically_deleted(pool, "processing")
            .await
            .unwrap()
    );

    mark_session_audio_absent(pool, "idle").await.unwrap();

    assert!(
        list_logically_deleted_session_audio(pool)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        !session_audio_is_logically_deleted(pool, "idle")
            .await
            .unwrap()
    );
}
