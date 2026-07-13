use sqlx::{Row, Sqlite, SqlitePool, Transaction};

pub const LEGACY_IMPORTER_VERSION: i64 = 1;

#[derive(Debug, Default)]
pub struct LegacyImportBatch {
    pub rows: Vec<LegacyImportRow>,
    pub skipped_count: usize,
    pub warning: String,
}

#[derive(Debug)]
pub enum LegacyImportRow {
    Calendar(LegacyCalendar),
    Event(LegacyEvent),
    Template(LegacyTemplate),
    Organization(LegacyOrganization),
    Human(LegacyHuman),
    Session(LegacySession),
    Document(LegacyDocument),
    Transcript(LegacyTranscript),
    Participant(LegacyParticipant),
    ActionItem(LegacyActionItem),
    Attachment(LegacyAttachment),
    Tag(LegacyTag),
    SessionTag(LegacySessionTag),
    ChatGroup(LegacyChatGroup),
    ChatMessage(LegacyChatMessage),
    DailyNote(LegacyDailyNote),
    AppSetting(LegacyAppSetting),
}

impl LegacyImportRow {
    fn table_name(&self) -> &'static str {
        match self {
            Self::Calendar(_) => "calendars",
            Self::Event(_) => "events",
            Self::Template(_) => "templates",
            Self::Organization(_) => "organizations",
            Self::Human(_) => "humans",
            Self::Session(_) => "sessions",
            Self::Document(_) => "session_documents",
            Self::Transcript(_) => "transcripts",
            Self::Participant(_) => "session_participants",
            Self::ActionItem(_) => "action_items",
            Self::Attachment(_) => "session_attachments",
            Self::Tag(_) => "tags",
            Self::SessionTag(_) => "session_tags",
            Self::ChatGroup(_) => "chat_groups",
            Self::ChatMessage(_) => "chat_messages",
            Self::DailyNote(_) => "daily_notes",
            Self::AppSetting(_) => "app_settings",
        }
    }

    fn id(&self) -> &str {
        match self {
            Self::Calendar(row) => &row.id,
            Self::Event(row) => &row.id,
            Self::Template(row) => &row.id,
            Self::Organization(row) => &row.id,
            Self::Human(row) => &row.id,
            Self::Session(row) => &row.id,
            Self::Document(row) => &row.id,
            Self::Transcript(row) => &row.id,
            Self::Participant(row) => &row.id,
            Self::ActionItem(row) => &row.id,
            Self::Attachment(row) => &row.id,
            Self::Tag(row) => &row.id,
            Self::SessionTag(row) => &row.id,
            Self::ChatGroup(row) => &row.id,
            Self::ChatMessage(row) => &row.id,
            Self::DailyNote(row) => &row.id,
            Self::AppSetting(row) => &row.id,
        }
    }

    fn existing_sqlite_is_authoritative(&self) -> bool {
        matches!(self, Self::Calendar(_) | Self::Event(_) | Self::Template(_))
    }
}

#[derive(Debug)]
pub struct LegacyCalendar {
    pub id: String,
    pub tracking_id_calendar: String,
    pub name: String,
    pub enabled: bool,
    pub provider: String,
    pub source: String,
    pub color: String,
    pub connection_id: String,
}

#[derive(Debug)]
pub struct LegacyEvent {
    pub id: String,
    pub tracking_id_event: String,
    pub calendar_id: String,
    pub title: String,
    pub started_at: String,
    pub ended_at: String,
    pub location: String,
    pub meeting_link: String,
    pub description: String,
    pub note: String,
    pub recurrence_series_id: String,
    pub has_recurrence_rules: bool,
    pub is_all_day: bool,
    pub provider: String,
    pub participants_json: Option<String>,
}

#[derive(Debug)]
pub struct LegacyTemplate {
    pub id: String,
    pub title: String,
    pub description: String,
    pub pinned: bool,
    pub pin_order: Option<i64>,
    pub category: Option<String>,
    pub targets_json: Option<String>,
    pub sections_json: String,
}

#[derive(Debug)]
pub struct LegacyOrganization {
    pub id: String,
    pub owner_user_id: String,
    pub name: String,
    pub memo: String,
    pub pinned: bool,
    pub pin_order: Option<i64>,
    pub created_at: String,
}

#[derive(Debug)]
pub struct LegacyHuman {
    pub id: String,
    pub owner_user_id: String,
    pub organization_id: String,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub job_title: String,
    pub linkedin_username: String,
    pub memo: String,
    pub pinned: bool,
    pub pin_order: Option<i64>,
    pub created_at: String,
}

#[derive(Debug)]
pub struct LegacySession {
    pub id: String,
    pub owner_user_id: String,
    pub title: String,
    pub created_at: String,
    pub started_at: String,
    pub ended_at: String,
    pub event_id: String,
    pub external_event_id: String,
    pub external_provider: String,
    pub series_id: String,
    pub event_json: String,
    pub folder_path: String,
}

#[derive(Debug)]
pub struct LegacyDocument {
    pub id: String,
    pub session_id: String,
    pub kind: String,
    pub template_id: String,
    pub title: String,
    pub body_format: String,
    pub body: String,
    pub source_hash: String,
    pub sort_order: i64,
    pub created_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug)]
pub struct LegacyTranscript {
    pub id: String,
    pub owner_user_id: String,
    pub session_id: String,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub memo: String,
    pub words_json: String,
    pub speaker_hints_json: String,
    pub created_at: String,
}

#[derive(Debug)]
pub struct LegacyParticipant {
    pub id: String,
    pub owner_user_id: String,
    pub session_id: String,
    pub human_id: String,
    pub source: String,
}

#[derive(Debug)]
pub struct LegacyActionItem {
    pub id: String,
    pub owner_user_id: String,
    pub session_id: String,
    pub source_type: String,
    pub source_id: String,
    pub source_order: i64,
    pub status: String,
    pub text: String,
    pub body_json: String,
    pub due_at: String,
}

#[derive(Debug)]
pub struct LegacyAttachment {
    pub id: String,
    pub session_id: String,
    pub filename: String,
    pub relative_path: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub source_id: String,
}

#[derive(Debug)]
pub struct LegacyTag {
    pub id: String,
    pub owner_user_id: String,
    pub name: String,
}

#[derive(Debug)]
pub struct LegacySessionTag {
    pub id: String,
    pub owner_user_id: String,
    pub session_id: String,
    pub tag_id: String,
}

#[derive(Debug)]
pub struct LegacyChatGroup {
    pub id: String,
    pub owner_user_id: String,
    pub title: String,
    pub created_at: String,
}

#[derive(Debug)]
pub struct LegacyChatMessage {
    pub id: String,
    pub chat_group_id: String,
    pub owner_user_id: String,
    pub role: String,
    pub content: String,
    pub metadata_json: String,
    pub parts_json: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug)]
pub struct LegacyDailyNote {
    pub id: String,
    pub owner_user_id: String,
    pub note_date: String,
    pub body_format: String,
    pub body: String,
}

#[derive(Debug)]
pub struct LegacyAppSetting {
    pub id: String,
    pub value_json: String,
}

pub struct LegacyImportItem<'a> {
    pub id: &'a str,
    pub run_id: &'a str,
    pub source_path: &'a str,
    pub source_kind: &'a str,
    pub source_sha256: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyImportItemResult {
    pub discovered_count: i64,
    pub imported_count: i64,
    pub matched_count: i64,
    pub skipped_count: i64,
    pub conflict_count: i64,
}

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

    for row in &batch.rows {
        let outcome = if dry_run {
            InsertOutcome::DryRun
        } else {
            insert_row_if_missing(&mut transaction, row).await?
        };
        match outcome {
            InsertOutcome::Inserted => imported_count += 1,
            InsertOutcome::Matched | InsertOutcome::RetainedExisting => matched_count += 1,
            InsertOutcome::Conflict => conflict_count += 1,
            InsertOutcome::DryRun => {}
        }
        record_import_target(&mut transaction, &item, row, outcome).await?;
    }

    let discovered_count = i64::try_from(batch.rows.len()).unwrap_or(i64::MAX)
        + i64::try_from(batch.skipped_count).unwrap_or(i64::MAX);
    let skipped_count = i64::try_from(batch.skipped_count).unwrap_or(i64::MAX);
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
    .fetch_one(pool)
    .await?;

    let skipped_count = aggregate.get::<i64, _>("skipped_count");
    let conflict_count = aggregate.get::<i64, _>("conflict_count");
    let error_count = aggregate.get::<i64, _>("error_count");
    let status = if skipped_count == 0 && conflict_count == 0 && error_count == 0 {
        "completed"
    } else {
        "completed_with_issues"
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
    .execute(pool)
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
    .execute(pool)
    .await?;

    Ok(status.to_string())
}

pub async fn fail_legacy_import_run(
    pool: &SqlitePool,
    run_id: &str,
    error: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE migration_import_runs
         SET status = 'failed', error = ?, completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(error)
    .bind(run_id)
    .execute(pool)
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
    .execute(pool)
    .await?;

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InsertOutcome {
    Inserted,
    Matched,
    RetainedExisting,
    Conflict,
    DryRun,
}

impl InsertOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Inserted => "inserted",
            Self::Matched => "matched",
            Self::RetainedExisting => "retained_existing",
            Self::Conflict => "conflict",
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
    .bind(outcome.as_str())
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
              ended_at, event_id, external_event_id, external_provider, series_id, event_json, folder_path) \
             VALUES (?, NULLIF((SELECT json_extract(value_json, '$.workspace_id') \
               FROM app_settings WHERE id = 'cloudsync_workspace_binding'), ''), \
               ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
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
        .bind(&row.folder_path)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::Document(row) => sqlx::query(
            "INSERT INTO session_documents \
             (id, workspace_id, session_id, kind, template_id, title, body_format, body, source_hash, \
              sort_order, created_by, updated_by, created_at, updated_at) \
             SELECT ?, session.workspace_id, session.id, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ? \
             FROM sessions AS session \
             WHERE session.id = ? \
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
        .bind(&row.session_id)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::Transcript(row) => sqlx::query(
            "INSERT INTO transcripts \
             (id, workspace_id, owner_user_id, session_id, started_at_ms, ended_at_ms, memo, \
              words_json, speaker_hints_json, created_at, updated_at) \
             SELECT ?, session.workspace_id, ?, session.id, ?, ?, ?, ?, ?, ?, ? \
             FROM sessions AS session \
             WHERE session.id = ? \
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
        .bind(&row.session_id)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::Participant(row) => sqlx::query(
            "INSERT INTO session_participants \
             (id, workspace_id, owner_user_id, session_id, human_id, source) \
             SELECT ?, session.workspace_id, ?, session.id, ?, ? \
             FROM sessions AS session \
             WHERE session.id = ? \
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
             WHERE session.id = ? \
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
             WHERE session.id = ? \
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
             VALUES (?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.owner_user_id)
        .bind(&row.session_id)
        .bind(&row.tag_id)
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
    } else if row.existing_sqlite_is_authoritative() {
        Ok(InsertOutcome::RetainedExisting)
    } else {
        Ok(InsertOutcome::Conflict)
    }
}

async fn row_matches_existing(
    transaction: &mut Transaction<'_, Sqlite>,
    row: &LegacyImportRow,
) -> Result<bool, sqlx::Error> {
    match row {
        LegacyImportRow::Calendar(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM calendars
               WHERE id = ?
                 AND tracking_id_calendar IS ?
                 AND name IS ?
                 AND enabled IS ?
                 AND provider IS ?
                 AND source IS ?
                 AND color IS ?
                 AND connection_id IS ?
             )",
            )
            .bind(&row.id)
            .bind(&row.tracking_id_calendar)
            .bind(&row.name)
            .bind(row.enabled)
            .bind(&row.provider)
            .bind(&row.source)
            .bind(&row.color)
            .bind(&row.connection_id)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Event(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM events
               WHERE id = ?
                 AND tracking_id_event IS ?
                 AND calendar_id IS ?
                 AND title IS ?
                 AND started_at IS ?
                 AND ended_at IS ?
                 AND location IS ?
                 AND meeting_link IS ?
                 AND description IS ?
                 AND note IS ?
                 AND recurrence_series_id IS ?
                 AND has_recurrence_rules IS ?
                 AND is_all_day IS ?
                 AND provider IS ?
                 AND participants_json IS ?
             )",
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
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Template(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM templates
               WHERE id = ?
                 AND title IS ?
                 AND description IS ?
                 AND pinned IS ?
                 AND pin_order IS ?
                 AND category IS ?
                 AND targets_json IS ?
                 AND sections_json IS ?
             )",
            )
            .bind(&row.id)
            .bind(&row.title)
            .bind(&row.description)
            .bind(row.pinned)
            .bind(row.pin_order)
            .bind(&row.category)
            .bind(&row.targets_json)
            .bind(&row.sections_json)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Organization(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM organizations
               WHERE id = ?
                 AND owner_user_id IS ?
                 AND name IS ?
                 AND memo IS ?
                 AND pinned IS ?
                 AND pin_order IS ?
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.name)
            .bind(&row.memo)
            .bind(row.pinned)
            .bind(row.pin_order)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Human(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM humans
               WHERE id = ?
                 AND owner_user_id IS ?
                 AND organization_id IS ?
                 AND name IS ?
                 AND email IS ?
                 AND phone IS ?
                 AND job_title IS ?
                 AND linkedin_username IS ?
                 AND memo IS ?
                 AND pinned IS ?
                 AND pin_order IS ?
             )",
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
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Session(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM sessions
               WHERE id = ?
                 AND owner_user_id IS ?
                 AND title IS ?
                 AND created_at IS ?
                 AND started_at IS ?
                 AND ended_at IS ?
                 AND event_id IS ?
                 AND external_event_id IS ?
                 AND external_provider IS ?
                 AND series_id IS ?
                 AND event_json IS ?
                 AND folder_path IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.title)
            .bind(&row.created_at)
            .bind(&row.started_at)
            .bind(&row.ended_at)
            .bind(&row.event_id)
            .bind(&row.external_event_id)
            .bind(&row.external_provider)
            .bind(&row.series_id)
            .bind(&row.event_json)
            .bind(&row.folder_path)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Document(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM session_documents
               WHERE id = ?
                 AND session_id IS ?
                 AND kind IS ?
                 AND template_id IS ?
                 AND title IS ?
                 AND body_format IS ?
                 AND body IS ?
                 AND source_hash IS ?
                 AND sort_order IS ?
                 AND created_by IS ?
                 AND created_at IS ?
                 AND updated_at IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.session_id)
            .bind(&row.kind)
            .bind(&row.template_id)
            .bind(&row.title)
            .bind(&row.body_format)
            .bind(&row.body)
            .bind(&row.source_hash)
            .bind(row.sort_order)
            .bind(&row.created_by)
            .bind(&row.created_at)
            .bind(&row.updated_at)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Transcript(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM transcripts
               WHERE id = ?
                 AND owner_user_id IS ?
                 AND session_id IS ?
                 AND started_at_ms IS ?
                 AND ended_at_ms IS ?
                 AND memo IS ?
                 AND words_json IS ?
                 AND speaker_hints_json IS ?
                 AND created_at IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.session_id)
            .bind(row.started_at_ms)
            .bind(row.ended_at_ms)
            .bind(&row.memo)
            .bind(&row.words_json)
            .bind(&row.speaker_hints_json)
            .bind(&row.created_at)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Participant(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM session_participants
               WHERE id = ?
                 AND owner_user_id IS ?
                 AND session_id IS ?
                 AND human_id IS ?
                 AND source IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.session_id)
            .bind(&row.human_id)
            .bind(&row.source)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::ActionItem(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM action_items
               WHERE id = ?
                 AND created_by IS ?
                 AND session_id IS ?
                 AND source_type IS ?
                 AND source_id IS ?
                 AND source_order IS ?
                 AND status IS ?
                 AND text IS ?
                 AND body_json IS ?
                 AND due_at IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.session_id)
            .bind(&row.source_type)
            .bind(&row.source_id)
            .bind(row.source_order)
            .bind(&row.status)
            .bind(&row.text)
            .bind(&row.body_json)
            .bind(&row.due_at)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Attachment(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM session_attachments
               WHERE id = ?
                 AND session_id IS ?
                 AND filename IS ?
                 AND relative_path IS ?
                 AND content_type IS ?
                 AND size_bytes IS ?
                 AND sha256 IS ?
                 AND source_id IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.session_id)
            .bind(&row.filename)
            .bind(&row.relative_path)
            .bind(&row.content_type)
            .bind(row.size_bytes)
            .bind(&row.sha256)
            .bind(&row.source_id)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::Tag(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM tags
               WHERE id = ? AND owner_user_id IS ? AND name IS ?
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.name)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::SessionTag(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM session_tags
               WHERE id = ?
                 AND owner_user_id IS ?
                 AND session_id IS ?
                 AND tag_id IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.session_id)
            .bind(&row.tag_id)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::ChatGroup(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM chat_groups
               WHERE id = ?
                 AND owner_user_id IS ?
                 AND title IS ?
                 AND created_at IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.title)
            .bind(&row.created_at)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::ChatMessage(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM chat_messages
               WHERE id = ?
                 AND chat_group_id IS ?
                 AND owner_user_id IS ?
                 AND role IS ?
                 AND content IS ?
                 AND metadata_json IS ?
                 AND parts_json IS ?
                 AND status IS ?
                 AND created_at IS ?
                 AND deleted_at IS NULL
             )",
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
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::DailyNote(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM daily_notes
               WHERE id = ?
                 AND owner_user_id IS ?
                 AND note_date IS ?
                 AND body_format IS ?
                 AND body IS ?
                 AND deleted_at IS NULL
             )",
            )
            .bind(&row.id)
            .bind(&row.owner_user_id)
            .bind(&row.note_date)
            .bind(&row.body_format)
            .bind(&row.body)
            .fetch_one(&mut **transaction)
            .await
        }
        LegacyImportRow::AppSetting(row) => {
            sqlx::query_scalar(
                "SELECT EXISTS(
               SELECT 1 FROM app_settings
               WHERE id = ? AND value_json IS ?
             )",
            )
            .bind(&row.id)
            .bind(&row.value_json)
            .fetch_one(&mut **transaction)
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_db_core::Db;

    async fn test_db() -> Db {
        let db = Db::connect_memory_plain().await.unwrap();
        crate::prepare_schema(&db).await.unwrap();
        db
    }

    fn session_batch() -> LegacyImportBatch {
        LegacyImportBatch {
            rows: vec![LegacyImportRow::Session(LegacySession {
                id: "session-1".to_string(),
                owner_user_id: "user-1".to_string(),
                title: "Planning".to_string(),
                created_at: "2026-07-10T12:00:00Z".to_string(),
                started_at: String::new(),
                ended_at: String::new(),
                event_id: String::new(),
                external_event_id: String::new(),
                external_provider: String::new(),
                series_id: String::new(),
                event_json: String::new(),
                folder_path: "work".to_string(),
            })],
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn import_fails_closed_without_a_workspace_binding() {
        let db = test_db().await;
        sqlx::query("DELETE FROM app_settings WHERE id = 'cloudsync_workspace_binding'")
            .execute(db.pool())
            .await
            .unwrap();
        begin_legacy_import_run(db.pool(), "run-1", "/vault", false)
            .await
            .unwrap();

        let error = apply_legacy_import_item(
            db.pool(),
            LegacyImportItem {
                id: "item-1",
                run_id: "run-1",
                source_path: "sessions/session-1/_meta.json",
                source_kind: "session_meta",
                source_sha256: "hash-1",
            },
            &session_batch(),
            false,
        )
        .await
        .unwrap_err();

        assert!(matches!(error, sqlx::Error::Database(_)));
        let session_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
            .fetch_one(db.pool())
            .await
            .unwrap();
        let item_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM migration_import_items")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(session_count, 0);
        assert_eq!(item_count, 0);
    }

    #[tokio::test]
    async fn import_item_is_atomic_and_existing_sqlite_rows_win() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO app_settings (id, value_json) \
             VALUES ('cloudsync_workspace_binding', \
               '{\"workspace_id\":\"workspace-1\",\"account_user_id\":\"user-1\"}') \
             ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json",
        )
        .execute(db.pool())
        .await
        .unwrap();
        begin_legacy_import_run(db.pool(), "run-1", "/vault", false)
            .await
            .unwrap();

        let result = apply_legacy_import_item(
            db.pool(),
            LegacyImportItem {
                id: "item-1",
                run_id: "run-1",
                source_path: "sessions/session-1/_meta.json",
                source_kind: "session_meta",
                source_sha256: "hash-1",
            },
            &session_batch(),
            false,
        )
        .await
        .unwrap();

        assert_eq!(result.imported_count, 1);
        assert_eq!(result.conflict_count, 0);
        let workspace_id: String =
            sqlx::query_scalar("SELECT workspace_id FROM sessions WHERE id = 'session-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(workspace_id, "workspace-1");

        begin_legacy_import_run(db.pool(), "run-2", "/vault", false)
            .await
            .unwrap();
        let mut conflicting_batch = session_batch();
        let LegacyImportRow::Session(session) = &mut conflicting_batch.rows[0] else {
            panic!("expected session");
        };
        session.title = "Conflicting legacy title".to_string();
        let result = apply_legacy_import_item(
            db.pool(),
            LegacyImportItem {
                id: "item-2",
                run_id: "run-2",
                source_path: "sessions/session-1/_meta.json",
                source_kind: "session_meta",
                source_sha256: "hash-2",
            },
            &conflicting_batch,
            false,
        )
        .await
        .unwrap();

        assert_eq!(result.imported_count, 0);
        assert_eq!(result.conflict_count, 1);
        let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = ?")
            .bind("session-1")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(title, "Planning");
    }

    #[tokio::test]
    async fn imported_session_children_inherit_parent_workspace() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO app_settings (id, value_json) \
             VALUES ('cloudsync_workspace_binding', \
               '{\"workspace_id\":\"workspace-1\",\"account_user_id\":\"user-1\"}') \
             ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json",
        )
        .execute(db.pool())
        .await
        .unwrap();
        begin_legacy_import_run(db.pool(), "run-1", "/vault", false)
            .await
            .unwrap();

        let mut batch = session_batch();
        batch.rows.extend([
            LegacyImportRow::Document(LegacyDocument {
                id: "document-1".to_string(),
                session_id: "session-1".to_string(),
                kind: "note".to_string(),
                template_id: String::new(),
                title: String::new(),
                body_format: "markdown".to_string(),
                body: "Notes".to_string(),
                source_hash: String::new(),
                sort_order: 0,
                created_by: "user-1".to_string(),
                created_at: "2026-07-10T12:00:00Z".to_string(),
                updated_at: "2026-07-10T12:00:00Z".to_string(),
            }),
            LegacyImportRow::Transcript(LegacyTranscript {
                id: "transcript-1".to_string(),
                owner_user_id: "user-1".to_string(),
                session_id: "session-1".to_string(),
                started_at_ms: 0,
                ended_at_ms: Some(1000),
                memo: String::new(),
                words_json: "[]".to_string(),
                speaker_hints_json: "[]".to_string(),
                created_at: "2026-07-10T12:00:00Z".to_string(),
            }),
            LegacyImportRow::Participant(LegacyParticipant {
                id: "participant-1".to_string(),
                owner_user_id: "user-1".to_string(),
                session_id: "session-1".to_string(),
                human_id: "human-1".to_string(),
                source: "manual".to_string(),
            }),
            LegacyImportRow::ActionItem(LegacyActionItem {
                id: "action-1".to_string(),
                owner_user_id: "user-1".to_string(),
                session_id: "session-1".to_string(),
                source_type: "session".to_string(),
                source_id: "session-1".to_string(),
                source_order: 0,
                status: "todo".to_string(),
                text: "Follow up".to_string(),
                body_json: "{}".to_string(),
                due_at: String::new(),
            }),
            LegacyImportRow::Attachment(LegacyAttachment {
                id: "attachment-1".to_string(),
                session_id: "session-1".to_string(),
                filename: "notes.txt".to_string(),
                relative_path: "notes.txt".to_string(),
                content_type: "text/plain".to_string(),
                size_bytes: 5,
                sha256: "hash".to_string(),
                source_id: "legacy-1".to_string(),
            }),
        ]);

        let result = apply_legacy_import_item(
            db.pool(),
            LegacyImportItem {
                id: "item-1",
                run_id: "run-1",
                source_path: "sessions/session-1",
                source_kind: "session",
                source_sha256: "hash-1",
            },
            &batch,
            false,
        )
        .await
        .unwrap();

        assert_eq!(result.imported_count, 6);
        for table in [
            "sessions",
            "session_documents",
            "transcripts",
            "session_participants",
            "action_items",
            "session_attachments",
        ] {
            let sql = format!("SELECT workspace_id FROM {table} LIMIT 1");
            let workspace_id: String = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()))
                .fetch_one(db.pool())
                .await
                .unwrap();
            assert_eq!(workspace_id, "workspace-1", "table {table}");
        }
    }

    #[tokio::test]
    async fn preexisting_sqlite_domains_retain_newer_rows_without_blocking_parity() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO calendars \
             (id, tracking_id_calendar, name, enabled, provider, source, color, connection_id) \
             VALUES ('calendar-1', 'tracking-1', 'Work', 0, 'google', 'work@example.com', '#123456', 'connection-1')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO events \
             (id, tracking_id_event, calendar_id, title, started_at, ended_at, location, \
              meeting_link, description, note, recurrence_series_id, has_recurrence_rules, \
              is_all_day, provider, participants_json) \
             VALUES ('event-1', 'tracking-event-1', 'calendar-1', 'Updated title', \
                     '2026-07-11T10:00:00Z', '2026-07-11T11:00:00Z', '', '', '', '', '', 0, 0, \
                     'google', '[]')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        begin_legacy_import_run(db.pool(), "run-1", "/vault", false)
            .await
            .unwrap();
        let batch = LegacyImportBatch {
            rows: vec![
                LegacyImportRow::Calendar(LegacyCalendar {
                    id: "calendar-1".to_string(),
                    tracking_id_calendar: "tracking-1".to_string(),
                    name: "Work".to_string(),
                    enabled: true,
                    provider: "google".to_string(),
                    source: "work@example.com".to_string(),
                    color: "#123456".to_string(),
                    connection_id: "connection-1".to_string(),
                }),
                LegacyImportRow::Event(LegacyEvent {
                    id: "event-1".to_string(),
                    tracking_id_event: "tracking-event-1".to_string(),
                    calendar_id: "calendar-1".to_string(),
                    title: "Stale title".to_string(),
                    started_at: "2026-07-11T09:00:00Z".to_string(),
                    ended_at: "2026-07-11T10:00:00Z".to_string(),
                    location: String::new(),
                    meeting_link: String::new(),
                    description: String::new(),
                    note: String::new(),
                    recurrence_series_id: String::new(),
                    has_recurrence_rules: false,
                    is_all_day: false,
                    provider: "google".to_string(),
                    participants_json: Some("[]".to_string()),
                }),
            ],
            ..Default::default()
        };

        let result = apply_legacy_import_item(
            db.pool(),
            LegacyImportItem {
                id: "item-1",
                run_id: "run-1",
                source_path: "calendar-data.json",
                source_kind: "calendar_data",
                source_sha256: "hash-1",
            },
            &batch,
            false,
        )
        .await
        .unwrap();

        assert_eq!(result.imported_count, 0);
        assert_eq!(result.matched_count, 2);
        assert_eq!(result.conflict_count, 0);
        assert_eq!(
            finish_legacy_import_run(db.pool(), "run-1").await.unwrap(),
            "completed"
        );

        let target_statuses: Vec<String> = sqlx::query_scalar(
            "SELECT status FROM migration_import_targets WHERE run_id = 'run-1' ORDER BY target_id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(
            target_statuses,
            vec!["retained_existing", "retained_existing"]
        );

        let calendar_enabled: bool =
            sqlx::query_scalar("SELECT enabled FROM calendars WHERE id = 'calendar-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let event_title: String =
            sqlx::query_scalar("SELECT title FROM events WHERE id = 'event-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(!calendar_enabled);
        assert_eq!(event_title, "Updated title");
    }

    #[tokio::test]
    async fn dry_run_records_counts_without_writing_domain_rows() {
        let db = test_db().await;
        begin_legacy_import_run(db.pool(), "run-1", "/vault", true)
            .await
            .unwrap();

        let result = apply_legacy_import_item(
            db.pool(),
            LegacyImportItem {
                id: "item-1",
                run_id: "run-1",
                source_path: "sessions/session-1/_meta.json",
                source_kind: "session_meta",
                source_sha256: "hash-1",
            },
            &session_batch(),
            true,
        )
        .await
        .unwrap();

        assert_eq!(result.discovered_count, 1);
        assert_eq!(result.imported_count, 0);
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn completed_source_hash_is_restartable() {
        let db = test_db().await;
        begin_legacy_import_run(db.pool(), "run-1", "/vault", false)
            .await
            .unwrap();
        apply_legacy_import_item(
            db.pool(),
            LegacyImportItem {
                id: "item-1",
                run_id: "run-1",
                source_path: "sessions/session-1/_meta.json",
                source_kind: "session_meta",
                source_sha256: "hash-1",
            },
            &session_batch(),
            false,
        )
        .await
        .unwrap();
        finish_legacy_import_run(db.pool(), "run-1").await.unwrap();

        assert!(
            legacy_source_already_imported(db.pool(), "sessions/session-1/_meta.json", "hash-1")
                .await
                .unwrap()
        );
        assert!(
            !legacy_source_already_imported(db.pool(), "sessions/session-1/_meta.json", "changed")
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn successful_run_marks_import_parity_verified() {
        let db = test_db().await;
        begin_legacy_import_run(db.pool(), "run-1", "/vault", false)
            .await
            .unwrap();
        apply_legacy_import_item(
            db.pool(),
            LegacyImportItem {
                id: "item-1",
                run_id: "run-1",
                source_path: "sessions/session-1/_meta.json",
                source_kind: "session_meta",
                source_sha256: "hash-1",
            },
            &session_batch(),
            false,
        )
        .await
        .unwrap();

        assert_eq!(
            finish_legacy_import_run(db.pool(), "run-1").await.unwrap(),
            "completed"
        );

        let parity_verified: bool = sqlx::query_scalar(
            "SELECT parity_verified FROM storage_migration_state WHERE id = 'legacy_v1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert!(parity_verified);
    }

    #[tokio::test]
    async fn identical_shared_rows_are_matched_instead_of_reported_as_conflicts() {
        let db = test_db().await;
        let batch = LegacyImportBatch {
            rows: vec![LegacyImportRow::Tag(LegacyTag {
                id: "work".to_string(),
                owner_user_id: "user-1".to_string(),
                name: "work".to_string(),
            })],
            ..Default::default()
        };

        for run in 1..=2 {
            let run_id = format!("run-{run}");
            let item_id = format!("item-{run}");
            let source_path = format!("sessions/session-{run}/_meta.json");
            let source_hash = format!("hash-{run}");
            begin_legacy_import_run(db.pool(), &run_id, "/vault", false)
                .await
                .unwrap();
            let result = apply_legacy_import_item(
                db.pool(),
                LegacyImportItem {
                    id: &item_id,
                    run_id: &run_id,
                    source_path: &source_path,
                    source_kind: "session_meta",
                    source_sha256: &source_hash,
                },
                &batch,
                false,
            )
            .await
            .unwrap();

            if run == 1 {
                assert_eq!(result.imported_count, 1);
                assert_eq!(result.matched_count, 0);
            } else {
                assert_eq!(result.imported_count, 0);
                assert_eq!(result.matched_count, 1);
                assert_eq!(result.conflict_count, 0);
            }
        }
    }

    #[tokio::test]
    async fn first_import_restores_legacy_edits_over_untouched_default_templates() {
        let db = test_db().await;
        begin_legacy_import_run(db.pool(), "run-1", "/vault", false)
            .await
            .unwrap();
        let batch = LegacyImportBatch {
            rows: vec![LegacyImportRow::Template(LegacyTemplate {
                id: "default-daily-standup".to_string(),
                title: "My Standup".to_string(),
                description: "User-edited legacy template".to_string(),
                pinned: true,
                pin_order: Some(1),
                category: Some("Custom".to_string()),
                targets_json: Some("[\"Engineering\"]".to_string()),
                sections_json: "[]".to_string(),
            })],
            ..Default::default()
        };

        apply_legacy_import_item(
            db.pool(),
            LegacyImportItem {
                id: "item-1",
                run_id: "run-1",
                source_path: "templates.json",
                source_kind: "template",
                source_sha256: "hash-1",
            },
            &batch,
            false,
        )
        .await
        .unwrap();

        let title: String =
            sqlx::query_scalar("SELECT title FROM templates WHERE id = 'default-daily-standup'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(title, "My Standup");
        finish_legacy_import_run(db.pool(), "run-1").await.unwrap();

        sqlx::query(
            "UPDATE templates SET updated_at = '2099-01-01T00:00:00Z' \
             WHERE id = 'default-daily-standup'",
        )
        .execute(db.pool())
        .await
        .unwrap();
        begin_legacy_import_run(db.pool(), "run-2", "/vault", false)
            .await
            .unwrap();
        let changed_batch = LegacyImportBatch {
            rows: vec![LegacyImportRow::Template(LegacyTemplate {
                id: "default-daily-standup".to_string(),
                title: "Stale Legacy Title".to_string(),
                description: String::new(),
                pinned: false,
                pin_order: None,
                category: None,
                targets_json: None,
                sections_json: "[]".to_string(),
            })],
            ..Default::default()
        };
        let result = apply_legacy_import_item(
            db.pool(),
            LegacyImportItem {
                id: "item-2",
                run_id: "run-2",
                source_path: "templates.json",
                source_kind: "template",
                source_sha256: "hash-2",
            },
            &changed_batch,
            false,
        )
        .await
        .unwrap();

        assert_eq!(result.matched_count, 1);
        assert_eq!(result.conflict_count, 0);
        let target_status: String = sqlx::query_scalar(
            "SELECT status FROM migration_import_targets WHERE run_id = 'run-2' AND target_id = 'default-daily-standup'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(target_status, "retained_existing");
        assert_eq!(
            finish_legacy_import_run(db.pool(), "run-2").await.unwrap(),
            "completed"
        );

        let title: String =
            sqlx::query_scalar("SELECT title FROM templates WHERE id = 'default-daily-standup'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(title, "My Standup");
    }
}
