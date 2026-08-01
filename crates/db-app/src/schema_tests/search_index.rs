use super::*;
use sqlx::Row;

#[test]
fn search_index_trigger_migrations_are_cloudsync_guarded() {
    let queue_step = APP_MIGRATION_STEPS
        .iter()
        .find(|step| step.id == "20260714120000_search_index_queue")
        .unwrap();
    assert_eq!(queue_step.scope, anlg_db_migrate::MigrationScope::Plain);

    for (id, table_name) in [
        ("20260714120100_search_index_sessions_triggers", "sessions"),
        (
            "20260714120200_search_index_session_documents_triggers",
            "session_documents",
        ),
        (
            "20260714120300_search_index_transcripts_triggers",
            "transcripts",
        ),
        ("20260714120400_search_index_humans_triggers", "humans"),
        (
            "20260714120500_search_index_organizations_triggers",
            "organizations",
        ),
    ] {
        let step = APP_MIGRATION_STEPS
            .iter()
            .find(|step| step.id == id)
            .unwrap();
        assert_eq!(
            step.scope,
            anlg_db_migrate::MigrationScope::CloudsyncAlter { table_name }
        );
    }
}

#[tokio::test]
async fn search_index_queue_coalesces_changes_and_tracks_session_moves() {
    let db = test_db().await;

    sqlx::query(
        "INSERT INTO sessions (id, title) VALUES ('session-1', 'One'), ('session-2', 'Two')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query("DELETE FROM search_index_dirty")
        .execute(db.pool())
        .await
        .unwrap();

    sqlx::query(
            "INSERT INTO session_documents (id, session_id, body) VALUES ('document-1', 'session-1', 'one')",
        )
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE session_documents SET body = 'two' WHERE id = 'document-1'")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE session_documents SET session_id = 'session-2' WHERE id = 'document-1'")
        .execute(db.pool())
        .await
        .unwrap();

    sqlx::query(
            "INSERT INTO transcripts (id, session_id, words_json) VALUES ('transcript-1', 'session-1', '[]')",
        )
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE transcripts SET session_id = 'session-2' WHERE id = 'transcript-1'")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("DELETE FROM transcripts WHERE id = 'transcript-1'")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("DELETE FROM session_documents WHERE id = 'document-1'")
        .execute(db.pool())
        .await
        .unwrap();

    let rows = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT entity_type, entity_id, generation
             FROM search_index_dirty
             ORDER BY entity_type, entity_id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();

    assert_eq!(
        rows,
        vec![
            ("session".to_string(), "session-1".to_string(), 5),
            ("session".to_string(), "session-2".to_string(), 4),
        ]
    );
}

#[tokio::test]
async fn search_index_queue_tracks_entity_lifecycle_and_starts_unversioned() {
    let db = test_db().await;

    let projection_version: i64 = sqlx::query_scalar(
        "SELECT projection_version FROM search_index_state WHERE id = 'default'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(projection_version, 0);

    sqlx::query("INSERT INTO sessions (id, title) VALUES ('session-1', 'One')")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO humans (id, name) VALUES ('human-1', 'Ada')")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO organizations (id, name) VALUES ('organization-1', 'Acme')")
        .execute(db.pool())
        .await
        .unwrap();

    sqlx::query("UPDATE sessions SET deleted_at = '2026-07-14T00:00:00Z' WHERE id = 'session-1'")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE humans SET memo = 'Updated' WHERE id = 'human-1'")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("DELETE FROM organizations WHERE id = 'organization-1'")
        .execute(db.pool())
        .await
        .unwrap();

    let rows = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT entity_type, entity_id, generation
             FROM search_index_dirty
             ORDER BY entity_type, entity_id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();

    assert_eq!(
        rows,
        vec![
            ("human".to_string(), "human-1".to_string(), 2),
            ("organization".to_string(), "organization-1".to_string(), 2,),
            ("session".to_string(), "session-1".to_string(), 2),
        ]
    );
}

#[tokio::test]
async fn registered_tables_match_cloudsync_schema_requirements() {
    let db = test_db().await;

    for table in cloudsync_table_registry() {
        let rows = sqlx::query(
            "SELECT name, type, \"notnull\", dflt_value, pk
                 FROM pragma_table_info(?)
                 ORDER BY cid",
        )
        .bind(&table.table_name)
        .fetch_all(db.pool())
        .await
        .unwrap();

        let pk_columns: Vec<_> = rows
            .iter()
            .filter(|row| row.get::<i64, _>("pk") > 0)
            .collect();
        assert_eq!(
            pk_columns.len(),
            1,
            "{} must have one primary key",
            table.table_name
        );

        let pk = pk_columns[0];
        assert_eq!(pk.get::<String, _>("name"), "id", "{}", table.table_name);
        assert_eq!(
            pk.get::<String, _>("type").to_uppercase(),
            "TEXT",
            "{}",
            table.table_name
        );
        assert_eq!(pk.get::<i64, _>("notnull"), 1, "{}", table.table_name);

        for row in rows
            .iter()
            .filter(|row| row.get::<i64, _>("pk") == 0 && row.get::<i64, _>("notnull") == 1)
        {
            assert!(
                row.get::<Option<String>, _>("dflt_value").is_some(),
                "{}.{} must define a DEFAULT value for SQLite Sync compatibility",
                table.table_name,
                row.get::<String, _>("name")
            );
        }
    }
}
