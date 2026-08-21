use super::*;

#[tokio::test]
async fn consent_evidence_tables_stay_local_only() {
    let db = test_db().await;

    sqlx::query(
        "INSERT INTO session_disclosure_attempts (
           id, session_id, attempted_at, platform, surface,
           message_version, message, delivery, failure_reason
         ) VALUES (
           'attempt-1', 'session-1', '2026-08-21T00:00:00Z', 'slack_huddle', 'huddle',
           'anarlog-disclosure-v1', 'disclosure', 'sent', ''
         )",
    )
    .execute(db.pool())
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO session_participant_consent (
           session_id, participant_key, status, source, updated_at
         ) VALUES (
           'session-1', 'late-joiner', 'unknown', 'unseen', '2026-08-21T00:01:00Z'
         )",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let delivery: String = sqlx::query_scalar(
        "SELECT delivery FROM session_disclosure_attempts WHERE id = 'attempt-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(delivery, "sent");

    assert!(
        !cloudsync_table_registry()
            .iter()
            .any(|table| table.table_name == "session_disclosure_attempts"
                || table.table_name == "session_participant_consent")
    );
    assert!(!E2EE_DOMAIN_TABLES.contains(&"session_disclosure_attempts"));
    assert!(!E2EE_DOMAIN_TABLES.contains(&"session_participant_consent"));
}

#[tokio::test]
async fn sent_disclosure_cannot_be_stored_as_a_consent_source() {
    let db = test_db().await;
    let error = sqlx::query(
        "INSERT INTO session_participant_consent (
           session_id, participant_key, status, source, updated_at
         ) VALUES (
           'session-1', 'ada', 'consented', 'disclosure_sent', '2026-08-21T00:00:00Z'
         )",
    )
    .execute(db.pool())
    .await
    .unwrap_err();
    assert!(error.to_string().contains("CHECK"));
}
