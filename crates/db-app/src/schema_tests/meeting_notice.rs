use super::*;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const NOW: i64 = 1_800_000_000_000;

async fn file_db(path: &std::path::Path) -> Db {
    let db = Db::open(anlg_db_core::DbOpenOptions {
        storage: anlg_db_core::DbStorage::Local(path),
        cloudsync_enabled: false,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(4),
    })
    .await
    .unwrap();
    prepare_schema(&db).await.unwrap();
    db
}

#[tokio::test]
async fn a_claim_survives_a_restart_until_a_definitely_unsent_notice_releases_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    {
        let db = file_db(&path).await;
        assert!(
            claim_meeting_notice(db.pool(), "account-a", "session-1", "capture-1", NOW)
                .await
                .unwrap()
        );
        db.pool().close().await;
    }
    // A restarted app reads only the disk: the claim still holds, whatever the capture.
    let db = file_db(&path).await;
    let pool = db.pool();
    assert!(
        !claim_meeting_notice(pool, "account-a", "session-1", "capture-2", NOW + 1)
            .await
            .unwrap()
    );
    // Another capture can't give back the first one's claim.
    assert!(
        !release_meeting_notice(pool, "account-a", "session-1", "capture-2")
            .await
            .unwrap()
    );
    assert!(
        release_meeting_notice(pool, "account-a", "session-1", "capture-1")
            .await
            .unwrap()
    );
    assert!(
        claim_meeting_notice(pool, "account-a", "session-1", "capture-2", NOW + 2)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn simultaneous_claims_give_one_notice_per_session() {
    let dir = tempfile::tempdir().unwrap();
    let db = file_db(&dir.path().join("app.db")).await;
    let pool = db.pool();
    let (first, second, other) = tokio::join!(
        claim_meeting_notice(pool, "account-a", "session-1", "capture-1", NOW),
        claim_meeting_notice(pool, "account-a", "session-1", "capture-2", NOW),
        claim_meeting_notice(pool, "account-a", "session-2", "capture-3", NOW),
    );
    let (first, second, other) = (first.unwrap(), second.unwrap(), other.unwrap());
    assert!(first ^ second);
    assert!(other);
}

#[tokio::test]
async fn sent_and_unconfirmed_claims_are_never_released() {
    let db = test_db().await;
    let pool = db.pool();
    for (session, settlement) in [
        ("session-sent", MeetingNoticeSettlement::Sent),
        ("session-unconfirmed", MeetingNoticeSettlement::Unconfirmed),
    ] {
        assert!(
            claim_meeting_notice(pool, "account-a", session, "capture-1", NOW)
                .await
                .unwrap()
        );
        assert!(
            settle_meeting_notice(pool, "account-a", session, "capture-1", settlement, NOW)
                .await
                .unwrap()
        );
        assert!(
            !release_meeting_notice(pool, "account-a", session, "capture-1")
                .await
                .unwrap()
        );
        assert!(
            !claim_meeting_notice(pool, "account-a", session, "capture-2", NOW + DAY_MS)
                .await
                .unwrap()
        );
    }
    // Settling is the dispatching capture's alone, and only once.
    assert!(
        !settle_meeting_notice(
            pool,
            "account-a",
            "session-sent",
            "capture-1",
            MeetingNoticeSettlement::Unconfirmed,
            NOW
        )
        .await
        .unwrap()
    );
}

#[tokio::test]
async fn claims_are_per_account_and_store_neither_id() {
    let db = test_db().await;
    let pool = db.pool();
    assert!(
        claim_meeting_notice(pool, "account-a", "session-1", "capture-1", NOW)
            .await
            .unwrap()
    );
    assert!(
        claim_meeting_notice(pool, "account-b", "session-1", "capture-2", NOW)
            .await
            .unwrap()
    );
    let stored: Vec<(String, String)> =
        sqlx::query_as("SELECT claim_key, capture_id FROM local_meeting_notice_claims")
            .fetch_all(pool)
            .await
            .unwrap();
    assert_eq!(stored.len(), 2);
    for (key, _) in &stored {
        assert!(!key.contains("account") && !key.contains("session"));
    }
}

#[tokio::test]
async fn claims_expire_after_seven_days_but_a_claim_from_the_future_holds() {
    let db = test_db().await;
    let pool = db.pool();
    claim_meeting_notice(pool, "account-a", "old", "capture-1", NOW)
        .await
        .unwrap();
    claim_meeting_notice(pool, "account-a", "recent", "capture-2", NOW + 60_000)
        .await
        .unwrap();
    let later = NOW + MEETING_NOTICE_CLAIM_RETENTION_MS + 30_000;
    assert!(
        claim_meeting_notice(pool, "account-a", "old", "capture-3", later)
            .await
            .unwrap()
    );
    assert!(
        !claim_meeting_notice(pool, "account-a", "recent", "capture-4", later)
            .await
            .unwrap()
    );
    // The clock went back: a claim dated after now is kept, not taken as expired.
    assert!(
        !claim_meeting_notice(pool, "account-a", "recent", "capture-5", NOW - 10 * DAY_MS)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn unreadable_claim_storage_fails_the_claim() {
    let db = test_db().await;
    let pool = db.pool();
    sqlx::query("DROP TABLE local_meeting_notice_claims")
        .execute(pool)
        .await
        .unwrap();
    assert!(
        claim_meeting_notice(pool, "account-a", "session-1", "capture-1", NOW)
            .await
            .is_err()
    );
    let db = test_db().await;
    let error = sqlx::query(
        "INSERT INTO local_meeting_notice_claims (claim_key, capture_id, claimed_at_ms, state)
         VALUES (?, 'capture-1', 0, 'maybe')",
    )
    .bind(meeting_notice_claim_key("account-a", "session-1"))
    .execute(db.pool())
    .await
    .unwrap_err();
    assert!(error.to_string().contains("CHECK"));
}

#[tokio::test]
async fn an_unconfirmed_delivery_has_its_own_evidence_row() {
    let db = test_db().await;
    let pool = db.pool();
    // The shipped attempts table cannot say "may have been sent".
    let error = sqlx::query(
        "INSERT INTO session_disclosure_attempts (
           id, session_id, attempted_at, delivery
         ) VALUES ('attempt-1', 'session-1', '2026-11-17T00:00:00Z', 'unconfirmed')",
    )
    .execute(pool)
    .await
    .unwrap_err();
    assert!(error.to_string().contains("CHECK"));

    insert_disclosure_delivery(
        pool,
        &DisclosureDelivery {
            id: "delivery-1".into(),
            session_id: "session-1".into(),
            capture_id: "capture-1".into(),
            attempted_at_ms: NOW,
            platform: "zoom".into(),
            message_version: "anarlog-disclosure-v1".into(),
            message: "notice".into(),
            delivery: "unconfirmed".into(),
            reason: "".into(),
        },
    )
    .await
    .unwrap();
    let (delivery, attempted_at): (String, String) = sqlx::query_as(
        "SELECT delivery, attempted_at FROM session_disclosure_deliveries WHERE id = ?",
    )
    .bind("delivery-1")
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(delivery, "unconfirmed");
    assert_eq!(attempted_at, "2027-01-15T08:00:00.000Z");
}

#[tokio::test]
async fn notice_claim_migration_is_local_only_and_keeps_old_evidence() {
    let id = "20261117120500_local_meeting_notice_claims";
    let migration = APP_MIGRATION_STEPS
        .iter()
        .find(|step| step.id == id)
        .unwrap();
    assert!(matches!(
        migration.scope,
        anlg_db_migrate::MigrationScope::Plain
    ));
    assert!(!migration.sql.contains("-- breaking"));
    for table in [
        "local_meeting_notice_claims",
        "session_disclosure_deliveries",
    ] {
        assert!(
            !cloudsync_table_registry()
                .iter()
                .any(|entry| entry.table_name == table)
        );
        assert!(!E2EE_DOMAIN_TABLES.contains(&table));
    }

    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_migrate::migrate(
        &db,
        anlg_db_migrate::DbSchema {
            steps: migration_steps_before(id),
            validate_cloudsync_table: cloudsync_alter_guard_required,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO session_disclosure_attempts (
           id, session_id, attempted_at, delivery
         ) VALUES ('attempt-1', 'session-1', '2026-11-16T00:00:00Z', 'sent')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    anlg_db_migrate::migrate(&db, schema()).await.unwrap();

    let delivery: String = sqlx::query_scalar(
        "SELECT delivery FROM session_disclosure_attempts WHERE id = 'attempt-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(delivery, "sent");
}
