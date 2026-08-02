use super::*;

async fn insert_source(db: &Db) {
    sqlx::raw_sql(
        "INSERT INTO humans (id, workspace_id, name)
         VALUES ('human-1', 'workspace-1', 'Ada Lovelace');

         INSERT INTO sessions (id, workspace_id, title)
         VALUES ('session-1', 'workspace-1', 'Architecture review');

         INSERT INTO session_attachments (
           id, workspace_id, session_id, filename, content_type
         ) VALUES (
           'attachment-1', 'workspace-1', 'session-1', 'meeting.wav', 'audio/wav'
         );

         INSERT INTO transcripts (
           id, workspace_id, session_id, audio_attachment_id
         ) VALUES (
           'transcript-1', 'workspace-1', 'session-1', 'attachment-1'
         );

         INSERT INTO session_participants (
           id, workspace_id, session_id, human_id, display_name, source
         ) VALUES (
           'participant-1', 'workspace-1', 'session-1', 'human-1',
           'Ada Lovelace', 'calendar'
         );",
    )
    .execute(db.pool())
    .await
    .unwrap();
}

fn exemplar<'a>(
    id: &'a str,
    keyring_key: &'a str,
    capture_domain: &'a str,
) -> NewVoiceprintExemplar<'a> {
    NewVoiceprintExemplar {
        id,
        workspace_id: "workspace-1",
        human_id: "human-1",
        keyring_key,
        model_provider: "pyannote",
        model_version: "precision-2",
        capture_domain,
        confirmation_source: "accessibility_active_speaker",
        source_session_id: "session-1",
        source_transcript_id: "transcript-1",
        source_attachment_id: "attachment-1",
        source_speaker_label: "Ada Lovelace",
        source_start_ms: 1_000,
        source_end_ms: 8_000,
        quality_score: 0.94,
        label_confidence: 0.99,
    }
}

#[tokio::test]
async fn confirmed_voiceprints_store_only_local_keyring_coordinates() {
    let db = test_db().await;
    insert_source(&db).await;

    let inserted = insert_voiceprint_exemplar(
        db.pool(),
        exemplar("voiceprint-1", "voiceprint-1", "conference_remote:zoom"),
    )
    .await
    .unwrap();

    assert_eq!(inserted.keyring_scope, VOICEPRINT_KEYRING_SCOPE);
    assert_eq!(inserted.keyring_key, "voiceprint-1");
    assert_eq!(inserted.sync_scope, VOICEPRINT_SYNC_SCOPE);
    assert_eq!(inserted.model_version, "precision-2");
    assert_eq!(inserted.source_speaker_label, "Ada Lovelace");
    assert_eq!(inserted.quality_score, 0.94);
    assert!(
        !cloudsync_table_registry()
            .iter()
            .any(|table| table.table_name == "voiceprint_exemplars")
    );
    assert!(!E2EE_DOMAIN_TABLES.contains(&"voiceprint_exemplars"));

    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM pragma_table_info('voiceprint_exemplars') ORDER BY cid",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert!(
        !columns.iter().any(|column| {
            matches!(column.as_str(), "voiceprint" | "embedding" | "secret_value")
        })
    );
}

#[tokio::test]
async fn voiceprints_retain_multiple_capture_domains_for_one_human() {
    let db = test_db().await;
    insert_source(&db).await;

    insert_voiceprint_exemplar(
        db.pool(),
        exemplar(
            "voiceprint-remote",
            "voiceprint-remote",
            "conference_remote:zoom",
        ),
    )
    .await
    .unwrap();
    insert_voiceprint_exemplar(
        db.pool(),
        exemplar("voiceprint-room", "voiceprint-room", "in_person:direct_mic"),
    )
    .await
    .unwrap();

    let exemplars = list_active_voiceprint_exemplars_for_human(db.pool(), "workspace-1", "human-1")
        .await
        .unwrap();
    assert_eq!(exemplars.len(), 2);
    assert_eq!(
        exemplars
            .iter()
            .map(|exemplar| exemplar.capture_domain.as_str())
            .collect::<Vec<_>>(),
        vec!["conference_remote:zoom", "in_person:direct_mic"]
    );
}

#[tokio::test]
async fn voiceprint_insert_requires_confirmed_provenance_and_matching_audio() {
    let db = test_db().await;
    insert_source(&db).await;

    let mut unconfirmed = exemplar(
        "voiceprint-unconfirmed",
        "voiceprint-unconfirmed",
        "conference_remote:zoom",
    );
    unconfirmed.confirmation_source = "automatic_suggestion";
    assert!(matches!(
        insert_voiceprint_exemplar(db.pool(), unconfirmed).await,
        Err(VoiceprintExemplarError::InvalidField("confirmation source"))
    ));

    let mut wrong_source = exemplar(
        "voiceprint-wrong-source",
        "voiceprint-wrong-source",
        "conference_remote:zoom",
    );
    wrong_source.source_attachment_id = "different-attachment";
    assert!(matches!(
        insert_voiceprint_exemplar(db.pool(), wrong_source).await,
        Err(VoiceprintExemplarError::SourceNotFound)
    ));

    let mut invalid_quality = exemplar(
        "voiceprint-invalid-quality",
        "voiceprint-invalid-quality",
        "conference_remote:zoom",
    );
    invalid_quality.quality_score = f64::NAN;
    assert!(matches!(
        insert_voiceprint_exemplar(db.pool(), invalid_quality).await,
        Err(VoiceprintExemplarError::InvalidField("quality score"))
    ));
}

#[tokio::test]
async fn reset_returns_keyring_secrets_before_metadata_is_purged() {
    let db = test_db().await;
    insert_source(&db).await;
    for (id, domain) in [
        ("voiceprint-remote", "conference_remote:zoom"),
        ("voiceprint-room", "in_person:direct_mic"),
    ] {
        insert_voiceprint_exemplar(db.pool(), exemplar(id, id, domain))
            .await
            .unwrap();
    }

    let secret_refs = tombstone_voiceprint_exemplars_for_human(db.pool(), "workspace-1", "human-1")
        .await
        .unwrap();
    assert_eq!(
        secret_refs,
        vec![
            VoiceprintSecretRef {
                keyring_scope: VOICEPRINT_KEYRING_SCOPE.to_string(),
                keyring_key: "voiceprint-remote".to_string(),
            },
            VoiceprintSecretRef {
                keyring_scope: VOICEPRINT_KEYRING_SCOPE.to_string(),
                keyring_key: "voiceprint-room".to_string(),
            },
        ]
    );
    assert!(
        list_active_voiceprint_exemplars_for_human(db.pool(), "workspace-1", "human-1",)
            .await
            .unwrap()
            .is_empty()
    );

    assert!(
        purge_tombstoned_voiceprint_exemplar(db.pool(), "workspace-1", "voiceprint-remote",)
            .await
            .unwrap()
    );
    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM voiceprint_exemplars WHERE workspace_id = 'workspace-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(remaining, 1);
}
