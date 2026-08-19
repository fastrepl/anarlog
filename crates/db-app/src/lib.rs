#![forbid(unsafe_code)]

mod calendar_ops;
mod calendar_types;
mod cloudsync;
mod e2ee;
mod event_ops;
mod event_types;
mod legacy_import;
mod session_ops;
mod session_types;
mod template_ops;
mod template_types;
mod voiceprint_ops;
mod voiceprint_types;
mod webhook_ops;
mod webhook_types;

pub use calendar_ops::*;
pub use calendar_types::*;
pub use cloudsync::*;
pub use e2ee::*;
pub use event_ops::*;
pub use event_types::*;
pub use legacy_import::*;
pub use session_ops::*;
pub use session_types::*;
use sha2::{Digest, Sha384};
pub use template_ops::*;
pub use template_types::*;
pub use voiceprint_ops::*;
pub use voiceprint_types::*;
pub use webhook_ops::*;
pub use webhook_types::*;

// Older builds keep running against databases that carry newer migrations, so
// every new migration must be downgrade-safe by default: additive only, new
// columns nullable or with a DEFAULT, no renames or drops of anything older
// builds read or write. If a migration cannot be downgrade-safe, put a
// "-- breaking" line in the leading comment block of its .sql file; older
// builds will then refuse to open the database with an explicit
// "update Anarlog" dialog instead of misbehaving on it.
pub const APP_MIGRATION_STEPS: &[anlg_db_migrate::MigrationStep] = &[
    anlg_db_migrate::MigrationStep {
        id: "20260413020000_templates",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260413020000_templates.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260414120000_calendars_events",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260414120000_calendars_events.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260524000000_default_templates",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260524000000_default_templates.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260624000000_repair_templates",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260624000000_repair_templates.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260710223922_canonical_data_model",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260710223922_canonical_data_model.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260710231809_import_target_audit",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260710231809_import_target_audit.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260711000000_calendar_event_tombstones",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260711000000_calendar_event_tombstones.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260712170000_template_icons",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260712170000_template_icons.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260713164500_repair_empty_session_titles",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260713164500_repair_empty_session_titles.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260714120000_search_index_queue",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260714120000_search_index_queue.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260714120100_search_index_sessions_triggers",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "sessions",
        },
        sql: include_str!("../migrations/20260714120100_search_index_sessions_triggers.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260714120200_search_index_session_documents_triggers",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "session_documents",
        },
        sql: include_str!(
            "../migrations/20260714120200_search_index_session_documents_triggers.sql"
        ),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260714120300_search_index_transcripts_triggers",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "transcripts",
        },
        sql: include_str!("../migrations/20260714120300_search_index_transcripts_triggers.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260714120400_search_index_humans_triggers",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "humans",
        },
        sql: include_str!("../migrations/20260714120400_search_index_humans_triggers.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260714120500_search_index_organizations_triggers",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "organizations",
        },
        sql: include_str!("../migrations/20260714120500_search_index_organizations_triggers.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260716120000_personal_workspaces",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260716120000_personal_workspaces.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260716130000_cloudsync_session_evictions",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260716130000_cloudsync_session_evictions.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260716173000_shared_session_cache",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260716173000_shared_session_cache.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260717120000_e2ee_replica",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260717120000_e2ee_replica.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260717140000_attachment_local_state",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260717140000_attachment_local_state.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260717192000_e2ee_replica_order",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260717192000_e2ee_replica_order.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260717193000_e2ee_freshness_witness",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260717193000_e2ee_freshness_witness.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260717150000_attachment_transfer_jobs",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260717150000_attachment_transfer_jobs.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260717170000_attachment_cloud_sync_intent",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "session_attachments",
        },
        sql: include_str!("../migrations/20260717170000_attachment_cloud_sync_intent.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260717171000_shared_session_attachment_cache",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260717171000_shared_session_attachment_cache.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260717172000_shared_session_cache_attachments",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260717172000_shared_session_cache_attachments.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260717190000_shared_session_cache_web_edits",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260717190000_shared_session_cache_web_edits.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260717191000_session_share_sync_state",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260717191000_session_share_sync_state.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260723120000_e2ee_dirty_rows",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260723120000_e2ee_dirty_rows.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260723120100_e2ee_dirty_action_items_triggers",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "action_items",
        },
        sql: include_str!("../migrations/20260723120100_e2ee_dirty_action_items_triggers.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260723120200_e2ee_dirty_humans_triggers",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "humans",
        },
        sql: include_str!("../migrations/20260723120200_e2ee_dirty_humans_triggers.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260723120300_e2ee_dirty_organizations_triggers",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "organizations",
        },
        sql: include_str!("../migrations/20260723120300_e2ee_dirty_organizations_triggers.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260723120400_e2ee_dirty_session_attachments_triggers",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "session_attachments",
        },
        sql: include_str!(
            "../migrations/20260723120400_e2ee_dirty_session_attachments_triggers.sql"
        ),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260723120500_e2ee_dirty_session_documents_triggers",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "session_documents",
        },
        sql: include_str!("../migrations/20260723120500_e2ee_dirty_session_documents_triggers.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260723120600_e2ee_dirty_session_participants_triggers",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "session_participants",
        },
        sql: include_str!(
            "../migrations/20260723120600_e2ee_dirty_session_participants_triggers.sql"
        ),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260723120700_e2ee_dirty_sessions_triggers",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "sessions",
        },
        sql: include_str!("../migrations/20260723120700_e2ee_dirty_sessions_triggers.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260723120800_e2ee_dirty_transcripts_triggers",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "transcripts",
        },
        sql: include_str!("../migrations/20260723120800_e2ee_dirty_transcripts_triggers.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260723120900_e2ee_witness_upload_index",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260723120900_e2ee_witness_upload_index.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260725001400_e2ee_witness_pending",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260725001400_e2ee_witness_pending.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260725020000_cloudsync_projection_indexes",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260725020000_cloudsync_projection_indexes.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260728090000_local_api",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260728090000_local_api.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260802120000_voiceprint_exemplars",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260802120000_voiceprint_exemplars.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260804110000_session_share_activation",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260804110000_session_share_activation.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260810120000_synced_preferences",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260810120000_synced_preferences.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260810120100_e2ee_dirty_synced_preferences_triggers",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "synced_preferences",
        },
        sql: include_str!(
            "../migrations/20260810120100_e2ee_dirty_synced_preferences_triggers.sql"
        ),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260810120200_synced_preferences_backfill",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260810120200_synced_preferences_backfill.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260810120300_synced_preferences_backfill_legacy",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260810120300_synced_preferences_backfill_legacy.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260811090000_voiceprint_candidates",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260811090000_voiceprint_candidates.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260811100000_drop_api_keys",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260811100000_drop_api_keys.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260812100000_e2ee_reconciliation_queues",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260812100000_e2ee_reconciliation_queues.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260812100100_e2ee_replica_reconciliation_triggers",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "e2ee_records",
        },
        sql: include_str!("../migrations/20260812100100_e2ee_replica_reconciliation_triggers.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260814090000_enterprise_session_delivery",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260814090000_enterprise_session_delivery.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260815100000_transcript_content_revision",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "transcripts",
        },
        sql: include_str!("../migrations/20260815100000_transcript_content_revision.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260815100100_transcript_live_deltas",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260815100100_transcript_live_deltas.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260815100200_search_index_acknowledgements",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260815100200_search_index_acknowledgements.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260815100300_e2ee_record_payload_hash",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "e2ee_records",
        },
        sql: include_str!("../migrations/20260815100300_e2ee_record_payload_hash.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260815100400_e2ee_ciphertext_ownership",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260815100400_e2ee_ciphertext_ownership.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260816100000_e2ee_replica_payload_hashes",
        scope: anlg_db_migrate::MigrationScope::Plain,
        sql: include_str!("../migrations/20260816100000_e2ee_replica_payload_hashes.sql"),
    },
    anlg_db_migrate::MigrationStep {
        id: "20260816100100_e2ee_payload_hash_local_state",
        scope: anlg_db_migrate::MigrationScope::CloudsyncAlter {
            table_name: "e2ee_records",
        },
        sql: include_str!("../migrations/20260816100100_e2ee_payload_hash_local_state.sql"),
    },
];

pub fn schema() -> anlg_db_migrate::DbSchema {
    anlg_db_migrate::DbSchema {
        steps: APP_MIGRATION_STEPS,
        validate_cloudsync_table: cloudsync_alter_guard_required,
    }
}

const SHARED_SESSION_CACHE_MIGRATION_VERSION: i64 = 20260716173000;
const LEGACY_SHARED_SESSION_CACHE_CHECKSUM: &str = "4813db532e44e6db8a3ba85e0b248ff99a927ec40b6aa971210452b588bcb2361d3661bd23d045a32ac3a9fcbed99b4a";
const CURRENT_SHARED_SESSION_CACHE_CHECKSUM: &str = "84376d223c0b1ca3298810b101fef7f599d50267dca4e5693feaddd481e4a082b87a12d37360e11b94b3cb39e8c58dee";
const ATTACHMENT_TRANSFER_JOBS_MIGRATION_VERSION: i64 = 20260717150000;
const LEGACY_ATTACHMENT_TRANSFER_JOBS_CHECKSUM: &str = "fd846a54c653d8e737371a950747b442bba2da055566dbcd35907b36fe7829a3a09c81052346bb05100ba71be552efc0";
const CURRENT_ATTACHMENT_TRANSFER_JOBS_CHECKSUM: &str = "9250233e7b51b83bb74ef4e4af159a9c2bbc753ee64751cac8316968b66bb1b2e0e224770d8b63a631350183dc81c2e0";
const LEGACY_ATTACHMENT_TRANSFER_JOBS_SCHEMA_CHECKSUM: &str = "5702d2831ccfe75bfca957aa6d37cacc54347090c0d106aaaa9c3af124fcf7e57e49b0432ad5191ff3075f2d9d489916";
const CURRENT_ATTACHMENT_TRANSFER_JOBS_SCHEMA_CHECKSUM: &str = "3beb182b60f84e0f53ae6069fc7a6e4ba28e51e87d37e4fda7e5ab9b8e8ca713bd43bb8bd79100c6116a4d23dfff55ef";
const E2EE_PAYLOAD_HASH_LOCAL_STATE_MIGRATION_VERSION: i64 = 20260816100100;
const E2EE_REPLICA_PAYLOAD_HASHES_MIGRATION_VERSION: i64 = 20260816100000;
const CURRENT_E2EE_PAYLOAD_HASH_LOCAL_STATE_CHECKSUM: &str = "1e0da0d8b5a524adf12cf1bb5a9c3910f922c9c2e7feb9c1f2704c4b093af41b50519083546409fe55a0d9d9f6f12f5a";
const E2EE_PAYLOAD_HASH_LOCAL_STATE_ALTER: &str =
    "ALTER TABLE e2ee_records DROP COLUMN payload_hash;";

#[derive(Debug)]
pub enum AppSchemaError {
    Migrate(anlg_db_migrate::MigrateError),
    Sqlx(sqlx::Error),
    Cloudsync(anlg_cloudsync::Error),
    CloudsyncWorkspace(CloudsyncWorkspaceError),
    SharedSessionCacheRepair(&'static str),
    AttachmentTransferJobsRepair(&'static str),
    E2eePayloadHashLocalStateRepair(&'static str),
    LegacyMobileSchema(&'static str),
}

impl std::fmt::Display for AppSchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Migrate(error) => write!(f, "{error}"),
            Self::Sqlx(error) => write!(f, "{error}"),
            Self::Cloudsync(error) => write!(f, "{error}"),
            Self::CloudsyncWorkspace(error) => write!(f, "{error}"),
            Self::SharedSessionCacheRepair(error) => write!(f, "{error}"),
            Self::AttachmentTransferJobsRepair(error) => write!(f, "{error}"),
            Self::E2eePayloadHashLocalStateRepair(error) => write!(f, "{error}"),
            Self::LegacyMobileSchema(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for AppSchemaError {}

impl From<anlg_db_migrate::MigrateError> for AppSchemaError {
    fn from(error: anlg_db_migrate::MigrateError) -> Self {
        Self::Migrate(error)
    }
}

impl From<sqlx::Error> for AppSchemaError {
    fn from(error: sqlx::Error) -> Self {
        Self::Sqlx(error)
    }
}

impl From<CloudsyncWorkspaceError> for AppSchemaError {
    fn from(error: CloudsyncWorkspaceError) -> Self {
        Self::CloudsyncWorkspace(error)
    }
}

impl From<anlg_cloudsync::Error> for AppSchemaError {
    fn from(error: anlg_cloudsync::Error) -> Self {
        Self::Cloudsync(error)
    }
}

pub async fn prepare_schema(db: &anlg_db_core::Db) -> Result<(), AppSchemaError> {
    prepare_schema_with_progress(db, |_| {}).await
}

pub async fn prepare_schema_with_progress(
    db: &anlg_db_core::Db,
    on_migration_progress: impl FnMut(anlg_db_migrate::MigrationProgress) + Send,
) -> Result<(), AppSchemaError> {
    let templates_missing_before_migration = !templates_table_exists(db.pool()).await?;
    adopt_legacy_mobile_schema_migration(db.pool()).await?;
    repair_legacy_shared_session_cache_migration(db.pool()).await?;
    repair_legacy_attachment_transfer_jobs_migration(db.pool()).await?;
    repair_torn_e2ee_payload_hash_local_state_migration(db).await?;
    anlg_db_migrate::migrate_with_progress(db, schema(), on_migration_progress).await?;
    repair_missing_core_tables(db.pool(), templates_missing_before_migration).await?;
    backfill_session_share_activation(db.pool()).await?;
    ensure_cloudsync_workspace_binding(db.pool()).await?;
    Ok(())
}

const CALENDAR_EVENT_TOMBSTONES_MIGRATION_VERSION: i64 = 20260711000000;
const ATTACHMENT_CLOUD_SYNC_INTENT_MIGRATION_VERSION: i64 = 20260717170000;

async fn adopt_legacy_mobile_schema_migration(
    pool: &sqlx::SqlitePool,
) -> Result<(), AppSchemaError> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let user_version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&mut *transaction)
        .await?;
    if user_version != 1 {
        transaction.commit().await?;
        return Ok(());
    }

    let has_legacy_schema: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'sessions'
         )",
    )
    .fetch_one(&mut *transaction)
    .await?;
    if !has_legacy_schema {
        transaction.commit().await?;
        return Ok(());
    }

    sqlx::raw_sql(
        "CREATE TABLE IF NOT EXISTS _sqlx_migrations (
            version BIGINT PRIMARY KEY,
            description TEXT NOT NULL,
            installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            success BOOLEAN NOT NULL,
            checksum BLOB NOT NULL,
            execution_time BIGINT NOT NULL
        );",
    )
    .execute(&mut *transaction)
    .await?;

    let calendar_tombstones_present: bool = sqlx::query_scalar(
        "SELECT
            EXISTS(SELECT 1 FROM pragma_table_info('calendars') WHERE name = 'deleted_at')
            AND EXISTS(SELECT 1 FROM pragma_table_info('events') WHERE name = 'deleted_at')",
    )
    .fetch_one(&mut *transaction)
    .await?;
    if calendar_tombstones_present {
        adopt_legacy_mobile_migration(
            &mut transaction,
            CALENDAR_EVENT_TOMBSTONES_MIGRATION_VERSION,
            "calendar_event_tombstones",
            include_str!("../migrations/20260711000000_calendar_event_tombstones.sql"),
        )
        .await?;
    }

    let attachment_cloud_sync_intent_present: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM pragma_table_info('session_attachments')
            WHERE name = 'cloud_sync_enabled'
         )",
    )
    .fetch_one(&mut *transaction)
    .await?;
    if attachment_cloud_sync_intent_present {
        adopt_legacy_mobile_migration(
            &mut transaction,
            ATTACHMENT_CLOUD_SYNC_INTENT_MIGRATION_VERSION,
            "attachment_cloud_sync_intent",
            include_str!("../migrations/20260717170000_attachment_cloud_sync_intent.sql"),
        )
        .await?;
    }

    transaction.commit().await?;
    Ok(())
}

async fn adopt_legacy_mobile_migration(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    version: i64,
    description: &'static str,
    sql: &'static str,
) -> Result<(), AppSchemaError> {
    let expected_checksum = migration_checksum(sql);
    let existing: Option<(bool, Vec<u8>)> =
        sqlx::query_as("SELECT success, checksum FROM _sqlx_migrations WHERE version = ?")
            .bind(version)
            .fetch_optional(&mut **transaction)
            .await?;
    if let Some((success, checksum)) = existing {
        if !success || checksum != expected_checksum {
            return Err(AppSchemaError::LegacyMobileSchema(
                "legacy mobile migration history contains an unexpected record",
            ));
        }
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO _sqlx_migrations (
            version, description, success, checksum, execution_time
         ) VALUES (?, ?, 1, ?, 0)",
    )
    .bind(version)
    .bind(description)
    .bind(expected_checksum)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn backfill_session_share_activation(pool: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT OR IGNORE INTO session_share_activation (
                viewer_user_id,
                share_id,
                session_id,
                activated_at
             )
             SELECT
                viewer_user_id,
                share_id,
                session_id,
                cached_at
             FROM shared_session_cache
             WHERE manage_access = 1
               AND access_version > 1",
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn repair_legacy_attachment_transfer_jobs_migration(
    pool: &sqlx::SqlitePool,
) -> Result<(), AppSchemaError> {
    let migration_table_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type = 'table' AND name = '_sqlx_migrations'
        )",
    )
    .fetch_one(pool)
    .await?;
    if !migration_table_exists {
        return Ok(());
    }

    let checksum: Option<String> = sqlx::query_scalar(
        "SELECT lower(hex(checksum))
         FROM _sqlx_migrations
         WHERE version = ? AND success = 1",
    )
    .bind(ATTACHMENT_TRANSFER_JOBS_MIGRATION_VERSION)
    .fetch_optional(pool)
    .await?;
    match checksum.as_deref() {
        None | Some(CURRENT_ATTACHMENT_TRANSFER_JOBS_CHECKSUM) => return Ok(()),
        Some(LEGACY_ATTACHMENT_TRANSFER_JOBS_CHECKSUM) => {}
        Some(_) => {
            return Err(AppSchemaError::AttachmentTransferJobsRepair(
                "attachment-transfer jobs migration has an unknown checksum",
            ));
        }
    }

    let current_migration_checksum = migration_checksum(include_str!(
        "../migrations/20260717150000_attachment_transfer_jobs.sql"
    ));
    if checksum_hex(&current_migration_checksum) != CURRENT_ATTACHMENT_TRANSFER_JOBS_CHECKSUM {
        return Err(AppSchemaError::AttachmentTransferJobsRepair(
            "compiled attachment-transfer jobs migration has an unexpected checksum",
        ));
    }

    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let locked_checksum: Option<String> = sqlx::query_scalar(
        "SELECT lower(hex(checksum))
         FROM _sqlx_migrations
         WHERE version = ? AND success = 1",
    )
    .bind(ATTACHMENT_TRANSFER_JOBS_MIGRATION_VERSION)
    .fetch_optional(&mut *transaction)
    .await?;
    match locked_checksum.as_deref() {
        Some(LEGACY_ATTACHMENT_TRANSFER_JOBS_CHECKSUM) => {}
        Some(CURRENT_ATTACHMENT_TRANSFER_JOBS_CHECKSUM) => {
            transaction.commit().await?;
            return Ok(());
        }
        _ => {
            return Err(AppSchemaError::AttachmentTransferJobsRepair(
                "attachment-transfer jobs migration changed during repair",
            ));
        }
    }

    let legacy_schema_checksum = attachment_transfer_jobs_schema_checksum(&mut transaction).await?;
    if legacy_schema_checksum != LEGACY_ATTACHMENT_TRANSFER_JOBS_SCHEMA_CHECKSUM {
        return Err(AppSchemaError::AttachmentTransferJobsRepair(
            "attachment-transfer jobs migration has an unexpected schema",
        ));
    }

    // Only the queue's uniqueness rules changed; keep every transfer job intact.
    sqlx::raw_sql(
        "DROP INDEX idx_attachment_transfer_jobs_delete_object;
         DROP INDEX idx_attachment_transfer_jobs_upload_object_id;
         DROP INDEX idx_attachment_transfer_jobs_delete_object_id;

         CREATE UNIQUE INDEX idx_attachment_transfer_jobs_delete_object
         ON attachment_transfer_jobs(object_key)
         WHERE direction = 'delete' AND phase <> 'completed';

         CREATE UNIQUE INDEX idx_attachment_transfer_jobs_upload_object_id
         ON attachment_transfer_jobs(remote_object_id)
         WHERE direction = 'upload'
           AND remote_object_id <> ''
           AND phase <> 'completed';

         CREATE UNIQUE INDEX idx_attachment_transfer_jobs_delete_object_id
         ON attachment_transfer_jobs(remote_object_id)
         WHERE direction = 'delete'
           AND remote_object_id <> ''
           AND phase <> 'completed';",
    )
    .execute(&mut *transaction)
    .await?;

    let current_schema_checksum =
        attachment_transfer_jobs_schema_checksum(&mut transaction).await?;
    if current_schema_checksum != CURRENT_ATTACHMENT_TRANSFER_JOBS_SCHEMA_CHECKSUM {
        return Err(AppSchemaError::AttachmentTransferJobsRepair(
            "attachment-transfer jobs repair produced an unexpected schema",
        ));
    }

    let result = sqlx::query(
        "UPDATE _sqlx_migrations
         SET checksum = ?
         WHERE version = ? AND success = 1 AND lower(hex(checksum)) = ?",
    )
    .bind(current_migration_checksum.as_slice())
    .bind(ATTACHMENT_TRANSFER_JOBS_MIGRATION_VERSION)
    .bind(LEGACY_ATTACHMENT_TRANSFER_JOBS_CHECKSUM)
    .execute(&mut *transaction)
    .await?;
    if result.rows_affected() != 1 {
        return Err(AppSchemaError::AttachmentTransferJobsRepair(
            "attachment-transfer jobs migration changed during repair",
        ));
    }

    transaction.commit().await?;
    Ok(())
}

async fn attachment_transfer_jobs_schema_checksum(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> Result<String, sqlx::Error> {
    let objects: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT type, name, sql
         FROM sqlite_master
         WHERE sql IS NOT NULL
           AND (
             (type = 'table' AND name = 'attachment_transfer_jobs')
             OR (
               type IN ('index', 'trigger')
               AND tbl_name = 'attachment_transfer_jobs'
             )
           )
         ORDER BY type, name",
    )
    .fetch_all(&mut **transaction)
    .await?;
    let mut schema = String::new();
    for (object_type, name, sql) in objects {
        schema.push_str(&object_type);
        schema.push('\n');
        schema.push_str(&name);
        schema.push('\n');
        schema.push_str(&normalize_schema_sql(&sql));
        schema.push('\n');
    }

    Ok(checksum_hex(&migration_checksum(&schema)))
}

fn normalize_schema_sql(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}

async fn repair_legacy_shared_session_cache_migration(
    pool: &sqlx::SqlitePool,
) -> Result<(), AppSchemaError> {
    let migration_table_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type = 'table' AND name = '_sqlx_migrations'
        )",
    )
    .fetch_one(pool)
    .await?;
    if !migration_table_exists {
        return Ok(());
    }

    let checksum: Option<String> = sqlx::query_scalar(
        "SELECT lower(hex(checksum))
         FROM _sqlx_migrations
         WHERE version = ? AND success = 1",
    )
    .bind(SHARED_SESSION_CACHE_MIGRATION_VERSION)
    .fetch_optional(pool)
    .await?;
    match checksum.as_deref() {
        None | Some(CURRENT_SHARED_SESSION_CACHE_CHECKSUM) => return Ok(()),
        Some(LEGACY_SHARED_SESSION_CACHE_CHECKSUM) => {}
        Some(_) => {
            return Err(AppSchemaError::SharedSessionCacheRepair(
                "shared-session cache migration has an unknown checksum",
            ));
        }
    }

    let current_migration_checksum = migration_checksum(include_str!(
        "../migrations/20260716173000_shared_session_cache.sql"
    ));
    if checksum_hex(&current_migration_checksum) != CURRENT_SHARED_SESSION_CACHE_CHECKSUM {
        return Err(AppSchemaError::SharedSessionCacheRepair(
            "compiled shared-session cache migration has an unexpected checksum",
        ));
    }

    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let locked_checksum: Option<String> = sqlx::query_scalar(
        "SELECT lower(hex(checksum))
         FROM _sqlx_migrations
         WHERE version = ? AND success = 1",
    )
    .bind(SHARED_SESSION_CACHE_MIGRATION_VERSION)
    .fetch_optional(&mut *transaction)
    .await?;
    match locked_checksum.as_deref() {
        Some(LEGACY_SHARED_SESSION_CACHE_CHECKSUM) => {}
        Some(CURRENT_SHARED_SESSION_CACHE_CHECKSUM) => {
            transaction.commit().await?;
            return Ok(());
        }
        _ => {
            return Err(AppSchemaError::SharedSessionCacheRepair(
                "shared-session cache migration changed during repair",
            ));
        }
    }

    let table_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type = 'table' AND name = 'shared_session_cache'
        )",
    )
    .fetch_one(&mut *transaction)
    .await?;
    let has_viewer_user_id = table_exists
        && shared_session_cache_column_exists(&mut transaction, "viewer_user_id").await?;
    let share_id_is_primary_key: bool = if table_exists {
        sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM pragma_table_info('shared_session_cache')
                WHERE name = 'share_id' AND pk = 1
            )",
        )
        .fetch_one(&mut *transaction)
        .await?
    } else {
        false
    };
    if !table_exists || has_viewer_user_id || !share_id_is_primary_key {
        return Err(AppSchemaError::SharedSessionCacheRepair(
            "legacy shared-session cache migration has an unexpected schema",
        ));
    }

    let attachments_checksum = applied_migration_checksum(&mut transaction, 20260717172000).await?;
    let expected_attachments_checksum = migration_checksum(include_str!(
        "../migrations/20260717172000_shared_session_cache_attachments.sql"
    ));
    if attachments_checksum
        .as_deref()
        .is_some_and(|checksum| checksum != expected_attachments_checksum.as_slice())
    {
        return Err(AppSchemaError::SharedSessionCacheRepair(
            "shared-session cache attachment migration has an unknown checksum",
        ));
    }
    let attachments_applied = attachments_checksum.is_some();
    let attachments_present =
        shared_session_cache_column_exists(&mut transaction, "attachments_json").await?;
    let web_edits_checksum = applied_migration_checksum(&mut transaction, 20260717190000).await?;
    let expected_web_edits_checksum = migration_checksum(include_str!(
        "../migrations/20260717190000_shared_session_cache_web_edits.sql"
    ));
    if web_edits_checksum
        .as_deref()
        .is_some_and(|checksum| checksum != expected_web_edits_checksum.as_slice())
    {
        return Err(AppSchemaError::SharedSessionCacheRepair(
            "shared-session cache web-edit migration has an unknown checksum",
        ));
    }
    let web_edits_applied = web_edits_checksum.is_some();
    let web_edit_columns = [
        "web_editable",
        "web_edit_base_content_revision",
        "web_edit_base_title",
        "web_edit_base_body_json",
    ];
    let mut web_edit_columns_present = Vec::with_capacity(web_edit_columns.len());
    for column in web_edit_columns {
        web_edit_columns_present
            .push(shared_session_cache_column_exists(&mut transaction, column).await?);
    }
    if attachments_applied != attachments_present
        || web_edit_columns_present
            .iter()
            .any(|present| *present != web_edits_applied)
    {
        return Err(AppSchemaError::SharedSessionCacheRepair(
            "legacy shared-session cache migration history does not match its schema",
        ));
    }

    // The legacy cache has no viewer identity, so its derived rows cannot be
    // assigned to an account safely. Rebuild it and let sync repopulate it.
    sqlx::query("DROP TABLE shared_session_cache")
        .execute(&mut *transaction)
        .await?;
    sqlx::raw_sql(include_str!(
        "../migrations/20260716173000_shared_session_cache.sql"
    ))
    .execute(&mut *transaction)
    .await?;
    if attachments_applied {
        sqlx::raw_sql(include_str!(
            "../migrations/20260717172000_shared_session_cache_attachments.sql"
        ))
        .execute(&mut *transaction)
        .await?;
    }
    if web_edits_applied {
        sqlx::raw_sql(include_str!(
            "../migrations/20260717190000_shared_session_cache_web_edits.sql"
        ))
        .execute(&mut *transaction)
        .await?;
    }

    let result = sqlx::query(
        "UPDATE _sqlx_migrations
         SET checksum = ?
         WHERE version = ? AND lower(hex(checksum)) = ?",
    )
    .bind(current_migration_checksum.as_slice())
    .bind(SHARED_SESSION_CACHE_MIGRATION_VERSION)
    .bind(LEGACY_SHARED_SESSION_CACHE_CHECKSUM)
    .execute(&mut *transaction)
    .await?;
    if result.rows_affected() != 1 {
        return Err(AppSchemaError::SharedSessionCacheRepair(
            "legacy shared-session cache migration changed during repair",
        ));
    }

    transaction.commit().await?;
    Ok(())
}

// Builds of 1.4.10 without the transactional CloudsyncAlter runner executed
// this migration statement-by-statement with autocommit, so a force-quit after
// the ALTER left the column dropped, some views/triggers missing, and no
// history row. The re-run then dies on the ALTER with "no such column".
async fn repair_torn_e2ee_payload_hash_local_state_migration(
    db: &anlg_db_core::Db,
) -> Result<(), AppSchemaError> {
    let pool = db.pool();
    let migration_table_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type = 'table' AND name = '_sqlx_migrations'
        )",
    )
    .fetch_one(pool)
    .await?;
    if !migration_table_exists {
        return Ok(());
    }
    if !e2ee_payload_hash_migration_is_torn(pool).await? {
        return Ok(());
    }

    let migration_sql =
        include_str!("../migrations/20260816100100_e2ee_payload_hash_local_state.sql");
    let current_migration_checksum = migration_checksum(migration_sql);
    if checksum_hex(&current_migration_checksum) != CURRENT_E2EE_PAYLOAD_HASH_LOCAL_STATE_CHECKSUM {
        return Err(AppSchemaError::E2eePayloadHashLocalStateRepair(
            "compiled e2ee payload-hash migration has an unexpected checksum",
        ));
    }

    // The replica-hash triggers are created without IF EXISTS in the shipped
    // migration; the torn run may have gotten far enough to create them.
    let repair_sql = format!(
        "DROP TRIGGER IF EXISTS e2ee_replica_payload_hash_insert;
         DROP TRIGGER IF EXISTS e2ee_replica_payload_hash_update;
         DROP TRIGGER IF EXISTS e2ee_replica_payload_hash_delete;
         {}",
        migration_sql.replace(E2EE_PAYLOAD_HASH_LOCAL_STATE_ALTER, ""),
    );

    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    if !e2ee_payload_hash_migration_is_torn(&mut *transaction).await? {
        transaction.commit().await?;
        return Ok(());
    }

    // The torn run died after cloudsync_begin_alter and the committed DROP
    // COLUMN but before cloudsync_commit_alter, so the CloudSync schema hash
    // for e2ee_records still describes the dropped column. Redo the alter
    // window around the repair so commit_alter records the current schema;
    // otherwise sync fails with a schema-hash mismatch after the unbrick.
    let cloudsync_alter = db.cloudsync_enabled()
        && anlg_db_core::cloudsync_is_enabled_on(&mut *transaction, "e2ee_records").await?;
    if cloudsync_alter {
        anlg_db_core::cloudsync_begin_alter_on(&mut *transaction, "e2ee_records").await?;
    }

    sqlx::raw_sql(sqlx::AssertSqlSafe(repair_sql.as_str()))
        .execute(&mut *transaction)
        .await?;

    sqlx::query(
        "INSERT INTO _sqlx_migrations (
            version, description, success, checksum, execution_time
         ) VALUES (?, ?, 1, ?, 0)",
    )
    .bind(E2EE_PAYLOAD_HASH_LOCAL_STATE_MIGRATION_VERSION)
    .bind("e2ee_payload_hash_local_state")
    .bind(current_migration_checksum.as_slice())
    .execute(&mut *transaction)
    .await?;

    if cloudsync_alter {
        anlg_db_core::cloudsync_commit_alter_on(&mut *transaction, "e2ee_records").await?;
    }

    transaction.commit().await?;
    Ok(())
}

async fn e2ee_payload_hash_migration_is_torn<'e, E>(executor: E) -> Result<bool, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_scalar(
        "SELECT NOT EXISTS(
            SELECT 1 FROM _sqlx_migrations WHERE version = ?
        )
        AND EXISTS(
            SELECT 1 FROM _sqlx_migrations WHERE version = ? AND success = 1
        )
        AND EXISTS(
            SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'e2ee_records'
        )
        AND NOT EXISTS(
            SELECT 1 FROM pragma_table_info('e2ee_records') WHERE name = 'payload_hash'
        )",
    )
    .bind(E2EE_PAYLOAD_HASH_LOCAL_STATE_MIGRATION_VERSION)
    .bind(E2EE_REPLICA_PAYLOAD_HASHES_MIGRATION_VERSION)
    .fetch_one(executor)
    .await
}

async fn shared_session_cache_column_exists(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    column: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM pragma_table_info('shared_session_cache')
            WHERE name = ?
        )",
    )
    .bind(column)
    .fetch_one(&mut **transaction)
    .await
}

async fn applied_migration_checksum(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    version: i64,
) -> Result<Option<Vec<u8>>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT checksum FROM _sqlx_migrations
         WHERE version = ? AND success = 1",
    )
    .bind(version)
    .fetch_optional(&mut **transaction)
    .await
}

fn migration_checksum(sql: &str) -> Vec<u8> {
    Sha384::digest(sql.as_bytes()).to_vec()
}

fn checksum_hex(checksum: &[u8]) -> String {
    use std::fmt::Write as _;

    checksum.iter().fold(
        String::with_capacity(checksum.len() * 2),
        |mut hex, byte| {
            write!(hex, "{byte:02x}").expect("writing to a string cannot fail");
            hex
        },
    )
}

async fn templates_table_exists(pool: &sqlx::SqlitePool) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1
            FROM sqlite_master
            WHERE type = 'table' AND name = 'templates'
        )",
    )
    .fetch_one(pool)
    .await
}

async fn repair_missing_core_tables(
    pool: &sqlx::SqlitePool,
    templates_missing_before_migration: bool,
) -> Result<(), sqlx::Error> {
    if !templates_table_exists(pool).await? {
        sqlx::query(include_str!("../migrations/20260413020000_templates.sql"))
            .execute(pool)
            .await?;
    }

    let has_icon_json = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('templates') WHERE name = 'icon_json')",
    )
    .fetch_one(pool)
    .await?;
    if !has_icon_json {
        sqlx::query(include_str!(
            "../migrations/20260712170000_template_icons.sql"
        ))
        .execute(pool)
        .await?;
    }

    if templates_missing_before_migration {
        sqlx::query(include_str!(
            "../migrations/20260524000000_default_templates.sql"
        ))
        .execute(pool)
        .await?;
    }

    Ok(())
}

#[cfg(test)]
mod schema_tests;
