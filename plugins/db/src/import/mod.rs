mod calendars;
mod events;
mod legacy_vault;
mod templates;

use std::path::PathBuf;

use sqlx::SqlitePool;

pub async fn import_legacy_data<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    pool: &SqlitePool,
) -> crate::Result<()> {
    if !legacy_import_required(pool).await? {
        return Ok(());
    }

    let vault_base = resolve_startup_vault_base(app)?;
    let run_id = legacy_vault::import_legacy_vault(pool, &vault_base, false).await?;
    require_verified_import(pool, &run_id).await
}

async fn require_verified_import(pool: &SqlitePool, run_id: &str) -> crate::Result<()> {
    if legacy_import_required(pool).await? {
        return Err(std::io::Error::other(format!(
            "legacy import {run_id} did not pass parity verification; source files were left unchanged",
        ))
        .into());
    }

    Ok(())
}

async fn legacy_import_required(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let verified: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1
           FROM storage_migration_state
           WHERE id = 'legacy_v1'
             AND importer_version = ?
             AND parity_verified = 1
         )",
    )
    .bind(hypr_db_app::LEGACY_IMPORTER_VERSION)
    .fetch_one(pool)
    .await?;

    Ok(!verified)
}

pub async fn rerun_legacy_import(pool: &SqlitePool, dry_run: bool) -> crate::Result<String> {
    let source_root = sqlx::query_scalar::<_, String>(
        "SELECT source_root
         FROM migration_import_runs
         WHERE dry_run = 0 AND source_root <> ''
         ORDER BY started_at DESC
         LIMIT 1",
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| std::io::Error::other("no legacy import source has been recorded"))?;

    legacy_vault::import_legacy_vault(pool, std::path::Path::new(&source_root), dry_run).await
}

pub async fn get_legacy_import_report(
    pool: &SqlitePool,
) -> crate::Result<crate::LegacyImportReport> {
    let state = sqlx::query_as::<_, crate::StorageMigrationState>(
        "SELECT phase, latest_run_id, parity_verified, cutover_at, rollback_until, last_error, updated_at
         FROM storage_migration_state
         WHERE id = 'legacy_v1'",
    )
    .fetch_one(pool)
    .await?;

    let latest_run = if state.latest_run_id.is_empty() {
        None
    } else {
        sqlx::query_as::<_, crate::LegacyImportRun>(
            "SELECT id, importer_version, source_root, dry_run, status, discovered_count,
                    imported_count, matched_count, skipped_count, conflict_count, error_count, started_at,
                    completed_at, error
             FROM migration_import_runs
             WHERE id = ?",
        )
        .bind(&state.latest_run_id)
        .fetch_optional(pool)
        .await?
    };

    let items = if state.latest_run_id.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, crate::LegacyImportItemReport>(
            "SELECT source_path, source_kind, source_sha256, status, discovered_count,
                    imported_count, matched_count, skipped_count, conflict_count, error
             FROM migration_import_items
             WHERE run_id = ?
             ORDER BY source_path",
        )
        .bind(&state.latest_run_id)
        .fetch_all(pool)
        .await?
    };

    let targets = if state.latest_run_id.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as::<_, crate::LegacyImportTargetReport>(
            "SELECT source_path, table_name, target_id, status, error
             FROM migration_import_targets
             WHERE run_id = ?
             ORDER BY table_name, target_id, source_path",
        )
        .bind(&state.latest_run_id)
        .fetch_all(pool)
        .await?
    };

    Ok(crate::LegacyImportReport {
        state,
        latest_run,
        items,
        targets,
    })
}

fn resolve_startup_vault_base<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> crate::Result<PathBuf> {
    let bundle_id: &str = app.config().identifier.as_ref();
    let settings_base = hypr_storage::global::compute_default_base(bundle_id)
        .ok_or(std::io::Error::other("settings base unavailable"))?;
    std::fs::create_dir_all(&settings_base)?;

    Ok(hypr_storage::vault::resolve_base(
        &settings_base,
        &settings_base,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn verified_current_import_is_not_repeated_at_startup() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();

        assert!(legacy_import_required(db.pool()).await.unwrap());

        sqlx::query(
            "UPDATE storage_migration_state
             SET importer_version = ?, parity_verified = 1
             WHERE id = 'legacy_v1'",
        )
        .bind(hypr_db_app::LEGACY_IMPORTER_VERSION)
        .execute(db.pool())
        .await
        .unwrap();

        assert!(!legacy_import_required(db.pool()).await.unwrap());
        require_verified_import(db.pool(), "verified-run")
            .await
            .unwrap();

        sqlx::query(
            "UPDATE storage_migration_state
             SET importer_version = importer_version - 1
             WHERE id = 'legacy_v1'",
        )
        .execute(db.pool())
        .await
        .unwrap();

        assert!(legacy_import_required(db.pool()).await.unwrap());
    }

    #[tokio::test]
    async fn incomplete_import_prevents_cutover() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();

        let error = require_verified_import(db.pool(), "run-with-errors")
            .await
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("did not pass parity verification")
        );
        assert!(
            error
                .to_string()
                .contains("source files were left unchanged")
        );
    }
}
