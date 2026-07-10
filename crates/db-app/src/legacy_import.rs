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
    pub skipped_count: i64,
    pub conflict_count: i64,
}

pub async fn begin_legacy_import_run(
    pool: &SqlitePool,
    run_id: &str,
    source_root: &str,
    dry_run: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO migration_import_runs \
         (id, importer_version, source_root, dry_run, status) \
         VALUES (?, ?, ?, ?, 'running')",
    )
    .bind(run_id)
    .bind(LEGACY_IMPORTER_VERSION)
    .bind(source_root)
    .bind(dry_run)
    .execute(pool)
    .await?;

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
             AND item.status IN ('complete', 'conflict')
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
          imported_count, skipped_count, conflict_count, error, completed_at) \
         SELECT ?, ?, previous.source_path, previous.source_kind, previous.source_sha256, \
                'unchanged', previous.discovered_count, 0, 0, 0, '', \
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         FROM migration_import_items AS previous \
         JOIN migration_import_runs AS previous_run ON previous_run.id = previous.run_id \
         WHERE previous.source_path = ? \
           AND previous.source_sha256 = ? \
           AND previous.status IN ('complete', 'conflict', 'unchanged') \
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

    if !dry_run {
        for row in &batch.rows {
            if insert_row_if_missing(&mut transaction, row).await? {
                imported_count += 1;
            }
        }
    }

    let discovered_count = i64::try_from(batch.rows.len()).unwrap_or(i64::MAX)
        + i64::try_from(batch.skipped_count).unwrap_or(i64::MAX);
    let skipped_count = i64::try_from(batch.skipped_count).unwrap_or(i64::MAX);
    let conflict_count = if dry_run {
        0
    } else {
        i64::try_from(batch.rows.len()).unwrap_or(i64::MAX) - imported_count
    };
    let status = if skipped_count > 0 || !batch.warning.is_empty() {
        "partial"
    } else if conflict_count > 0 {
        "conflict"
    } else {
        "complete"
    };

    sqlx::query(
        "INSERT INTO migration_import_items \
         (id, run_id, source_path, source_kind, source_sha256, status, \
          discovered_count, imported_count, skipped_count, conflict_count, error, completed_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
    )
    .bind(item.id)
    .bind(item.run_id)
    .bind(item.source_path)
    .bind(item.source_kind)
    .bind(item.source_sha256)
    .bind(status)
    .bind(discovered_count)
    .bind(imported_count)
    .bind(skipped_count)
    .bind(conflict_count)
    .bind(&batch.warning)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(LegacyImportItemResult {
        discovered_count,
        imported_count,
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
             skipped_count = ?,
             conflict_count = ?,
             error_count = ?,
             completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(status)
    .bind(aggregate.get::<i64, _>("discovered_count"))
    .bind(aggregate.get::<i64, _>("imported_count"))
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

async fn insert_row_if_missing(
    transaction: &mut Transaction<'_, Sqlite>,
    row: &LegacyImportRow,
) -> Result<bool, sqlx::Error> {
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
             (id, owner_user_id, name, memo, pinned, pin_order, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
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
             (id, owner_user_id, organization_id, name, email, phone, job_title, \
              linkedin_username, memo, pinned, pin_order, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
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
             (id, owner_user_id, title, created_at, updated_at, started_at, ended_at, \
              event_id, external_event_id, external_provider, series_id, event_json, folder_path) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
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
             (id, session_id, kind, template_id, title, body_format, body, source_hash, sort_order, \
              created_by, updated_by, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
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
        .bind(&row.created_by)
        .bind(&row.created_at)
        .bind(&row.updated_at)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::Transcript(row) => sqlx::query(
            "INSERT INTO transcripts \
             (id, owner_user_id, session_id, started_at_ms, ended_at_ms, memo, words_json, \
              speaker_hints_json, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
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
        .bind(&row.created_at)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::Participant(row) => sqlx::query(
            "INSERT INTO session_participants \
             (id, owner_user_id, session_id, human_id, source) \
             VALUES (?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.owner_user_id)
        .bind(&row.session_id)
        .bind(&row.human_id)
        .bind(&row.source)
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::ActionItem(row) => sqlx::query(
            "INSERT INTO action_items \
             (id, created_by, session_id, source_type, source_id, source_order, status, text, body_json, due_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO NOTHING",
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
        .execute(&mut **transaction)
        .await?,
        LegacyImportRow::Attachment(row) => sqlx::query(
            "INSERT INTO session_attachments \
             (id, session_id, filename, relative_path, content_type, size_bytes, sha256, source_type, source_id) \
             VALUES (?, ?, ?, ?, ?, ?, ?, 'legacy_file', ?) ON CONFLICT(id) DO NOTHING",
        )
        .bind(&row.id)
        .bind(&row.session_id)
        .bind(&row.filename)
        .bind(&row.relative_path)
        .bind(&row.content_type)
        .bind(row.size_bytes)
        .bind(&row.sha256)
        .bind(&row.source_id)
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

    Ok(result.rows_affected() > 0)
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
    async fn import_item_is_atomic_and_existing_sqlite_rows_win() {
        let db = test_db().await;
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

        begin_legacy_import_run(db.pool(), "run-2", "/vault", false)
            .await
            .unwrap();
        let result = apply_legacy_import_item(
            db.pool(),
            LegacyImportItem {
                id: "item-2",
                run_id: "run-2",
                source_path: "sessions/session-1/_meta.json",
                source_kind: "session_meta",
                source_sha256: "hash-2",
            },
            &session_batch(),
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
        apply_legacy_import_item(
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

        let title: String =
            sqlx::query_scalar("SELECT title FROM templates WHERE id = 'default-daily-standup'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(title, "My Standup");
    }
}
