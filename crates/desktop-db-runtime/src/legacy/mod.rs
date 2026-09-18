pub mod cleanup;

pub use cleanup::{execute as cleanup_legacy_files, get_status as get_legacy_cleanup_status};

pub async fn legacy_migration_ready(
    pool: &sqlx::SqlitePool,
) -> std::result::Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1
           FROM storage_migration_state AS state
           LEFT JOIN migration_import_runs AS run ON run.id = state.latest_run_id
           WHERE state.id = 'legacy_v1'
             AND (
               (state.importer_version = ? AND state.parity_verified = 1)
               OR (
                 run.importer_version = ?
                 AND run.dry_run = 0
                 AND run.status = 'completed_with_conflicts'
                 AND run.conflict_count > 0
                 AND run.skipped_count = 0
                 AND run.error_count = 0
               )
             )
         )",
    )
    .bind(anlg_db_app::LEGACY_IMPORTER_VERSION)
    .bind(anlg_db_app::LEGACY_IMPORTER_VERSION)
    .fetch_one(pool)
    .await
}
