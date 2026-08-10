use super::*;

#[test]
fn cloudsync_registry_enables_only_the_encrypted_replica() {
    let registry = cloudsync_table_registry();
    let enabled: Vec<&str> = registry
        .iter()
        .filter(|table| table.enabled)
        .map(|table| table.table_name.as_str())
        .collect();

    assert_eq!(registry.len(), 21);
    assert_eq!(enabled, vec!["e2ee_records"]);
    assert!(
        !registry
            .iter()
            .any(|table| table.table_name == "migration_import_runs")
    );
    assert!(!registry.iter().any(|table| {
        matches!(
            table.table_name.as_str(),
            "search_index_dirty" | "search_index_state"
        )
    }));
    assert!(
        registry
            .iter()
            .any(|table| { table.table_name == "workspaces" && !table.enabled })
    );
    assert!(
        registry
            .iter()
            .any(|table| { table.table_name == "workspace_memberships" && !table.enabled })
    );
    assert!(cloudsync_alter_guard_required("sessions"));
    assert!(cloudsync_alter_guard_required("synced_preferences"));
    assert!(cloudsync_alter_guard_required("e2ee_records"));
    assert!(!cloudsync_alter_guard_required("workspaces"));
    assert!(!cloudsync_alter_guard_required("workspace_memberships"));
    assert!(!cloudsync_alter_guard_required("calendars"));
}

#[tokio::test]
async fn e2ee_order_migration_backfills_the_canonical_payload_locally() {
    let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
    let payload = "ciphertext";
    let payload_hash = anlg_e2ee::payload_hash(payload);
    sqlx::raw_sql(include_str!(
        "../../migrations/20260717120000_e2ee_replica.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO e2ee_records (id, workspace_id, payload)
             VALUES ('record-1', 'workspace-a', ?)",
    )
    .bind(payload)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO e2ee_local_state (
               record_id, workspace_id, table_name, row_id, field_name,
               revision, value_tag, payload_hash
             ) VALUES (
               'record-1', 'workspace-a', 'sessions', 'session-1',
               'title', 1, 'tag', ?
             )",
    )
    .bind(&payload_hash)
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO e2ee_local_state (
               record_id, workspace_id, table_name, row_id, field_name,
               revision, value_tag, payload_hash
             ) VALUES (
               'orphan-record', 'workspace-a', 'sessions', 'session-2',
               'title', 1, 'tag', 'stale-hash'
             )",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let migration = APP_MIGRATION_STEPS
        .iter()
        .find(|step| step.id == "20260717192000_e2ee_replica_order")
        .unwrap();
    assert_eq!(migration.scope, anlg_db_migrate::MigrationScope::Plain);
    sqlx::raw_sql(migration.sql)
        .execute(db.pool())
        .await
        .unwrap();

    let state: (String, String, String) = sqlx::query_as(
        "SELECT writer_id, payload, payload_hash
             FROM e2ee_local_state
             WHERE record_id = 'record-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(state, (String::new(), payload.to_string(), payload_hash));
    let orphan_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM e2ee_local_state WHERE record_id = 'orphan-record'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(orphan_count, 0);
    assert!(
        !cloudsync_table_registry()
            .iter()
            .any(|table| table.table_name == "e2ee_local_device")
    );
    assert!(!cloudsync_alter_guard_required("e2ee_local_device"));
}

#[test]
fn e2ee_witness_state_is_local_only() {
    let migration = APP_MIGRATION_STEPS
        .iter()
        .find(|step| step.id == "20260717193000_e2ee_freshness_witness")
        .unwrap();

    assert_eq!(migration.scope, anlg_db_migrate::MigrationScope::Plain);
    for table_name in ["e2ee_witness_records", "e2ee_witness_state"] {
        assert!(
            !cloudsync_table_registry()
                .iter()
                .any(|table| table.table_name == table_name)
        );
        assert!(!cloudsync_alter_guard_required(table_name));
    }
}

#[test]
fn e2ee_witness_pending_is_local_only_and_checksum_is_pinned() {
    let migration = APP_MIGRATION_STEPS
        .iter()
        .find(|step| step.id == "20260725001400_e2ee_witness_pending")
        .unwrap();

    assert_eq!(migration.scope, anlg_db_migrate::MigrationScope::Plain);
    assert!(
        !cloudsync_table_registry()
            .iter()
            .any(|table| table.table_name == "e2ee_witness_pending")
    );
    assert!(!cloudsync_alter_guard_required("e2ee_witness_pending"));
    assert_eq!(
        checksum_hex(&migration_checksum(migration.sql)),
        "c9d894875eac335555da8b9c093394f98df423e483bbaa4edca1d651bb46e23c9b37064f4fcacaeaaed12a720f89f654"
    );
}

#[tokio::test]
async fn e2ee_witness_pending_migration_seeds_only_dominating_local_state() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_migrate::migrate(
        &db,
        anlg_db_migrate::DbSchema {
            steps: migration_steps_before("20260725001400_e2ee_witness_pending"),
            validate_cloudsync_table: cloudsync_alter_guard_required,
        },
    )
    .await
    .unwrap();

    sqlx::raw_sql(
            "INSERT INTO e2ee_local_state (
               record_id, workspace_id, table_name, row_id, field_name, revision,
               writer_id, value_tag, payload_hash, payload
             ) VALUES
               ('missing', 'workspace-a', 'sessions', 'row', 'title', 1, 'a', 'tag', 'a', 'payload'),
               ('local-revision', 'workspace-a', 'sessions', 'row', 'title', 2, 'a', 'tag', 'a', 'payload'),
               ('local-writer', 'workspace-a', 'sessions', 'row', 'title', 1, 'b', 'tag', 'a', 'payload'),
               ('local-hash', 'workspace-a', 'sessions', 'row', 'title', 1, 'a', 'tag', 'b', 'payload'),
               ('equal', 'workspace-a', 'sessions', 'row', 'title', 1, 'a', 'tag', 'a', 'payload'),
               ('witness-revision', 'workspace-a', 'sessions', 'row', 'title', 1, 'a', 'tag', 'a', 'payload'),
               ('witness-writer', 'workspace-a', 'sessions', 'row', 'title', 1, 'a', 'tag', 'a', 'payload'),
               ('witness-hash', 'workspace-a', 'sessions', 'row', 'title', 1, 'a', 'tag', 'a', 'payload');

             INSERT INTO e2ee_witness_records (
               workspace_id, record_id, revision, writer_id, payload_hash, payload
             ) VALUES
               ('workspace-a', 'local-revision', 1, 'a', 'a', 'payload'),
               ('workspace-a', 'local-writer', 1, 'a', 'a', 'payload'),
               ('workspace-a', 'local-hash', 1, 'a', 'a', 'payload'),
               ('workspace-a', 'equal', 1, 'a', 'a', 'payload'),
               ('workspace-a', 'witness-revision', 2, 'a', 'a', 'payload'),
               ('workspace-a', 'witness-writer', 1, 'b', 'a', 'payload'),
               ('workspace-a', 'witness-hash', 1, 'a', 'b', 'payload');",
        )
        .execute(db.pool())
        .await
        .unwrap();

    anlg_db_migrate::migrate(&db, schema()).await.unwrap();

    let pending: Vec<String> =
        sqlx::query_scalar("SELECT record_id FROM e2ee_witness_pending ORDER BY record_id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(
        pending,
        ["local-hash", "local-revision", "local-writer", "missing"]
    );

    let foreign_key: (String, String, String, String) = sqlx::query_as(
        "SELECT \"table\", \"from\", \"to\", on_delete
             FROM pragma_foreign_key_list('e2ee_witness_pending')",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        foreign_key,
        (
            "e2ee_local_state".to_string(),
            "record_id".to_string(),
            "record_id".to_string(),
            "CASCADE".to_string(),
        )
    );
}

#[test]
fn e2ee_dirty_rows_is_local_only_with_scoped_domain_triggers() {
    let table_migration = APP_MIGRATION_STEPS
        .iter()
        .find(|step| step.id == "20260723120000_e2ee_dirty_rows")
        .unwrap();
    assert_eq!(
        table_migration.scope,
        anlg_db_migrate::MigrationScope::Plain
    );
    for local_table in ["e2ee_apply_guard", "e2ee_dirty_rows"] {
        assert!(
            !cloudsync_table_registry()
                .iter()
                .any(|table| table.table_name == local_table)
        );
        assert!(!cloudsync_alter_guard_required(local_table));
    }

    let trigger_migrations: Vec<_> = APP_MIGRATION_STEPS
        .iter()
        .filter(|step| step.id.contains("_e2ee_dirty_") && step.id.ends_with("_triggers"))
        .collect();
    assert_eq!(trigger_migrations.len(), E2EE_DOMAIN_TABLES.len());
    for table_name in E2EE_DOMAIN_TABLES {
        assert!(trigger_migrations.iter().any(|step| matches!(
            step.scope,
            anlg_db_migrate::MigrationScope::CloudsyncAlter {
                table_name: scoped_table
            } if scoped_table == *table_name
        )));
    }
}

#[tokio::test]
async fn e2ee_dirty_rows_migration_seeds_existing_domain_rows_and_tombstones() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_migrate::migrate(
        &db,
        anlg_db_migrate::DbSchema {
            steps: migration_steps_before("20260723120000_e2ee_dirty_rows"),
            validate_cloudsync_table: cloudsync_alter_guard_required,
        },
    )
    .await
    .unwrap();

    // Only the domain tables that predate the dirty-rows migration were backfilled.
    let backfilled_tables: Vec<&str> = E2EE_DOMAIN_TABLES
        .iter()
        .copied()
        .filter(|table_name| *table_name != "synced_preferences")
        .collect();
    for table_name in &backfilled_tables {
        let insert_sql = format!(
            "INSERT INTO {table_name} (id, workspace_id, deleted_at)
                 VALUES (?, 'workspace-a', NULL), (?, 'workspace-a', '2026-07-23T00:00:00Z')"
        );
        sqlx::query(sqlx::AssertSqlSafe(insert_sql.as_str()))
            .bind(format!("live-{table_name}"))
            .bind(format!("deleted-{table_name}"))
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO e2ee_local_state (
                   record_id, workspace_id, table_name, row_id, field_name
                 ) VALUES (?, 'workspace-a', ?, ?, '$row')",
        )
        .bind(format!("manifest-{table_name}"))
        .bind(table_name)
        .bind(format!("absent-{table_name}"))
        .execute(db.pool())
        .await
        .unwrap();
    }

    anlg_db_migrate::migrate(&db, schema()).await.unwrap();

    let dirty_rows: Vec<(String, String, String, i64)> = sqlx::query_as(
        "SELECT workspace_id, table_name, row_id, generation
             FROM e2ee_dirty_rows
             ORDER BY table_name, row_id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(dirty_rows.len(), backfilled_tables.len() * 3);
    for table_name in &backfilled_tables {
        assert!(dirty_rows.contains(&(
            "workspace-a".to_string(),
            table_name.to_string(),
            format!("live-{table_name}"),
            1,
        )));
        assert!(dirty_rows.contains(&(
            "workspace-a".to_string(),
            table_name.to_string(),
            format!("deleted-{table_name}"),
            1,
        )));
        assert!(dirty_rows.contains(&(
            "workspace-a".to_string(),
            table_name.to_string(),
            format!("absent-{table_name}"),
            1,
        )));
    }
}

#[tokio::test]
async fn synced_preferences_migration_backfills_settings_and_marks_them_dirty() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_migrate::migrate(
        &db,
        anlg_db_migrate::DbSchema {
            steps: migration_steps_before("20260810120000_synced_preferences"),
            validate_cloudsync_table: cloudsync_alter_guard_required,
        },
    )
    .await
    .unwrap();

    sqlx::raw_sql(
        "INSERT INTO app_settings (id, value_json, updated_at) VALUES
           ('cloudsync_workspace_binding', '{\"workspace_id\":\"workspace-a\"}', '2026-08-10T00:00:00Z'),
           ('theme', '\"dark\"', '2026-08-01T00:00:00Z'),
           ('app_icon', '\"anagram\"', '2026-08-02T00:00:00Z'),
           ('microphone_device', '\"builtin\"', '2026-08-03T00:00:00Z')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    anlg_db_migrate::migrate(&db, schema()).await.unwrap();

    let preferences: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, workspace_id, value_json, updated_at
             FROM synced_preferences
             ORDER BY id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        preferences,
        vec![
            (
                "app_icon".to_string(),
                "workspace-a".to_string(),
                "\"anagram\"".to_string(),
                "2026-08-02T00:00:00Z".to_string(),
            ),
            (
                "theme".to_string(),
                "workspace-a".to_string(),
                "\"dark\"".to_string(),
                "2026-08-01T00:00:00Z".to_string(),
            ),
        ]
    );

    let dirty_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT workspace_id, row_id
             FROM e2ee_dirty_rows
             WHERE table_name = 'synced_preferences'
             ORDER BY row_id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        dirty_rows,
        vec![
            ("workspace-a".to_string(), "app_icon".to_string()),
            ("workspace-a".to_string(), "theme".to_string()),
        ]
    );
}

#[tokio::test]
async fn synced_preferences_backfill_is_skipped_without_a_workspace_binding() {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_migrate::migrate(
        &db,
        anlg_db_migrate::DbSchema {
            steps: migration_steps_before("20260810120000_synced_preferences"),
            validate_cloudsync_table: cloudsync_alter_guard_required,
        },
    )
    .await
    .unwrap();

    sqlx::query("INSERT INTO app_settings (id, value_json) VALUES ('theme', '\"dark\"')")
        .execute(db.pool())
        .await
        .unwrap();

    anlg_db_migrate::migrate(&db, schema()).await.unwrap();

    let preference_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM synced_preferences")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(preference_count, 0);
}

#[tokio::test]
async fn e2ee_dirty_triggers_track_domain_inserts_updates_and_deletes() {
    let db = test_db().await;

    let trigger_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
             FROM sqlite_master
             WHERE type = 'trigger' AND name LIKE 'e2ee_dirty_%'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(trigger_count, (E2EE_DOMAIN_TABLES.len() * 3) as i64);

    for table_name in E2EE_DOMAIN_TABLES {
        let row_id = format!("row-{table_name}");
        let insert_sql =
            format!("INSERT INTO {table_name} (id, workspace_id) VALUES (?, 'workspace-a')");
        sqlx::query(sqlx::AssertSqlSafe(insert_sql.as_str()))
            .bind(&row_id)
            .execute(db.pool())
            .await
            .unwrap();

        let update_sql = format!(
            "UPDATE {table_name}
                 SET updated_at = '2026-07-23T01:00:00Z'
                 WHERE id = ?"
        );
        sqlx::query(sqlx::AssertSqlSafe(update_sql.as_str()))
            .bind(&row_id)
            .execute(db.pool())
            .await
            .unwrap();

        let delete_sql = format!("DELETE FROM {table_name} WHERE id = ?");
        sqlx::query(sqlx::AssertSqlSafe(delete_sql.as_str()))
            .bind(&row_id)
            .execute(db.pool())
            .await
            .unwrap();

        let generation: i64 = sqlx::query_scalar(
            "SELECT generation
                 FROM e2ee_dirty_rows
                 WHERE workspace_id = 'workspace-a'
                   AND table_name = ?
                   AND row_id = ?",
        )
        .bind(table_name)
        .bind(&row_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(generation, 3, "{table_name}");
    }
}

#[tokio::test]
async fn e2ee_dirty_update_tracks_both_sides_of_an_identity_move() {
    let db = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id)
             VALUES ('session-a', 'workspace-a')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE sessions
             SET workspace_id = 'workspace-b'
             WHERE id = 'session-a'",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let generations: Vec<(String, i64)> = sqlx::query_as(
        "SELECT workspace_id, generation
             FROM e2ee_dirty_rows
             WHERE table_name = 'sessions' AND row_id = 'session-a'
             ORDER BY workspace_id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        generations,
        vec![
            ("workspace-a".to_string(), 2),
            ("workspace-b".to_string(), 1)
        ]
    );
}

#[tokio::test]
async fn e2ee_apply_guard_suppresses_only_the_guarded_write() {
    let db = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id)
             VALUES ('local-session', 'workspace-a')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO e2ee_apply_guard (workspace_id, table_name, row_id)
             VALUES ('workspace-a', 'sessions', 'local-session')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE sessions
             SET title = 'remote title'
             WHERE id = 'local-session'",
    )
    .execute(db.pool())
    .await
    .unwrap();

    let generation: i64 = sqlx::query_scalar(
        "SELECT generation
             FROM e2ee_dirty_rows
             WHERE workspace_id = 'workspace-a'
               AND table_name = 'sessions'
               AND row_id = 'local-session'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(generation, 1);
    sqlx::query("DELETE FROM sessions WHERE id = 'local-session'")
        .execute(db.pool())
        .await
        .unwrap();
    let generation_after_delete: i64 = sqlx::query_scalar(
        "SELECT generation
             FROM e2ee_dirty_rows
             WHERE workspace_id = 'workspace-a'
               AND table_name = 'sessions'
               AND row_id = 'local-session'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(generation_after_delete, 1);

    sqlx::query(
        "INSERT INTO e2ee_apply_guard (workspace_id, table_name, row_id)
             VALUES
               ('workspace-a', 'sessions', 'remote-session'),
               ('workspace-b', 'sessions', 'remote-session')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id)
             VALUES ('remote-session', 'workspace-a')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE sessions
             SET workspace_id = 'workspace-b'
             WHERE id = 'remote-session'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query("DELETE FROM sessions WHERE id = 'remote-session'")
        .execute(db.pool())
        .await
        .unwrap();

    let remote_dirty_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
             FROM e2ee_dirty_rows
             WHERE table_name = 'sessions' AND row_id = 'remote-session'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(remote_dirty_count, 0);
}

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(target_os = "linux", target_env = "musl", target_arch = "aarch64"),
    all(target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(target_os = "windows", target_arch = "x86_64"),
))]
#[tokio::test]
async fn e2ee_dirty_triggers_apply_to_initialized_cloudsync_tables() {
    let db = Db::connect_memory().await.unwrap();
    anlg_db_migrate::migrate(
        &db,
        anlg_db_migrate::DbSchema {
            steps: migration_steps_before("20260723120000_e2ee_dirty_rows"),
            validate_cloudsync_table: cloudsync_alter_guard_required,
        },
    )
    .await
    .unwrap();
    // synced_preferences does not exist yet at this point in the migration history.
    for table_name in E2EE_DOMAIN_TABLES
        .iter()
        .filter(|table_name| **table_name != "synced_preferences")
    {
        db.cloudsync_init(table_name, None, None).await.unwrap();
    }

    anlg_db_migrate::migrate(&db, schema()).await.unwrap();

    let trigger_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
             FROM sqlite_master
             WHERE type = 'trigger' AND name LIKE 'e2ee_dirty_%'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(trigger_count, (E2EE_DOMAIN_TABLES.len() * 3) as i64);

    sqlx::query(
        "INSERT INTO transcripts (id, workspace_id)
             VALUES ('transcript-a', 'workspace-a')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let generation: i64 = sqlx::query_scalar(
        "SELECT generation
             FROM e2ee_dirty_rows
             WHERE workspace_id = 'workspace-a'
               AND table_name = 'transcripts'
               AND row_id = 'transcript-a'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(generation, 1);
}
