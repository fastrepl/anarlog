use super::*;

#[tokio::test]
async fn local_edits_advance_beyond_witnessed_remote_versions() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    let key = &workspace_keys["workspace-a"];
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Base')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();

    let remote_title = key
        .seal_field(
            "workspace-a",
            "sessions",
            "session-1",
            "title",
            "ffffffffffffffffffffffffffffffff",
            8,
            false,
            json!("Remote"),
        )
        .unwrap();
    let remote_tombstone = key
        .seal_field(
            "workspace-a",
            "sessions",
            "session-1",
            ROW_MANIFEST_FIELD,
            "ffffffffffffffffffffffffffffffff",
            8,
            true,
            Value::Null,
        )
        .unwrap();
    merge_e2ee_witness_events(
        db.pool(),
        key,
        "workspace-a",
        &[
            E2eeWitnessEvent {
                sequence: 1,
                record_id: remote_title.record_id,
                workspace_id: "workspace-a".to_string(),
                payload_hash: anlg_e2ee::payload_hash(&remote_title.payload),
                payload: remote_title.payload,
            },
            E2eeWitnessEvent {
                sequence: 2,
                record_id: remote_tombstone.record_id,
                workspace_id: "workspace-a".to_string(),
                payload_hash: anlg_e2ee::payload_hash(&remote_tombstone.payload),
                payload: remote_tombstone.payload,
            },
        ],
    )
    .await
    .unwrap();

    sqlx::query("UPDATE sessions SET title = 'Local' WHERE id = 'session-1'")
        .execute(db.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();

    let title_id = key.blind_field_id("sessions", "session-1", "title");
    let manifest_id = key.blind_field_id("sessions", "session-1", ROW_MANIFEST_FIELD);
    let title_payload: String = sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
        .bind(&title_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
    let manifest_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&manifest_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let title = key
        .open_field("workspace-a", &title_id, &title_payload)
        .unwrap();
    let manifest = key
        .open_field("workspace-a", &manifest_id, &manifest_payload)
        .unwrap();

    assert_eq!(title.revision, 9);
    assert_eq!(title.value, json!("Local"));
    assert_eq!(manifest.revision, 9);
    assert!(!manifest.deleted);
}

#[tokio::test]
async fn encrypted_local_edit_advances_the_live_manifest() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    let key = &workspace_keys["workspace-a"];
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Base')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();

    sqlx::query("UPDATE sessions SET title = 'Local' WHERE id = 'session-1'")
        .execute(db.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();

    let manifest_id = key.blind_field_id("sessions", "session-1", ROW_MANIFEST_FIELD);
    let local_manifest_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&manifest_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let local_manifest = key
        .open_field("workspace-a", &manifest_id, &local_manifest_payload)
        .unwrap();
    assert_eq!(local_manifest.revision, 2);

    let remote_tombstone = key
        .seal_field(
            "workspace-a",
            "sessions",
            "session-1",
            ROW_MANIFEST_FIELD,
            "00000000000000000000000000000000",
            2,
            true,
            Value::Null,
        )
        .unwrap();
    sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
        .bind(remote_tombstone.payload)
        .bind(&manifest_id)
        .execute(db.pool())
        .await
        .unwrap();
    let stats = apply_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();
    let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let repaired_manifest: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(manifest_id)
            .fetch_one(db.pool())
            .await
            .unwrap();

    assert_eq!(stats.rejected_rollbacks, 1);
    assert_eq!(title, "Local");
    assert_eq!(repaired_manifest, local_manifest_payload);
}

#[tokio::test]
async fn local_transcript_edit_rebases_above_witnessed_field_and_tombstone() {
    let db = test_db().await;
    let workspace_keys = keys("workspace-a");
    let key = &workspace_keys["workspace-a"];
    let local_words = r#"[{"text":"preserve this local tail"}]"#;
    sqlx::query(
        "INSERT INTO transcripts (id, workspace_id, words_json)
             VALUES ('transcript-1', 'workspace-a', '[{\"text\":\"base\"}]')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();

    let words_id = key.blind_field_id("transcripts", "transcript-1", "words_json");
    let manifest_id = key.blind_field_id("transcripts", "transcript-1", ROW_MANIFEST_FIELD);
    let remote_words = key
        .seal_field(
            "workspace-a",
            "transcripts",
            "transcript-1",
            "words_json",
            "ffffffffffffffffffffffffffffffff",
            7,
            false,
            json!(r#"[{"text":"remote"}]"#),
        )
        .unwrap();
    let remote_tombstone = key
        .seal_field(
            "workspace-a",
            "transcripts",
            "transcript-1",
            ROW_MANIFEST_FIELD,
            "ffffffffffffffffffffffffffffffff",
            8,
            true,
            Value::Null,
        )
        .unwrap();
    merge_e2ee_witness_events(
        db.pool(),
        key,
        "workspace-a",
        &[
            E2eeWitnessEvent {
                sequence: 1,
                record_id: words_id.clone(),
                workspace_id: "workspace-a".to_string(),
                payload_hash: anlg_e2ee::payload_hash(&remote_words.payload),
                payload: remote_words.payload.clone(),
            },
            E2eeWitnessEvent {
                sequence: 2,
                record_id: manifest_id.clone(),
                workspace_id: "workspace-a".to_string(),
                payload_hash: anlg_e2ee::payload_hash(&remote_tombstone.payload),
                payload: remote_tombstone.payload.clone(),
            },
        ],
    )
    .await
    .unwrap();
    sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
        .bind(&remote_words.payload)
        .bind(&words_id)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
        .bind(&remote_tombstone.payload)
        .bind(&manifest_id)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE transcripts SET words_json = ? WHERE id = 'transcript-1'")
        .bind(local_words)
        .execute(db.pool())
        .await
        .unwrap();

    let skipped = apply_e2ee_replica_changes_with_witness(db.pool(), &workspace_keys)
        .await
        .unwrap();
    assert!(skipped.skipped_local_changes > 0);
    encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();

    let local_words_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&words_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let local_manifest_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&manifest_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let rebased_words = key
        .open_field("workspace-a", &words_id, &local_words_payload)
        .unwrap();
    let rebased_manifest = key
        .open_field("workspace-a", &manifest_id, &local_manifest_payload)
        .unwrap();
    assert_eq!(rebased_words.value, json!(local_words));
    assert!(rebased_words.revision > 7);
    assert!(!rebased_manifest.deleted);
    assert!(rebased_manifest.revision > 8);

    sqlx::query("UPDATE e2ee_records SET payload = ? WHERE id = ?")
        .bind(remote_tombstone.payload)
        .bind(&manifest_id)
        .execute(db.pool())
        .await
        .unwrap();
    apply_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();
    let preserved: String =
        sqlx::query_scalar("SELECT words_json FROM transcripts WHERE id = 'transcript-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(preserved, local_words);
}

#[tokio::test]
async fn tombstoned_rows_can_be_recreated_with_retained_or_changed_fields() {
    let workspace_keys = keys("workspace-a");
    let source = test_db().await;
    let target = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Same')",
    )
    .execute(source.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();
    copy_replica(source.pool(), target.pool()).await;
    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();

    for title in ["Same", "Changed"] {
        sqlx::query("DELETE FROM sessions WHERE id = 'session-1'")
            .execute(source.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();
        copy_replica(source.pool(), target.pool()).await;
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        let deleted: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE id = 'session-1'")
                .fetch_one(target.pool())
                .await
                .unwrap();
        assert_eq!(deleted, 0);

        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('session-1', 'workspace-a', 'user-a', ?)",
        )
        .bind(title)
        .execute(source.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
            .await
            .unwrap();
        copy_replica(source.pool(), target.pool()).await;
        apply_e2ee_replica_changes(target.pool(), &workspace_keys)
            .await
            .unwrap();
        let materialized: String =
            sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
                .fetch_one(target.pool())
                .await
                .unwrap();
        assert_eq!(materialized, title);
    }
}

#[tokio::test]
async fn chunked_recreation_materializes_late_transcript_and_summary_fields() {
    let workspace_keys = keys("workspace-a");
    let key = &workspace_keys["workspace-a"];
    let source = test_db().await;
    let target = test_db().await;
    let words = r#"[{"text":"the complete transcript"}]"#;
    let summary = r#"{"type":"doc","content":[{"type":"paragraph","text":"Full summary"}]}"#;
    sqlx::query(
        "INSERT INTO transcripts (id, workspace_id, words_json)
             VALUES ('transcript-1', 'workspace-a', ?)",
    )
    .bind(words)
    .execute(source.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO session_documents (id, workspace_id, kind, body)
             VALUES ('summary-1', 'workspace-a', 'summary', ?)",
    )
    .bind(summary)
    .execute(source.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();
    copy_replica(source.pool(), target.pool()).await;
    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();

    let words_id = key.blind_field_id("transcripts", "transcript-1", "words_json");
    let body_id = key.blind_field_id("session_documents", "summary-1", "body");
    let transcript_manifest_id =
        key.blind_field_id("transcripts", "transcript-1", ROW_MANIFEST_FIELD);
    let summary_manifest_id =
        key.blind_field_id("session_documents", "summary-1", ROW_MANIFEST_FIELD);
    let words_payload: String = sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
        .bind(&words_id)
        .fetch_one(source.pool())
        .await
        .unwrap();
    let body_payload: String = sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
        .bind(&body_id)
        .fetch_one(source.pool())
        .await
        .unwrap();

    sqlx::query("DELETE FROM transcripts WHERE id = 'transcript-1'")
        .execute(source.pool())
        .await
        .unwrap();
    sqlx::query("DELETE FROM session_documents WHERE id = 'summary-1'")
        .execute(source.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(source.pool(), &workspace_keys)
        .await
        .unwrap();
    copy_replica(source.pool(), target.pool()).await;
    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();

    let transcript_tombstone_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&transcript_manifest_id)
            .fetch_one(source.pool())
            .await
            .unwrap();
    let summary_tombstone_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&summary_manifest_id)
            .fetch_one(source.pool())
            .await
            .unwrap();
    let transcript_tombstone = key
        .open_field(
            "workspace-a",
            &transcript_manifest_id,
            &transcript_tombstone_payload,
        )
        .unwrap();
    let summary_tombstone = key
        .open_field(
            "workspace-a",
            &summary_manifest_id,
            &summary_tombstone_payload,
        )
        .unwrap();
    let live_transcript = key
        .seal_field(
            "workspace-a",
            "transcripts",
            "transcript-1",
            ROW_MANIFEST_FIELD,
            "ffffffffffffffffffffffffffffffff",
            transcript_tombstone.revision + 1,
            false,
            json!(true),
        )
        .unwrap();
    let live_summary = key
        .seal_field(
            "workspace-a",
            "session_documents",
            "summary-1",
            ROW_MANIFEST_FIELD,
            "ffffffffffffffffffffffffffffffff",
            summary_tombstone.revision + 1,
            false,
            json!(true),
        )
        .unwrap();

    sqlx::query("DELETE FROM e2ee_records")
        .execute(target.pool())
        .await
        .unwrap();
    for (id, payload) in [
        (&transcript_manifest_id, &live_transcript.payload),
        (&summary_manifest_id, &live_summary.payload),
    ] {
        sqlx::query(
            "INSERT INTO e2ee_records (id, workspace_id, payload)
                 VALUES (?, 'workspace-a', ?)",
        )
        .bind(id)
        .bind(payload)
        .execute(target.pool())
        .await
        .unwrap();
    }
    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();

    let defaults: (String, String) = (
        sqlx::query_scalar("SELECT words_json FROM transcripts WHERE id = 'transcript-1'")
            .fetch_one(target.pool())
            .await
            .unwrap(),
        sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'summary-1'")
            .fetch_one(target.pool())
            .await
            .unwrap(),
    );
    assert_eq!(defaults, ("[]".to_string(), String::new()));

    for (id, payload) in [(&words_id, &words_payload), (&body_id, &body_payload)] {
        sqlx::query(
            "INSERT INTO e2ee_records (id, workspace_id, payload)
                 VALUES (?, 'workspace-a', ?)",
        )
        .bind(id)
        .bind(payload)
        .execute(target.pool())
        .await
        .unwrap();
    }
    apply_e2ee_replica_changes(target.pool(), &workspace_keys)
        .await
        .unwrap();

    let materialized: (String, String) = (
        sqlx::query_scalar("SELECT words_json FROM transcripts WHERE id = 'transcript-1'")
            .fetch_one(target.pool())
            .await
            .unwrap(),
        sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'summary-1'")
            .fetch_one(target.pool())
            .await
            .unwrap(),
    );
    assert_eq!(materialized, (words.to_string(), summary.to_string()));
}

#[tokio::test]
async fn recreation_advances_retained_fields_beyond_remote_tombstone_state() {
    let workspace_keys = keys("workspace-a");
    let key = &workspace_keys["workspace-a"];
    let local = test_db().await;
    let remote = test_db().await;
    for db in [&local, &remote] {
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
                 VALUES ('session-1', 'workspace-a', 'user-a', 'A')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        encrypt_e2ee_replica_changes(db.pool(), &workspace_keys)
            .await
            .unwrap();
    }

    sqlx::query("UPDATE sessions SET title = 'B' WHERE id = 'session-1'")
        .execute(remote.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(remote.pool(), &workspace_keys)
        .await
        .unwrap();
    sqlx::query("DELETE FROM sessions WHERE id = 'session-1'")
        .execute(remote.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(remote.pool(), &workspace_keys)
        .await
        .unwrap();

    let title_id = key.blind_field_id("sessions", "session-1", "title");
    let manifest_id = key.blind_field_id("sessions", "session-1", ROW_MANIFEST_FIELD);
    let remote_title_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&title_id)
            .fetch_one(remote.pool())
            .await
            .unwrap();
    let remote_manifest_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&manifest_id)
            .fetch_one(remote.pool())
            .await
            .unwrap();
    let remote_title = key
        .open_field("workspace-a", &title_id, &remote_title_payload)
        .unwrap();
    let remote_manifest = key
        .open_field("workspace-a", &manifest_id, &remote_manifest_payload)
        .unwrap();

    copy_replica(remote.pool(), local.pool()).await;
    apply_e2ee_replica_changes(local.pool(), &workspace_keys)
        .await
        .unwrap();
    merge_e2ee_witness_events(
        local.pool(),
        key,
        "workspace-a",
        &[
            E2eeWitnessEvent {
                sequence: 1,
                record_id: title_id.clone(),
                workspace_id: "workspace-a".to_string(),
                payload_hash: anlg_e2ee::payload_hash(&remote_title_payload),
                payload: remote_title_payload,
            },
            E2eeWitnessEvent {
                sequence: 2,
                record_id: manifest_id.clone(),
                workspace_id: "workspace-a".to_string(),
                payload_hash: anlg_e2ee::payload_hash(&remote_manifest_payload),
                payload: remote_manifest_payload,
            },
        ],
    )
    .await
    .unwrap();
    let deleted: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions WHERE id = 'session-1'")
        .fetch_one(local.pool())
        .await
        .unwrap();
    assert_eq!(deleted, 0);

    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'A')",
    )
    .execute(local.pool())
    .await
    .unwrap();
    encrypt_e2ee_replica_changes(local.pool(), &workspace_keys)
        .await
        .unwrap();

    let local_title_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&title_id)
            .fetch_one(local.pool())
            .await
            .unwrap();
    let local_manifest_payload: String =
        sqlx::query_scalar("SELECT payload FROM e2ee_records WHERE id = ?")
            .bind(&manifest_id)
            .fetch_one(local.pool())
            .await
            .unwrap();
    let local_title = key
        .open_field("workspace-a", &title_id, &local_title_payload)
        .unwrap();
    let local_manifest = key
        .open_field("workspace-a", &manifest_id, &local_manifest_payload)
        .unwrap();

    assert_eq!(local_title.value, json!("A"));
    assert!(local_title.revision > remote_title.revision);
    assert!(!local_manifest.deleted);
    assert!(local_manifest.revision > remote_manifest.revision);
}
