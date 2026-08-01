use sqlx::{Sqlite, SqlitePool, Transaction};

mod conflict;
mod run;
mod types;

pub use run::*;
pub use types::*;

use conflict::{reconcile_content_conflict, row_matches_existing};

pub async fn apply_legacy_import_item(
    pool: &SqlitePool,
    item: LegacyImportItem<'_>,
    batch: &LegacyImportBatch,
    dry_run: bool,
) -> Result<LegacyImportItemResult, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let mut imported_count = 0_i64;
    let mut matched_count = 0_i64;
    let mut conflict_count = 0_i64;
    let mut missing_dependency_count = 0_usize;

    for row in &batch.rows {
        let outcome = if dry_run {
            InsertOutcome::DryRun
        } else {
            insert_row_if_missing(&mut transaction, row).await?
        };
        match outcome {
            InsertOutcome::Inserted => imported_count += 1,
            InsertOutcome::Matched
            | InsertOutcome::FilledFromLegacy
            | InsertOutcome::RetainedExisting => matched_count += 1,
            InsertOutcome::Conflict => conflict_count += 1,
            InsertOutcome::MissingDependency => missing_dependency_count += 1,
            InsertOutcome::DryRun => {}
        }
        record_import_target(&mut transaction, &item, row, outcome).await?;
    }

    let discovered_count = i64::try_from(batch.rows.len()).unwrap_or(i64::MAX)
        + i64::try_from(batch.skipped_count).unwrap_or(i64::MAX);
    let skipped_count = i64::try_from(batch.skipped_count.saturating_add(missing_dependency_count))
        .unwrap_or(i64::MAX);
    let status = if dry_run {
        "dry_run"
    } else if skipped_count > 0 || !batch.warning.is_empty() {
        "partial"
    } else if conflict_count > 0 {
        "conflict"
    } else {
        "complete"
    };

    sqlx::query(
        "INSERT INTO migration_import_items \
         (id, run_id, source_path, source_kind, source_sha256, status, \
          discovered_count, imported_count, matched_count, skipped_count, conflict_count, error, completed_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
    )
    .bind(item.id)
    .bind(item.run_id)
    .bind(item.source_path)
    .bind(item.source_kind)
    .bind(item.source_sha256)
    .bind(status)
    .bind(discovered_count)
    .bind(imported_count)
    .bind(matched_count)
    .bind(skipped_count)
    .bind(conflict_count)
    .bind(&batch.warning)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(LegacyImportItemResult {
        discovered_count,
        imported_count,
        matched_count,
        skipped_count,
        conflict_count,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InsertOutcome {
    Inserted,
    Matched,
    FilledFromLegacy,
    RetainedExisting,
    Conflict,
    MissingDependency,
    DryRun,
}

impl InsertOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Inserted => "inserted",
            Self::Matched => "matched",
            Self::FilledFromLegacy => "filled_from_legacy",
            Self::RetainedExisting => "retained_existing",
            Self::Conflict => "conflict",
            Self::MissingDependency => "missing_dependency",
            Self::DryRun => "dry_run",
        }
    }
}

async fn record_import_target(
    transaction: &mut Transaction<'_, Sqlite>,
    item: &LegacyImportItem<'_>,
    row: &LegacyImportRow,
    outcome: InsertOutcome,
) -> Result<(), sqlx::Error> {
    let id = format!("{}:{}:{}", item.id, row.table_name(), row.id());
    let status = match outcome {
        InsertOutcome::Inserted | InsertOutcome::Matched | InsertOutcome::FilledFromLegacy => {
            row.recovery_status().unwrap_or(outcome.as_str())
        }
        _ => outcome.as_str(),
    };
    sqlx::query(
        "INSERT INTO migration_import_targets \
         (id, run_id, item_id, source_path, source_kind, table_name, target_id, status) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(item.run_id)
    .bind(item.id)
    .bind(item.source_path)
    .bind(item.source_kind)
    .bind(row.table_name())
    .bind(row.id())
    .bind(status)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

async fn insert_row_if_missing(
    transaction: &mut Transaction<'_, Sqlite>,
    row: &LegacyImportRow,
) -> Result<InsertOutcome, sqlx::Error> {
    let result = match row {
        LegacyImportRow::Calendar(row) => sqlx::query(
            "INSERT INTO calendars \
             (id, tracking_id_calendar, name, enabled, provider, source, color, connection_id) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.tracking_id_calendar)
        .bind(&row.name)
        .bind(row.enabled)
        .bind(&row.provider)
        .bind(&row.source)
        .bind(&row.color)
        .bind(&row.connection_id)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::Event(row) => sqlx::query(
            "INSERT INTO events \
             (id, tracking_id_event, calendar_id, title, started_at, ended_at, location, \
              meeting_link, description, note, recurrence_series_id, has_recurrence_rules, \
              is_all_day, provider, participants_json) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.tracking_id_event)
        .bind(&row.calendar_id)
        .bind(&row.title)
        .bind(&row.started_at)
        .bind(&row.ended_at)
        .bind(&row.location)
        .bind(&row.meeting_link)
        .bind(&row.description)
        .bind(&row.note)
        .bind(&row.recurrence_series_id)
        .bind(row.has_recurrence_rules)
        .bind(row.is_all_day)
        .bind(&row.provider)
        .bind(&row.participants_json)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::Template(row) => sqlx::query(
            "INSERT INTO templates \
             (id, title, description, pinned, pin_order, category, targets_json, sections_json) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET \
               title = excluded.title, \
               description = excluded.description, \
               pinned = excluded.pinned, \
               pin_order = excluded.pin_order, \
               category = excluded.category, \
               targets_json = excluded.targets_json, \
               sections_json = excluded.sections_json, \
               updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE templates.id LIKE 'default-%' \
               AND templates.created_at = templates.updated_at",
        )
        .bind(&row.id)
        .bind(&row.title)
        .bind(&row.description)
        .bind(row.pinned)
        .bind(row.pin_order)
        .bind(&row.category)
        .bind(&row.targets_json)
        .bind(&row.sections_json)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::Organization(row) => sqlx::query(
            "INSERT INTO organizations \
             (id, workspace_id, owner_user_id, name, memo, pinned, pin_order, created_at, updated_at) \
             VALUES (?, NULLIF((SELECT json_extract(value_json, '$.workspace_id') \
               FROM app_settings WHERE id = 'cloudsync_workspace_binding'), ''), \
               ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.owner_user_id)
        .bind(&row.name)
        .bind(&row.memo)
        .bind(row.pinned)
        .bind(row.pin_order)
        .bind(&row.created_at)
        .bind(&row.created_at)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::Human(row) => sqlx::query(
            "INSERT INTO humans \
             (id, workspace_id, owner_user_id, organization_id, name, email, phone, job_title, \
              linkedin_username, memo, pinned, pin_order, created_at, updated_at) \
             VALUES (?, NULLIF((SELECT json_extract(value_json, '$.workspace_id') \
               FROM app_settings WHERE id = 'cloudsync_workspace_binding'), ''), \
               ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.owner_user_id)
        .bind(&row.organization_id)
        .bind(&row.name)
        .bind(&row.email)
        .bind(&row.phone)
        .bind(&row.job_title)
        .bind(&row.linkedin_username)
        .bind(&row.memo)
        .bind(row.pinned)
        .bind(row.pin_order)
        .bind(&row.created_at)
        .bind(&row.created_at)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::Session(row) => sqlx::query(
            "INSERT INTO sessions \
             (id, workspace_id, owner_user_id, title, created_at, updated_at, started_at, \
              ended_at, event_id, external_event_id, external_provider, series_id, event_json, \
              metadata_json, folder_path) \
             VALUES (?, NULLIF((SELECT json_extract(value_json, '$.workspace_id') \
               FROM app_settings WHERE id = 'cloudsync_workspace_binding'), ''), \
               ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.owner_user_id)
        .bind(&row.title)
        .bind(&row.created_at)
        .bind(&row.created_at)
        .bind(&row.started_at)
        .bind(&row.ended_at)
        .bind(&row.event_id)
        .bind(&row.external_event_id)
        .bind(&row.external_provider)
        .bind(&row.series_id)
        .bind(&row.event_json)
        .bind(&row.metadata_json)
        .bind(&row.folder_path)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::Document(row) => sqlx::query(
            "INSERT INTO session_documents \
             (id, workspace_id, session_id, kind, template_id, title, body_format, body, source_hash, \
              sort_order, created_by, updated_by, created_at, updated_at, generation_metadata_json) \
             SELECT ?, session.workspace_id, session.id, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ? \
             FROM sessions AS session \
             WHERE session.id = ? AND session.deleted_at IS NULL \
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.kind)
        .bind(&row.template_id)
        .bind(&row.title)
        .bind(&row.body_format)
        .bind(&row.body)
        .bind(&row.source_hash)
        .bind(row.sort_order)
        .bind(&row.created_by)
        .bind(&row.created_by)
        .bind(&row.created_at)
        .bind(&row.updated_at)
        .bind(&row.generation_metadata_json)
        .bind(&row.session_id)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::Transcript(row) => sqlx::query(
            "INSERT INTO transcripts \
             (id, workspace_id, owner_user_id, session_id, started_at_ms, ended_at_ms, memo, \
              words_json, speaker_hints_json, created_at, updated_at, metadata_json) \
             SELECT ?, session.workspace_id, ?, session.id, ?, ?, ?, ?, ?, ?, ?, ? \
             FROM sessions AS session \
             WHERE session.id = ? AND session.deleted_at IS NULL \
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.owner_user_id)
        .bind(row.started_at_ms)
        .bind(row.ended_at_ms)
        .bind(&row.memo)
        .bind(&row.words_json)
        .bind(&row.speaker_hints_json)
        .bind(&row.created_at)
        .bind(&row.created_at)
        .bind(&row.metadata_json)
        .bind(&row.session_id)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::Participant(row) => sqlx::query(
            "INSERT INTO session_participants \
             (id, workspace_id, owner_user_id, session_id, human_id, source) \
             SELECT ?, session.workspace_id, ?, session.id, ?, ? \
             FROM sessions AS session \
             WHERE session.id = ? AND session.deleted_at IS NULL \
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.owner_user_id)
        .bind(&row.human_id)
        .bind(&row.source)
        .bind(&row.session_id)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::ActionItem(row) => sqlx::query(
            "INSERT INTO action_items \
             (id, workspace_id, created_by, session_id, source_type, source_id, source_order, \
              status, text, body_json, due_at) \
             SELECT ?, session.workspace_id, ?, session.id, ?, ?, ?, ?, ?, ?, ? \
             FROM sessions AS session \
             WHERE session.id = ? AND session.deleted_at IS NULL \
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.owner_user_id)
        .bind(&row.source_type)
        .bind(&row.source_id)
        .bind(row.source_order)
        .bind(&row.status)
        .bind(&row.text)
        .bind(&row.body_json)
        .bind(&row.due_at)
        .bind(&row.session_id)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::Attachment(row) => sqlx::query(
            "INSERT INTO session_attachments \
             (id, workspace_id, session_id, filename, relative_path, content_type, size_bytes, \
              sha256, source_type, source_id) \
             SELECT ?, session.workspace_id, session.id, ?, ?, ?, ?, ?, 'legacy_file', ? \
             FROM sessions AS session \
             WHERE session.id = ? AND session.deleted_at IS NULL \
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.filename)
        .bind(&row.relative_path)
        .bind(&row.content_type)
        .bind(row.size_bytes)
        .bind(&row.sha256)
        .bind(&row.source_id)
        .bind(&row.session_id)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::Tag(row) => sqlx::query(
            "INSERT INTO tags (id, owner_user_id, name) \
             VALUES (?, ?, ?) ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.owner_user_id)
        .bind(&row.name)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::SessionTag(row) => sqlx::query(
            "INSERT INTO session_tags (id, owner_user_id, session_id, tag_id) \
             SELECT ?, ?, session.id, ? FROM sessions AS session \
             WHERE session.id = ? AND session.deleted_at IS NULL \
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.owner_user_id)
        .bind(&row.tag_id)
        .bind(&row.session_id)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::ChatGroup(row) => sqlx::query(
            "INSERT INTO chat_groups (id, owner_user_id, title, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.owner_user_id)
        .bind(&row.title)
        .bind(&row.created_at)
        .bind(&row.created_at)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::ChatMessage(row) => sqlx::query(
            "INSERT INTO chat_messages \
             (id, chat_group_id, owner_user_id, role, content, metadata_json, parts_json, status, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.chat_group_id)
        .bind(&row.owner_user_id)
        .bind(&row.role)
        .bind(&row.content)
        .bind(&row.metadata_json)
        .bind(&row.parts_json)
        .bind(&row.status)
        .bind(&row.created_at)
        .bind(&row.created_at)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::DailyNote(row) => sqlx::query(
            "INSERT INTO daily_notes (id, owner_user_id, note_date, body_format, body) \
             VALUES (?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.owner_user_id)
        .bind(&row.note_date)
        .bind(&row.body_format)
        .bind(&row.body)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::AppSetting(row) => sqlx::query(
            "INSERT INTO app_settings (id, value_json) \
             VALUES (?, ?) ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.value_json)
        .execute(&mut **transaction)
        .await?,
    };

    if result.rows_affected() > 0 {
        Ok(InsertOutcome::Inserted)
    } else if row_matches_existing(transaction, row).await? {
        Ok(InsertOutcome::Matched)
    } else if let Some(outcome) = reconcile_content_conflict(transaction, row).await? {
        Ok(outcome)
    } else if import_target_exists(transaction, row).await? {
        Ok(if row.existing_sqlite_is_authoritative() {
            InsertOutcome::RetainedExisting
        } else {
            InsertOutcome::Conflict
        })
    } else {
        Ok(InsertOutcome::MissingDependency)
    }
}

async fn import_target_exists(
    transaction: &mut Transaction<'_, Sqlite>,
    row: &LegacyImportRow,
) -> Result<bool, sqlx::Error> {
    if matches!(row, LegacyImportRow::Session(_)) {
        return sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE id = ? AND deleted_at IS NULL)",
        )
        .bind(row.id())
        .fetch_one(&mut **transaction)
        .await;
    }
    let query = format!(
        "SELECT EXISTS(SELECT 1 FROM {} WHERE id = ?)",
        row.table_name()
    );
    sqlx::query_scalar(sqlx::AssertSqlSafe(query.as_str()))
        .bind(row.id())
        .fetch_one(&mut **transaction)
        .await
}

#[cfg(test)]
mod tests;
