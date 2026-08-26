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

fn candidate<'a>(id: &'a str, expires_at: &'a str) -> NewVoiceprintCandidate<'a> {
    NewVoiceprintCandidate {
        id,
        workspace_id: "workspace-1",
        keyring_key: id,
        model_provider: "pyannote",
        model_version: "wespeaker-onnx-1",
        capture_domain: "system_audio",
        source_session_id: "session-1",
        source_transcript_id: "transcript-1",
        source_attachment_id: "attachment-1",
        source_speaker_label: "Speaker 2",
        speaker_channel: 1,
        speaker_index: Some(2),
        source_start_ms: 4_000,
        source_end_ms: 9_500,
        quality_score: 0.81,
        expires_at,
    }
}

#[tokio::test]
async fn candidates_promote_to_exemplars_after_audio_is_deleted() {
    let db = test_db().await;
    insert_source(&db).await;

    insert_voiceprint_candidate(db.pool(), candidate("candidate-1", "2099-01-01T00:00:00Z"))
        .await
        .unwrap();

    sqlx::raw_sql(
        "UPDATE session_attachments
         SET deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = 'attachment-1'",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let (exemplar, candidate_secret) = promote_voiceprint_candidate(
        db.pool(),
        PromoteVoiceprintCandidate {
            candidate_id: "candidate-1",
            workspace_id: "workspace-1",
            human_id: "human-1",
            confirmation_source: "manual_speaker_assignment",
            label_confidence: 1.0,
        },
    )
    .await
    .unwrap();

    assert_eq!(exemplar.id, "candidate-1");
    assert_eq!(exemplar.human_id, "human-1");
    assert_eq!(exemplar.keyring_scope, VOICEPRINT_KEYRING_SCOPE);
    assert_eq!(exemplar.keyring_key, "candidate-1");
    assert_eq!(exemplar.confirmation_source, "manual_speaker_assignment");
    assert_eq!(exemplar.capture_domain, "system_audio");
    assert_eq!(exemplar.source_start_ms, 4_000);
    assert_eq!(exemplar.quality_score, 0.81);
    assert_eq!(
        candidate_secret,
        VoiceprintSecretRef {
            keyring_scope: VOICEPRINT_CANDIDATE_KEYRING_SCOPE.to_string(),
            keyring_key: "candidate-1".to_string(),
        }
    );

    let remaining = list_active_voiceprint_candidates_for_speaker(
        db.pool(),
        "workspace-1",
        "transcript-1",
        1,
        Some(2),
    )
    .await
    .unwrap();
    assert!(remaining.is_empty());

    assert!(matches!(
        promote_voiceprint_candidate(
            db.pool(),
            PromoteVoiceprintCandidate {
                candidate_id: "candidate-1",
                workspace_id: "workspace-1",
                human_id: "human-1",
                confirmation_source: "manual_speaker_assignment",
                label_confidence: 1.0,
            },
        )
        .await,
        Err(VoiceprintExemplarError::CandidateNotFound)
    ));
}

#[tokio::test]
async fn candidate_insert_requires_live_audio_source() {
    let db = test_db().await;
    insert_source(&db).await;

    let mut wrong_source = candidate("candidate-wrong", "2099-01-01T00:00:00Z");
    wrong_source.source_attachment_id = "different-attachment";
    assert!(matches!(
        insert_voiceprint_candidate(db.pool(), wrong_source).await,
        Err(VoiceprintExemplarError::SourceNotFound)
    ));

    let mut bad_span = candidate("candidate-span", "2099-01-01T00:00:00Z");
    bad_span.source_end_ms = bad_span.source_start_ms;
    assert!(matches!(
        insert_voiceprint_candidate(db.pool(), bad_span).await,
        Err(VoiceprintExemplarError::InvalidField("source span"))
    ));
}

#[tokio::test]
async fn candidates_match_on_channel_and_nullable_speaker_index() {
    let db = test_db().await;
    insert_source(&db).await;

    insert_voiceprint_candidate(
        db.pool(),
        candidate("candidate-idx", "2099-01-01T00:00:00Z"),
    )
    .await
    .unwrap();
    let mut no_index = candidate("candidate-noidx", "2099-01-01T00:00:00Z");
    no_index.speaker_index = None;
    no_index.speaker_channel = 0;
    insert_voiceprint_candidate(db.pool(), no_index)
        .await
        .unwrap();

    let indexed = list_active_voiceprint_candidates_for_speaker(
        db.pool(),
        "workspace-1",
        "transcript-1",
        1,
        Some(2),
    )
    .await
    .unwrap();
    assert_eq!(indexed.len(), 1);
    assert_eq!(indexed[0].id, "candidate-idx");

    let unindexed = list_active_voiceprint_candidates_for_speaker(
        db.pool(),
        "workspace-1",
        "transcript-1",
        0,
        None,
    )
    .await
    .unwrap();
    assert_eq!(unindexed.len(), 1);
    assert_eq!(unindexed[0].id, "candidate-noidx");

    let all =
        list_active_voiceprint_candidates_for_transcript(db.pool(), "workspace-1", "transcript-1")
            .await
            .unwrap();
    assert_eq!(
        all.iter()
            .map(|candidate| candidate.id.as_str())
            .collect::<Vec<_>>(),
        vec!["candidate-noidx", "candidate-idx"]
    );
}

#[tokio::test]
async fn expired_candidates_tombstone_and_purge_with_secret_refs() {
    let db = test_db().await;
    insert_source(&db).await;

    insert_voiceprint_candidate(
        db.pool(),
        candidate("candidate-old", "2026-01-01T00:00:00Z"),
    )
    .await
    .unwrap();
    insert_voiceprint_candidate(
        db.pool(),
        candidate("candidate-new", "2099-01-01T00:00:00Z"),
    )
    .await
    .unwrap();

    let expired = tombstone_expired_voiceprint_candidates(db.pool(), "2026-06-01T00:00:00Z")
        .await
        .unwrap();
    assert_eq!(
        expired,
        vec![VoiceprintSecretRef {
            keyring_scope: VOICEPRINT_CANDIDATE_KEYRING_SCOPE.to_string(),
            keyring_key: "candidate-old".to_string(),
        }]
    );

    assert!(
        purge_tombstoned_voiceprint_candidate(db.pool(), "workspace-1", "candidate-old")
            .await
            .unwrap()
    );
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM voiceprint_candidates WHERE deleted_at IS NULL")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(remaining, 1);

    assert!(
        !cloudsync_table_registry()
            .iter()
            .any(|table| table.table_name == "voiceprint_candidates")
    );
    assert!(!E2EE_DOMAIN_TABLES.contains(&"voiceprint_candidates"));
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM pragma_table_info('voiceprint_candidates') ORDER BY cid",
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
