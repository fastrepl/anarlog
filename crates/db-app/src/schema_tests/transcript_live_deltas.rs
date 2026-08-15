use super::*;

#[tokio::test]
async fn transcript_content_revision_is_cloudsync_safe() {
    let migration = APP_MIGRATION_STEPS
        .iter()
        .find(|step| step.id == "20260815100000_transcript_content_revision")
        .unwrap();
    assert_eq!(
        migration.scope,
        anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "transcripts",
        }
    );

    let db = test_db().await;
    sqlx::query("INSERT INTO transcripts (id) VALUES ('transcript-1')")
        .execute(db.pool())
        .await
        .unwrap();
    let revision: i64 =
        sqlx::query_scalar("SELECT content_revision FROM transcripts WHERE id = 'transcript-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(revision, 0);
}

#[tokio::test]
async fn live_delta_journal_orders_chunks_and_rejects_duplicate_sequences() {
    let db = test_db().await;
    sqlx::query(
        "INSERT INTO transcript_live_state (transcript_id, next_sequence)
         VALUES ('transcript-1', 2)",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO transcript_live_deltas (id, transcript_id, sequence, delta_json)
         VALUES ('delta-2', 'transcript-1', 2, '{\"new_words\":[],\"replaced_ids\":[],\"partials\":[]}'),
                ('delta-1', 'transcript-1', 1, '{\"new_words\":[],\"replaced_ids\":[],\"partials\":[]}')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM transcript_live_deltas
         WHERE transcript_id = 'transcript-1'
         ORDER BY sequence",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(ids, ["delta-1", "delta-2"]);

    let duplicate = sqlx::query(
        "INSERT INTO transcript_live_deltas (id, transcript_id, sequence)
         VALUES ('delta-duplicate', 'transcript-1', 2)",
    )
    .execute(db.pool())
    .await;
    assert!(duplicate.is_err());
}

#[tokio::test]
async fn revision_cas_clears_journal_only_after_successful_compaction() {
    let db = test_db().await;
    sqlx::query("INSERT INTO transcripts (id) VALUES ('transcript-1')")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO transcript_live_state (transcript_id, next_sequence)
         VALUES ('transcript-1', 1)",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO transcript_live_deltas (id, transcript_id, sequence)
         VALUES ('delta-1', 'transcript-1', 0)",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let mut transaction = db.pool().begin().await.unwrap();
    sqlx::query(
        "UPDATE transcripts SET content_revision = content_revision + 1
         WHERE id = 'transcript-1' AND content_revision = 99",
    )
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "DELETE FROM transcript_live_state
         WHERE transcript_id = 'transcript-1' AND changes() = 1",
    )
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();

    let pending_after_failed_cas: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM transcript_live_deltas
         WHERE transcript_id = 'transcript-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(pending_after_failed_cas, 1);

    let mut transaction = db.pool().begin().await.unwrap();
    sqlx::query(
        "UPDATE transcripts SET content_revision = content_revision + 1
         WHERE id = 'transcript-1' AND content_revision = 0",
    )
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "DELETE FROM transcript_live_state
         WHERE transcript_id = 'transcript-1' AND changes() = 1",
    )
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();

    let pending_after_successful_cas: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM transcript_live_deltas
         WHERE transcript_id = 'transcript-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(pending_after_successful_cas, 0);
}
