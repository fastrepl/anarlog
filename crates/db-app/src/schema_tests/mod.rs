use super::*;
use anlg_db_core::Db;

const LEGACY_SHARED_SESSION_CACHE_SQL: &str = r#"CREATE TABLE IF NOT EXISTS shared_session_cache (
  share_id          TEXT PRIMARY KEY NOT NULL CHECK (
    share_id = trim(share_id) AND length(share_id) > 0
  ),
  workspace_id      TEXT NOT NULL CHECK (
    workspace_id = trim(workspace_id) AND length(workspace_id) > 0
  ),
  session_id        TEXT NOT NULL CHECK (
    session_id = trim(session_id) AND length(session_id) > 0
  ),
  schema_version    INTEGER NOT NULL DEFAULT 1 CHECK (schema_version = 1),
  content_revision  INTEGER NOT NULL CHECK (content_revision > 0),
  title             TEXT NOT NULL DEFAULT '' CHECK (
    title = trim(title) AND length(CAST(title AS BLOB)) <= 4096
  ),
  body_json         TEXT NOT NULL DEFAULT '{"type":"doc","content":[{"type":"paragraph"}]}' CHECK (
    CASE
      WHEN json_valid(body_json) THEN
        json_type(body_json) = 'object'
        AND json_extract(body_json, '$.type') = 'doc'
        AND length(CAST(body_json AS BLOB)) <= 2097152
      ELSE 0
    END
  ),
  capability        TEXT NOT NULL DEFAULT 'viewer' CHECK (
    capability IN ('viewer', 'commenter', 'editor')
  ),
  manage_access     INTEGER NOT NULL DEFAULT 0 CHECK (manage_access IN (0, 1)),
  access_version    INTEGER NOT NULL CHECK (access_version > 0),
  published_at      TEXT NOT NULL CHECK (
    published_at = trim(published_at) AND length(published_at) > 0
  ),
  cached_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_shared_session_cache_workspace_id
ON shared_session_cache(workspace_id);
"#;

async fn test_db() -> Db {
    let db = Db::open(anlg_db_core::DbOpenOptions {
        storage: anlg_db_core::DbStorage::Memory,
        cloudsync_enabled: false,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(1),
    })
    .await
    .unwrap();
    prepare_schema(&db).await.unwrap();
    db
}

async fn initialize_enabled_cloudsync_tables(db: &Db) {
    for table in cloudsync_table_registry()
        .iter()
        .filter(|table| table.enabled)
    {
        db.cloudsync_init(
            &table.table_name,
            table.crdt_algo.as_deref(),
            table.init_flags,
        )
        .await
        .unwrap();
    }
}

fn migration_steps_before(id: &str) -> &'static [anlg_db_migrate::MigrationStep] {
    let index = APP_MIGRATION_STEPS
        .iter()
        .position(|step| step.id == id)
        .unwrap();
    &APP_MIGRATION_STEPS[..index]
}

fn schema_with_legacy_shared_session_cache(
    include_later_migrations: bool,
) -> anlg_db_migrate::DbSchema {
    let migration_index = APP_MIGRATION_STEPS
        .iter()
        .position(|step| step.id == "20260716173000_shared_session_cache")
        .unwrap();
    let mut steps = if include_later_migrations {
        APP_MIGRATION_STEPS.to_vec()
    } else {
        APP_MIGRATION_STEPS[..=migration_index].to_vec()
    };
    let migration = steps
        .iter_mut()
        .find(|step| step.id == "20260716173000_shared_session_cache")
        .unwrap();
    migration.sql = LEGACY_SHARED_SESSION_CACHE_SQL;

    anlg_db_migrate::DbSchema {
        steps: Box::leak(steps.into_boxed_slice()),
        validate_cloudsync_table: cloudsync_alter_guard_required,
    }
}

fn legacy_attachment_transfer_jobs_sql() -> String {
    include_str!("../../migrations/20260717150000_attachment_transfer_jobs.sql")
        .replace(
            "WHERE direction = 'delete' AND phase <> 'completed';",
            "WHERE direction = 'delete';",
        )
        .replace(
            "WHERE direction = 'upload'\n  AND remote_object_id <> ''\n  AND phase <> 'completed';",
            "WHERE direction = 'upload' AND remote_object_id <> '';",
        )
        .replace(
            "WHERE direction = 'delete'\n  AND remote_object_id <> ''\n  AND phase <> 'completed';",
            "WHERE direction = 'delete' AND remote_object_id <> '';",
        )
}

fn schema_with_legacy_attachment_transfer_jobs() -> anlg_db_migrate::DbSchema {
    let mut steps = APP_MIGRATION_STEPS.to_vec();
    let migration = steps
        .iter_mut()
        .find(|step| step.id == "20260717150000_attachment_transfer_jobs")
        .unwrap();
    migration.sql = Box::leak(legacy_attachment_transfer_jobs_sql().into_boxed_str());

    anlg_db_migrate::DbSchema {
        steps: Box::leak(steps.into_boxed_slice()),
        validate_cloudsync_table: cloudsync_alter_guard_required,
    }
}

async fn attachment_transfer_jobs_schema_checksum_for_test(db: &Db) -> String {
    let mut transaction = db.pool().begin().await.unwrap();
    let checksum = attachment_transfer_jobs_schema_checksum(&mut transaction)
        .await
        .unwrap();
    transaction.rollback().await.unwrap();
    checksum
}

async fn test_db_without_default_templates() -> Db {
    let db = Db::open(anlg_db_core::DbOpenOptions {
        storage: anlg_db_core::DbStorage::Memory,
        cloudsync_enabled: false,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(1),
    })
    .await
    .unwrap();
    anlg_db_migrate::migrate(
        &db,
        anlg_db_migrate::DbSchema {
            steps: &APP_MIGRATION_STEPS[..2],
            validate_cloudsync_table: cloudsync_alter_guard_required,
        },
    )
    .await
    .unwrap();
    sqlx::query(include_str!(
        "../../migrations/20260712170000_template_icons.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    db
}

mod attachments;
mod consent;
mod encrypted_replica;
mod entities;
mod migrations;
mod search_index;
mod shared_session_cache;
mod transcript_live_deltas;
mod voiceprints;
