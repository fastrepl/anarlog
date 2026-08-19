use sqlx::{Row, SqlitePool};

use super::{LEGACY_IMPORTER_VERSION, LegacyImportItem};

pub async fn begin_legacy_import_run(
    pool: &SqlitePool,
    run_id: &str,
    source_root: &str,
    dry_run: bool,
) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "INSERT INTO migration_import_runs \
         (id, importer_version, source_root, dry_run, status) \
         VALUES (?, ?, ?, ?, 'running')",
    )
    .bind(run_id)
    .bind(LEGACY_IMPORTER_VERSION)
    .bind(source_root)
    .bind(dry_run)
    .execute(&mut *transaction)
    .await?;

    if !dry_run {
        sqlx::query(
            "UPDATE storage_migration_state
             SET phase = 'shadow',
                 latest_run_id = ?,
                 parity_verified = 0,
                 last_error = '',
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = 'legacy_v1'",
        )
        .bind(run_id)
        .execute(&mut *transaction)
        .await?;
    }

    transaction.commit().await?;

    Ok(())
}

pub async fn legacy_source_already_imported(
    pool: &SqlitePool,
    source_path: &str,
    source_sha256: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1
           FROM migration_import_items AS item
           JOIN migration_import_runs AS run ON run.id = item.run_id
           WHERE item.source_path = ?
             AND item.source_sha256 = ?
             AND item.status IN ('complete', 'unchanged')
             AND run.importer_version = ?
             AND run.dry_run = 0
         )",
    )
    .bind(source_path)
    .bind(source_sha256)
    .bind(LEGACY_IMPORTER_VERSION)
    .fetch_one(pool)
    .await
}

pub async fn record_legacy_import_unchanged(
    pool: &SqlitePool,
    item: LegacyImportItem<'_>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO migration_import_items \
         (id, run_id, source_path, source_kind, source_sha256, status, discovered_count, \
          imported_count, matched_count, skipped_count, conflict_count, error, completed_at) \
         SELECT ?, ?, previous.source_path, previous.source_kind, previous.source_sha256, \
                'unchanged', previous.discovered_count, 0, previous.discovered_count, 0, 0, '', \
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         FROM migration_import_items AS previous \
         JOIN migration_import_runs AS previous_run ON previous_run.id = previous.run_id \
         WHERE previous.source_path = ? \
           AND previous.source_sha256 = ? \
           AND previous.status IN ('complete', 'unchanged') \
           AND previous_run.importer_version = ? \
           AND previous_run.dry_run = 0 \
         ORDER BY previous.created_at DESC \
         LIMIT 1",
    )
    .bind(item.id)
    .bind(item.run_id)
    .bind(item.source_path)
    .bind(item.source_sha256)
    .bind(LEGACY_IMPORTER_VERSION)
    .execute(pool)
    .await?;

    if result.rows_affected() > 0 {
        sqlx::query(
            "INSERT INTO migration_import_targets \
             (id, run_id, item_id, source_path, source_kind, table_name, target_id, status) \
             SELECT DISTINCT ? || ':' || previous.table_name || ':' || previous.target_id, \
                    ?, ?, ?, ?, previous.table_name, previous.target_id, 'unchanged' \
             FROM migration_import_targets AS previous \
             JOIN migration_import_items AS previous_item ON previous_item.id = previous.item_id \
             JOIN migration_import_runs AS previous_run ON previous_run.id = previous.run_id \
             WHERE previous_item.source_path = ? \
               AND previous_item.source_sha256 = ? \
               AND previous_item.status IN ('complete', 'unchanged') \
               AND previous_run.importer_version = ? \
               AND previous_run.dry_run = 0 \
             ORDER BY previous.created_at DESC",
        )
        .bind(item.id)
        .bind(item.run_id)
        .bind(item.id)
        .bind(item.source_path)
        .bind(item.source_kind)
        .bind(item.source_path)
        .bind(item.source_sha256)
        .bind(LEGACY_IMPORTER_VERSION)
        .execute(pool)
        .await?;
    }

    Ok(result.rows_affected() > 0)
}

pub async fn record_legacy_import_error(
    pool: &SqlitePool,
    item: LegacyImportItem<'_>,
    error: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO migration_import_items \
         (id, run_id, source_path, source_kind, source_sha256, status, \
          discovered_count, skipped_count, error, completed_at) \
         VALUES (?, ?, ?, ?, ?, 'error', 1, 1, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
    )
    .bind(item.id)
    .bind(item.run_id)
    .bind(item.source_path)
    .bind(item.source_kind)
    .bind(item.source_sha256)
    .bind(error)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn finish_legacy_import_run(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<String, sqlx::Error> {
    // One transaction: the run outcome and the migration-state control
    // pointer must never diverge if the process dies between the writes.
    let mut transaction = pool.begin().await?;
    let aggregate = sqlx::query(
        "SELECT
           COALESCE(SUM(discovered_count), 0) AS discovered_count,
           COALESCE(SUM(imported_count), 0) AS imported_count,
           COALESCE(SUM(matched_count), 0) AS matched_count,
           COALESCE(SUM(skipped_count), 0) AS skipped_count,
           COALESCE(SUM(conflict_count), 0) AS conflict_count,
           COALESCE(SUM(CASE WHEN status IN ('error', 'partial') THEN 1 ELSE 0 END), 0) AS error_count
         FROM migration_import_items
         WHERE run_id = ?",
    )
    .bind(run_id)
    .fetch_one(&mut *transaction)
    .await?;

    let skipped_count = aggregate.get::<i64, _>("skipped_count");
    let conflict_count = aggregate.get::<i64, _>("conflict_count");
    let error_count = aggregate.get::<i64, _>("error_count");
    let status = if skipped_count > 0 || error_count > 0 {
        "completed_with_issues"
    } else if conflict_count > 0 {
        "completed_with_conflicts"
    } else {
        "completed"
    };

    sqlx::query(
        "UPDATE migration_import_runs
         SET status = ?,
             discovered_count = ?,
             imported_count = ?,
             matched_count = ?,
             skipped_count = ?,
             conflict_count = ?,
             error_count = ?,
             completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(status)
    .bind(aggregate.get::<i64, _>("discovered_count"))
    .bind(aggregate.get::<i64, _>("imported_count"))
    .bind(aggregate.get::<i64, _>("matched_count"))
    .bind(skipped_count)
    .bind(conflict_count)
    .bind(error_count)
    .bind(run_id)
    .execute(&mut *transaction)
    .await?;

    sqlx::query(
        "UPDATE storage_migration_state
         SET latest_run_id = ?,
             importer_version = ?,
             parity_verified = CASE WHEN ? = 'completed' THEN 1 ELSE 0 END,
             last_error = CASE WHEN ? = 'completed' THEN '' ELSE ? END,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = 'legacy_v1'
           AND EXISTS (
             SELECT 1 FROM migration_import_runs WHERE id = ? AND dry_run = 0
           )",
    )
    .bind(run_id)
    .bind(LEGACY_IMPORTER_VERSION)
    .bind(status)
    .bind(status)
    .bind(status)
    .bind(run_id)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(status.to_string())
}

pub async fn fail_legacy_import_run(
    pool: &SqlitePool,
    run_id: &str,
    error: &str,
) -> Result<(), sqlx::Error> {
    // One transaction, for the same reason as finish_legacy_import_run.
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "UPDATE migration_import_runs
         SET status = 'failed', error = ?, completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(error)
    .bind(run_id)
    .execute(&mut *transaction)
    .await?;

    sqlx::query(
        "UPDATE storage_migration_state
         SET latest_run_id = ?, last_error = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = 'legacy_v1'
           AND EXISTS (
             SELECT 1 FROM migration_import_runs WHERE id = ? AND dry_run = 0
           )",
    )
    .bind(run_id)
    .bind(error)
    .bind(run_id)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(())
}
