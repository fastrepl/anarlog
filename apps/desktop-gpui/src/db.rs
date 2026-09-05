use std::path::{Path, PathBuf};
use std::sync::Arc;

use anlg_db_core::Db;
use anyhow::Context as _;

use crate::document::{self, Block};
use crate::timeline::{EventRow, SessionRow};

const DB_FILENAME: &str = "app.db";

// Same rows the Tauri sidebar reads (apps/desktop/src/calendar/queries.ts,
// `useTimelineSessionsTable`), minus the tags aggregate it only shows behind a
// setting. Ordering is applied afterwards by `timeline::build`, as in the app.
const TIMELINE_SESSIONS_SQL: &str = "
    SELECT id, title, created_at, event_json, folder_path AS folder_id, locked
    FROM sessions
    WHERE deleted_at IS NULL
    ORDER BY created_at, id
";

// apps/desktop/src/calendar/queries.ts, `useTimelineEventsTable`.
const TIMELINE_EVENTS_SQL: &str = "
    SELECT
      event.id,
      event.title,
      event.started_at,
      event.ended_at,
      event.tracking_id_event,
      event.is_all_day,
      event.meeting_link,
      COALESCE(calendar.color, '') AS calendar_color
    FROM events AS event
    LEFT JOIN calendars AS calendar
      ON calendar.id = event.calendar_id AND calendar.deleted_at IS NULL
    WHERE event.deleted_at IS NULL
    ORDER BY event.started_at, event.id
";

// apps/desktop/src/session/queries/enhanced-notes.ts, `useEnhancedNoteRecords`.
const ENHANCED_NOTES_SQL: &str = "
    SELECT id, title, body, body_format
    FROM session_documents
    WHERE session_id = ?
      AND kind IN ('summary', 'template_output')
      AND deleted_at IS NULL
    ORDER BY sort_order, id
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteDocument {
    pub id: String,
    pub title: String,
    pub blocks: Vec<Block>,
}

/// `DEFAULT_USER_ID` in `apps/desktop/src/shared/utils.ts`.
const DEFAULT_USER_ID: &str = "00000000-0000-0000-0000-000000000000";

// apps/desktop/src/session/queries/creation.ts, `createSession`.
const CREATE_SESSION_SQL: &str = "
    INSERT INTO sessions (
      id, workspace_id, owner_user_id, title, event_json, folder_path,
      created_at, updated_at, deleted_at
    ) VALUES (
      ?, NULLIF((
        SELECT json_extract(value_json, '$.workspace_id')
        FROM app_settings
        WHERE id = 'cloudsync_workspace_binding'
      ), ''), COALESCE(
        NULLIF(NULLIF(?, ''), '00000000-0000-0000-0000-000000000000'),
        NULLIF((
          SELECT json_extract(value_json, '$.workspace_id')
          FROM app_settings
          WHERE id = 'cloudsync_workspace_binding'
        ), '')
      ), ?, ?, ?, ?, ?, NULL
    )
";

// `createEmptyNoteStatement`.
const CREATE_EMPTY_NOTE_SQL: &str = "
    INSERT INTO session_documents (
      id, workspace_id, session_id, kind, body_format, body, created_by,
      updated_by, created_at, updated_at, deleted_at
    )
    SELECT ?, workspace_id, id, 'note', 'prosemirror_json', ?,
      owner_user_id, owner_user_id, ?, ?, NULL
    FROM sessions
    WHERE id = ? AND deleted_at IS NULL
";

const UPSERT_OWNER_HUMAN_SQL: &str = "
    INSERT INTO humans (
      id, workspace_id, owner_user_id, updated_at, deleted_at
    )
    SELECT session.owner_user_id, session.workspace_id,
      session.owner_user_id, ?, NULL
    FROM sessions AS session
    WHERE session.id = ? AND session.deleted_at IS NULL
    ON CONFLICT(id) DO UPDATE SET
      deleted_at = NULL,
      updated_at = excluded.updated_at
";

const INSERT_OWNER_PARTICIPANT_SQL: &str = "
    INSERT INTO session_participants (
      id, workspace_id, owner_user_id, session_id, human_id, source,
      created_at, updated_at, deleted_at
    )
    SELECT ?, session.workspace_id, session.owner_user_id, session.id,
      session.owner_user_id, 'manual', ?, ?, NULL
    FROM sessions AS session
    WHERE session.id = ? AND session.deleted_at IS NULL
";

// apps/desktop/src/settings/queries.ts, `SETTING_ROWS_SQL`: synced rows sort
// after device rows so a last-write-wins map prefers the synced value.
const SETTING_ROWS_SQL: &str = "
    SELECT id, value_json, 0 AS source_rank FROM app_settings
    UNION ALL
    SELECT id, value_json, 1 AS source_rank FROM synced_preferences
    ORDER BY id, source_rank
";

/// The provider settings the toast host reads (`useConfigValues`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderSettings {
    pub llm_provider: Option<String>,
    pub llm_model: Option<String>,
    pub stt_provider: Option<String>,
    pub stt_model: Option<String>,
}

impl ProviderSettings {
    /// `parseSettingRows` for string settings: the direct row wins, then the
    /// `legacy_settings_document` path `ai.<key>`.
    pub fn from_rows(rows: &[(String, String)]) -> Self {
        let direct: std::collections::HashMap<&str, &str> = rows
            .iter()
            .map(|(id, json)| (id.as_str(), json.as_str()))
            .collect();
        let legacy = direct
            .get("legacy_settings_document")
            .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
            .unwrap_or(serde_json::Value::Null);
        let read = |key: &str| -> Option<String> {
            let direct_value = direct
                .get(key)
                .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
                .and_then(|value| value.as_str().map(str::to_string));
            direct_value.or_else(|| {
                legacy
                    .get("ai")
                    .and_then(|ai| ai.get(key))
                    .and_then(|value| value.as_str().map(str::to_string))
            })
        };
        Self {
            llm_provider: read("current_llm_provider"),
            llm_model: read("current_llm_model"),
            stt_provider: read("current_stt_provider"),
            stt_model: read("current_stt_model"),
        }
    }

    /// `hasLLMConfigured`
    pub fn has_llm(&self) -> bool {
        self.llm_provider.as_deref().is_some_and(|p| !p.is_empty())
            && self.llm_model.as_deref().is_some_and(|m| !m.is_empty())
    }

    /// `isConfiguredSttModel` in `apps/desktop/src/stt/capabilities.ts`.
    pub fn has_stt(&self) -> bool {
        let (Some(provider), Some(model)) =
            (self.stt_provider.as_deref(), self.stt_model.as_deref())
        else {
            return false;
        };
        if provider.is_empty() || model.is_empty() {
            return false;
        }
        match provider {
            "anarlog" => model == "cloud" || is_supported_local_stt_model(model),
            "soniqo" => model.starts_with("soniqo-"),
            "apple_speech" => model == "apple-speech",
            "local_file" => model == "local-file",
            _ => true,
        }
    }

    /// `isAnarlogCloudSttModel`
    pub fn has_pro_stt(&self) -> bool {
        self.stt_provider.as_deref() == Some("anarlog")
            && self.stt_model.as_deref() == Some("cloud")
    }

    /// `hasProLlmConfigured`
    pub fn has_pro_llm(&self) -> bool {
        self.llm_provider.as_deref() == Some("anarlog")
    }
}

/// `isSupportedLocalSttModel`
fn is_supported_local_stt_model(model: &str) -> bool {
    model.starts_with("soniqo-")
        || model == "apple-speech"
        || model.starts_with("am-")
        || model.starts_with("Quantized")
}

// apps/desktop/src/session/queries/deletion.ts, `isSessionEmpty`.
const SESSION_EMPTY_SQL: &str = "
    SELECT
      sessions.title,
      sessions.event_json,
      COALESCE(note.body, '') AS note_body,
      COALESCE(note.body_format, '') AS note_body_format,
      (
        SELECT COUNT(*)
        FROM transcripts
        WHERE session_id = sessions.id AND deleted_at IS NULL
      ) AS transcript_count,
      (
        SELECT COUNT(*)
        FROM session_documents
        WHERE session_id = sessions.id
          AND kind IN ('summary', 'template_output')
          AND deleted_at IS NULL
      ) AS enhanced_note_count,
      (
        SELECT COUNT(*)
        FROM session_documents
        WHERE session_id = sessions.id
          AND kind = 'meeting_chat'
          AND deleted_at IS NULL
      ) AS meeting_chat_count,
      (
        SELECT COUNT(*)
        FROM session_participants
        WHERE session_id = sessions.id
          AND source NOT IN ('auto', 'excluded')
          AND human_id <> sessions.owner_user_id
          AND deleted_at IS NULL
      ) AS manual_participant_count,
      (
        SELECT COUNT(*)
        FROM session_tags
        WHERE session_id = sessions.id AND deleted_at IS NULL
      ) AS tag_count
    FROM sessions
    LEFT JOIN session_documents AS note
      ON note.id = sessions.id
      AND note.kind = 'note'
      AND note.deleted_at IS NULL
    WHERE sessions.id = ? AND sessions.deleted_at IS NULL
    LIMIT 1
";

/// `buildSessionTombstoneStatements` tables, in order.
const TOMBSTONE_TABLES: [&str; 6] = [
    "session_documents",
    "transcripts",
    "session_participants",
    "session_tags",
    "action_items",
    "session_attachments",
];

/// `hasNoteContent`: a note counts as written once its Markdown rendering has
/// anything but whitespace or a bare `&nbsp;`.
fn has_note_content(body: &str, format: &str) -> bool {
    if body.is_empty() {
        return false;
    }
    let markdown = if format == "prosemirror_json" {
        serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|json| anlg_tiptap::tiptap_json_to_md(&json).ok())
            .unwrap_or_else(|| body.to_string())
    } else {
        body.to_string()
    };
    let markdown = markdown.trim();
    !markdown.is_empty() && markdown != "&nbsp;"
}

// apps/desktop/src/session/queries/sessions.ts, `updateSession({ raw_md })`.
const UPSERT_MEMO_SQL: &str = "
    INSERT INTO session_documents (
      id, workspace_id, session_id, kind, template_id, body_format, body,
      created_by, updated_by, created_at, updated_at, deleted_at
    )
    SELECT ?, workspace_id, id, 'note', ?, 'prosemirror_json', ?,
      owner_user_id, owner_user_id, ?, ?, NULL
    FROM sessions
    WHERE id = ? AND deleted_at IS NULL
    ON CONFLICT(id) DO UPDATE SET
      body_format = excluded.body_format,
      body = excluded.body,
      updated_by = excluded.updated_by,
      updated_at = excluded.updated_at,
      deleted_at = NULL
";

// apps/desktop/src/session/queries/sessions.ts, `updateSession({ title })`.
const UPDATE_TITLE_SQL: &str = "
    UPDATE sessions
    SET title = ?, updated_at = ?
    WHERE id = ? AND deleted_at IS NULL
";

// `getOrCreateSessionForEventId`.
const EVENT_FOR_SESSION_SQL: &str = "
    SELECT
      id,
      tracking_id_event,
      calendar_id,
      title,
      started_at,
      ended_at,
      location,
      meeting_link,
      description,
      recurrence_series_id,
      has_recurrence_rules,
      is_all_day,
      provider,
      participants_json
    FROM events
    WHERE id = ? AND deleted_at IS NULL
    LIMIT 1
";

// `findSessionForEvent`.
const FIND_SESSION_FOR_EVENT_SQL: &str = "
    SELECT id
    FROM sessions
    WHERE deleted_at IS NULL
      AND (event_id = ? OR (? <> '' AND external_event_id = ?))
    ORDER BY CASE WHEN id = ? THEN 0 ELSE 1 END, created_at, id
    LIMIT 1
";

const CREATE_EVENT_SESSION_SQL: &str = "
    INSERT INTO sessions (
      id, workspace_id, owner_user_id, title, created_at, updated_at,
      started_at, ended_at, event_id, external_event_id, external_provider,
      series_id, event_json, deleted_at
    )
    SELECT ?, NULLIF((
      SELECT json_extract(value_json, '$.workspace_id')
      FROM app_settings
      WHERE id = 'cloudsync_workspace_binding'
    ), ''), COALESCE(
      NULLIF(NULLIF(?, ''), '00000000-0000-0000-0000-000000000000'),
      NULLIF((
        SELECT json_extract(value_json, '$.workspace_id')
        FROM app_settings
        WHERE id = 'cloudsync_workspace_binding'
      ), '')
    ), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL
    WHERE NOT EXISTS (
      SELECT 1
      FROM sessions
      WHERE deleted_at IS NULL
        AND (event_id = ? OR (? <> '' AND external_event_id = ?))
    )
";

const INSERT_PARTICIPANT_HUMAN_SQL: &str = "
    INSERT INTO humans (
      id, workspace_id, owner_user_id, name, email, created_at,
      updated_at, deleted_at
    )
    SELECT ?, session.workspace_id, session.owner_user_id, ?, ?, ?, ?, NULL
    FROM sessions AS session
    WHERE session.id = ? AND session.deleted_at IS NULL
      AND NOT EXISTS (
        SELECT 1
        FROM humans
        WHERE lower(email) = lower(?) AND deleted_at IS NULL
      )
";

const INSERT_EVENT_PARTICIPANT_SQL: &str = "
    INSERT INTO session_participants (
      id, workspace_id, owner_user_id, session_id, human_id, display_name,
      email, source, created_at, updated_at, deleted_at
    )
    SELECT ?, session.workspace_id, session.owner_user_id, session.id,
      ?, ?, ?, 'auto', ?, ?, NULL
    FROM sessions AS session
    WHERE session.id = ? AND session.deleted_at IS NULL
      AND NOT EXISTS (
        SELECT 1
        FROM session_participants
        WHERE session_id = session.id AND human_id = ? AND deleted_at IS NULL
      )
";

/// `sessionEventSchema` in `packages/store/src/zod.ts`; field order matches
/// `toSessionEvent` so `JSON.stringify` output is byte-identical.
#[derive(serde::Serialize)]
struct StoredSessionEvent {
    tracking_id: String,
    calendar_id: String,
    title: String,
    started_at: String,
    ended_at: String,
    is_all_day: bool,
    has_recurrence_rules: bool,
    location: String,
    meeting_link: String,
    description: String,
    recurrence_series_id: String,
}

/// `eventParticipantSchema`: extra keys are ignored, missing ones are `None`.
#[derive(serde::Deserialize)]
struct EventParticipant {
    name: Option<String>,
    email: Option<String>,
}

fn parse_event_participants(value: Option<&str>) -> Vec<EventParticipant> {
    let Some(value) = value else {
        return Vec::new();
    };
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str::<serde_json::Value>(value)
    else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|item| serde_json::from_value::<EventParticipant>(item).ok())
        .collect()
}

#[derive(sqlx::FromRow)]
struct EventSqlRow {
    id: String,
    tracking_id_event: String,
    calendar_id: String,
    title: String,
    started_at: String,
    ended_at: String,
    location: String,
    meeting_link: String,
    description: String,
    recurrence_series_id: String,
    has_recurrence_rules: i64,
    is_all_day: i64,
    provider: String,
    participants_json: Option<String>,
}

// apps/desktop/src/session/queries/sessions.ts, `useSessionTranscriptExistence`.
const HAS_TRANSCRIPT_SQL: &str = "
    SELECT EXISTS (
        SELECT 1
        FROM transcripts
        WHERE session_id = ?
          AND deleted_at IS NULL
          AND CASE
            WHEN json_valid(words_json) THEN json_array_length(words_json)
            ELSE 0
          END > 0
    )
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotePreview {
    pub session: SessionRow,
    /// The raw memo (`kind = 'note'`).
    pub memo: Vec<Block>,
    /// The memo as TipTap JSON (`mapSessionRow` converts imported Markdown
    /// with `md2json`); what the editor loads and writes back.
    pub memo_body: String,
    /// Summaries and template outputs in the order the app tabs them.
    pub enhanced: Vec<NoteDocument>,
    pub has_transcript: bool,
}

/// Access to the SQLite database shared with the Tauri desktop app.
///
/// The GPUI shell coexists with the Tauri app during the migration: it never
/// runs migrations (the Tauri app stays the schema owner) and every write it
/// performs uses the same statements the Tauri frontend issues, so both apps
/// produce identical rows.
pub struct Store {
    runtime: tokio::runtime::Handle,
    db: Arc<Db>,
    path: PathBuf,
    changes: tokio::sync::watch::Receiver<u64>,
}

const CHANGE_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(750);

impl Store {
    pub async fn open(runtime: tokio::runtime::Handle, path: PathBuf) -> anyhow::Result<Self> {
        if !path.is_file() {
            anyhow::bail!(
                "no Anarlog database at {}. Launch the desktop app once to create it, or pass --db-path.",
                path.display()
            );
        }
        let db = Arc::new(
            Db::connect_local_read_write(&path)
                .await
                .with_context(|| format!("failed to open {}", path.display()))?,
        );
        let changes = spawn_change_watcher(&runtime, db.clone()).await?;
        Ok(Self {
            runtime,
            db,
            path,
            changes,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Ticks whenever another process commits to the database. This is how
    /// the shell stays in step with the Tauri app's writes; in-process change
    /// notifications (`anlg-db-reactive`) cannot see other connections.
    pub fn changes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.changes.clone()
    }

    /// Runs the sqlx futures on the tokio runtime and hands back a handle the
    /// GPUI foreground executor can await. Returns the two tables the Tauri
    /// timeline merges: sessions and calendar events.
    pub fn list_timeline(
        &self,
    ) -> tokio::task::JoinHandle<anyhow::Result<(Vec<SessionRow>, Vec<EventRow>)>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let sessions = sqlx::query_as::<_, SessionRow>(TIMELINE_SESSIONS_SQL)
                .fetch_all(db.pool())
                .await?;
            let events = sqlx::query_as::<_, EventRow>(TIMELINE_EVENTS_SQL)
                .fetch_all(db.pool())
                .await?;
            Ok((sessions, events))
        })
    }

    /// `createSession("")` from the Tauri frontend: the session, its empty
    /// memo, the owner's human row, and the owner participant in one
    /// transaction. Returns the new session id.
    pub fn create_note(&self) -> tokio::task::JoinHandle<anyhow::Result<String>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let session_id = uuid::Uuid::new_v4().to_string();
            let participant_id = uuid::Uuid::new_v4().to_string();
            // `new Date().toISOString()`
            let now = chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();

            let mut transaction = db.pool().begin().await?;
            sqlx::query(CREATE_SESSION_SQL)
                .bind(&session_id)
                .bind(DEFAULT_USER_ID)
                .bind("")
                .bind("")
                .bind("")
                .bind(&now)
                .bind(&now)
                .execute(&mut *transaction)
                .await?;
            sqlx::query(CREATE_EMPTY_NOTE_SQL)
                .bind(&session_id)
                .bind("")
                .bind(&now)
                .bind(&now)
                .bind(&session_id)
                .execute(&mut *transaction)
                .await?;
            sqlx::query(UPSERT_OWNER_HUMAN_SQL)
                .bind(&now)
                .bind(&session_id)
                .execute(&mut *transaction)
                .await?;
            sqlx::query(INSERT_OWNER_PARTICIPANT_SQL)
                .bind(&participant_id)
                .bind(&now)
                .bind(&now)
                .bind(&session_id)
                .execute(&mut *transaction)
                .await?;
            transaction.commit().await?;
            Ok(session_id)
        })
    }

    /// `useConfigValues` for the provider keys the toast host needs.
    pub fn load_provider_settings(
        &self,
    ) -> tokio::task::JoinHandle<anyhow::Result<ProviderSettings>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let rows = sqlx::query_as::<_, (String, String, i64)>(SETTING_ROWS_SQL)
                .fetch_all(db.pool())
                .await?
                .into_iter()
                .map(|(id, json, _)| (id, json))
                .collect::<Vec<_>>();
            Ok(ProviderSettings::from_rows(&rows))
        })
    }

    /// `createSessionTabCloseHandler`: a session whose tab closes while it is
    /// still empty (`isSessionEmpty`) is tombstoned with
    /// `softDeleteSession`. Returns whether it was deleted.
    pub fn close_empty_session(
        &self,
        session_id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<bool>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let pool = db.pool();
            let row = sqlx::query_as::<_, (String, String, String, String, i64, i64, i64, i64, i64)>(
                SESSION_EMPTY_SQL,
            )
            .bind(&session_id)
            .fetch_optional(pool)
            .await?;
            let Some((
                title,
                event_json,
                note_body,
                note_body_format,
                transcripts,
                enhanced,
                chats,
                manual_participants,
                tags,
            )) = row
            else {
                return Ok(false);
            };
            // Same early returns as `isSessionEmpty`.
            if !title.trim().is_empty() && event_json.is_empty() {
                return Ok(false);
            }
            if has_note_content(&note_body, &note_body_format) {
                return Ok(false);
            }
            if [transcripts, enhanced, chats, manual_participants, tags]
                .iter()
                .any(|count| *count != 0)
            {
                return Ok(false);
            }

            let tombstone = chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();
            let mut transaction = pool.begin().await?;
            for table in TOMBSTONE_TABLES {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "UPDATE {table} SET deleted_at = ?, updated_at = ? WHERE session_id = ? AND deleted_at IS NULL"
                )))
                .bind(&tombstone)
                .bind(&tombstone)
                .bind(&session_id)
                .execute(&mut *transaction)
                .await?;
            }
            sqlx::query(
                "UPDATE entity_mentions
                 SET deleted_at = ?, updated_at = ?
                 WHERE (
                   (source_type = 'session' AND source_id = ?)
                   OR (target_type = 'session' AND target_id = ?)
                 ) AND deleted_at IS NULL",
            )
            .bind(&tombstone)
            .bind(&tombstone)
            .bind(&session_id)
            .bind(&session_id)
            .execute(&mut *transaction)
            .await?;
            let result = sqlx::query(
                "UPDATE sessions SET deleted_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
            )
            .bind(&tombstone)
            .bind(&tombstone)
            .bind(&session_id)
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await?;
            Ok(result.rows_affected() == 1)
        })
    }

    /// `updateSession(sessionId, { raw_md })`: the memo upsert.
    pub fn update_memo(
        &self,
        session_id: String,
        body: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let now = chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();
            sqlx::query(UPSERT_MEMO_SQL)
                .bind(&session_id)
                .bind("")
                .bind(&body)
                .bind(&now)
                .bind(&now)
                .bind(&session_id)
                .execute(db.pool())
                .await?;
            Ok(())
        })
    }

    /// `updateSession(sessionId, { title })`.
    pub fn update_title(
        &self,
        session_id: String,
        title: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let now = chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();
            sqlx::query(UPDATE_TITLE_SQL)
                .bind(&title)
                .bind(&now)
                .bind(&session_id)
                .execute(db.pool())
                .await?;
            Ok(())
        })
    }

    /// `getOrCreateSessionForEventId`: open the session backing a calendar
    /// event, creating it (with participants from the event) if none exists.
    pub fn open_event_session(
        &self,
        event_id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<String>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let pool = db.pool();
            let Some(event) = sqlx::query_as::<_, EventSqlRow>(EVENT_FOR_SESSION_SQL)
                .bind(&event_id)
                .fetch_optional(pool)
                .await?
            else {
                anyhow::bail!("calendar event {event_id} no longer exists");
            };

            let find_existing = |preferred: String| {
                sqlx::query_scalar::<_, String>(FIND_SESSION_FOR_EVENT_SQL)
                    .bind(event.id.clone())
                    .bind(event.tracking_id_event.clone())
                    .bind(event.tracking_id_event.clone())
                    .bind(preferred)
                    .fetch_optional(pool)
            };
            if let Some(existing) = find_existing(String::new()).await? {
                return Ok(existing);
            }

            let session_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();
            let session_event = StoredSessionEvent {
                tracking_id: event.tracking_id_event.clone(),
                calendar_id: event.calendar_id.clone(),
                title: event.title.clone(),
                started_at: event.started_at.clone(),
                ended_at: event.ended_at.clone(),
                is_all_day: event.is_all_day != 0,
                has_recurrence_rules: event.has_recurrence_rules != 0,
                location: event.location.clone(),
                meeting_link: event.meeting_link.clone(),
                description: event.description.clone(),
                recurrence_series_id: event.recurrence_series_id.clone(),
            };
            let participants = parse_event_participants(event.participants_json.as_deref());

            // `findHumansByEmail`
            let emails: Vec<String> = {
                let mut seen = std::collections::BTreeSet::new();
                participants
                    .iter()
                    .filter_map(|p| p.email.as_deref())
                    .map(|email| email.trim().to_lowercase())
                    .filter(|email| !email.is_empty() && seen.insert(email.clone()))
                    .collect()
            };
            let mut humans_by_email = std::collections::HashMap::new();
            if !emails.is_empty() {
                let placeholders = vec!["?"; emails.len()].join(", ");
                let sql = format!(
                    "SELECT id, email FROM humans WHERE deleted_at IS NULL AND lower(email) IN ({placeholders}) ORDER BY id"
                );
                let mut query = sqlx::query_as::<_, (String, String)>(sqlx::AssertSqlSafe(sql));
                for email in &emails {
                    query = query.bind(email);
                }
                for (id, email) in query.fetch_all(pool).await? {
                    humans_by_email.insert(email.to_lowercase(), id);
                }
            }

            let mut transaction = pool.begin().await?;
            sqlx::query(CREATE_EVENT_SESSION_SQL)
                .bind(&session_id)
                .bind(DEFAULT_USER_ID)
                .bind(&session_event.title)
                .bind(&now)
                .bind(&now)
                .bind(&session_event.started_at)
                .bind(&session_event.ended_at)
                .bind(&event.id)
                .bind(&event.tracking_id_event)
                .bind(&event.provider)
                .bind(&event.recurrence_series_id)
                .bind(serde_json::to_string(&session_event)?)
                .bind(&event.id)
                .bind(&event.tracking_id_event)
                .bind(&event.tracking_id_event)
                .execute(&mut *transaction)
                .await?;
            sqlx::query(CREATE_EMPTY_NOTE_SQL)
                .bind(&session_id)
                .bind("")
                .bind(&now)
                .bind(&now)
                .bind(&session_id)
                .execute(&mut *transaction)
                .await?;

            let mut seen_emails = std::collections::HashSet::new();
            for participant in &participants {
                let Some(email) = participant.email.as_deref().map(str::trim).filter(|e| !e.is_empty())
                else {
                    continue;
                };
                let key = email.to_lowercase();
                if !seen_emails.insert(key.clone()) {
                    continue;
                }
                let display_name = participant
                    .name
                    .as_deref()
                    .filter(|name| !name.is_empty())
                    .unwrap_or(email);
                let human_id = match humans_by_email.get(&key) {
                    Some(id) => id.clone(),
                    None => {
                        let human_id = uuid::Uuid::new_v4().to_string();
                        sqlx::query(INSERT_PARTICIPANT_HUMAN_SQL)
                            .bind(&human_id)
                            .bind(display_name)
                            .bind(email)
                            .bind(&now)
                            .bind(&now)
                            .bind(&session_id)
                            .bind(email)
                            .execute(&mut *transaction)
                            .await?;
                        human_id
                    }
                };
                sqlx::query(INSERT_EVENT_PARTICIPANT_SQL)
                    .bind(uuid::Uuid::new_v4().to_string())
                    .bind(&human_id)
                    .bind(display_name)
                    .bind(email)
                    .bind(&now)
                    .bind(&now)
                    .bind(&session_id)
                    .bind(&human_id)
                    .execute(&mut *transaction)
                    .await?;
            }
            transaction.commit().await?;

            find_existing(session_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("failed to create a session for event {event_id}"))
        })
    }

    pub fn load_note(
        &self,
        session_id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<Option<NotePreview>>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let Some(session) = anlg_db_app::get_session(db.pool(), &session_id).await? else {
                return Ok(None);
            };
            let (memo, memo_body) = match anlg_db_app::get_session_note(db.pool(), &session_id)
                .await?
            {
                Some(note) => {
                    let body = if note.body_format == "markdown" && !note.body.trim().is_empty() {
                        anlg_tiptap::md_to_tiptap_json(&note.body)
                            .ok()
                            .and_then(|json| serde_json::to_string(&json).ok())
                            .unwrap_or_else(|| note.body.clone())
                    } else {
                        note.body.clone()
                    };
                    (document::from_body(&note.body_format, &note.body), body)
                }
                None => (Vec::new(), String::new()),
            };
            let enhanced =
                sqlx::query_as::<_, (String, String, String, String)>(ENHANCED_NOTES_SQL)
                    .bind(&session_id)
                    .fetch_all(db.pool())
                    .await?
                    .into_iter()
                    .map(|(id, title, body, body_format)| NoteDocument {
                        id,
                        title,
                        blocks: document::from_body(&body_format, &body),
                    })
                    .collect();
            let has_transcript: bool = sqlx::query_scalar(HAS_TRANSCRIPT_SQL)
                .bind(&session_id)
                .fetch_one(db.pool())
                .await?;
            Ok(Some(NotePreview {
                has_transcript,
                session: SessionRow {
                    id: session.id,
                    title: session.title,
                    created_at: session.created_at,
                    event_json: session.event_json,
                    folder_id: session.folder_path,
                    locked: session.locked,
                },
                memo,
                memo_body,
                enhanced,
            }))
        })
    }
}

/// `PRAGMA data_version` is scoped to one connection and increments when any
/// other connection commits, so the watcher holds a dedicated connection.
async fn spawn_change_watcher(
    runtime: &tokio::runtime::Handle,
    db: Arc<Db>,
) -> anyhow::Result<tokio::sync::watch::Receiver<u64>> {
    let mut connection = db
        .pool()
        .acquire()
        .await
        .context("failed to acquire change-watcher connection")?;
    let mut last = data_version(&mut connection).await?;
    let (tx, rx) = tokio::sync::watch::channel(0u64);
    runtime.spawn(async move {
        let mut generation = 0u64;
        loop {
            tokio::time::sleep(CHANGE_POLL_INTERVAL).await;
            match data_version(&mut connection).await {
                Ok(version) if version != last => {
                    last = version;
                    generation += 1;
                    if tx.send(generation).is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!(%error, "database change watcher failed; stopping");
                    break;
                }
            }
        }
    });
    Ok(rx)
}

async fn data_version(
    connection: &mut sqlx::pool::PoolConnection<sqlx::Sqlite>,
) -> anyhow::Result<i64> {
    let version: i64 = sqlx::query_scalar("PRAGMA data_version")
        .fetch_one(&mut **connection)
        .await?;
    Ok(version)
}

/// Resolves the same `app.db` the Tauri desktop app opens for `identifier`.
///
/// Mirrors `apps/desktop/src-tauri/src/db.rs`: prefer the raw identifier
/// folder when it already holds a database and the storage default does not.
pub fn default_db_path(identifier: &str) -> anyhow::Result<PathBuf> {
    let data_dir = dirs::data_dir().context("application data directory is unavailable")?;
    let default_dir = anlg_storage::global::compute_default_base(identifier)
        .context("application data directory is unavailable")?;
    Ok(resolve_db_dir(&data_dir, &default_dir, identifier).join(DB_FILENAME))
}

fn resolve_db_dir(data_dir: &Path, default_dir: &Path, identifier: &str) -> PathBuf {
    let identifier_dir = data_dir.join(identifier);
    if identifier_dir.join(DB_FILENAME).is_file() && !default_dir.join(DB_FILENAME).is_file() {
        identifier_dir
    } else {
        default_dir.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_db_dir_prefers_identifier_folder_only_when_default_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path();
        let default_dir = data_dir.join("anarlog");
        let identifier_dir = data_dir.join("com.hyprnote.stable");

        assert_eq!(
            resolve_db_dir(data_dir, &default_dir, "com.hyprnote.stable"),
            default_dir
        );

        std::fs::create_dir_all(&identifier_dir).unwrap();
        std::fs::write(identifier_dir.join(DB_FILENAME), "").unwrap();
        assert_eq!(
            resolve_db_dir(data_dir, &default_dir, "com.hyprnote.stable"),
            identifier_dir
        );

        std::fs::create_dir_all(&default_dir).unwrap();
        std::fs::write(default_dir.join(DB_FILENAME), "").unwrap();
        assert_eq!(
            resolve_db_dir(data_dir, &default_dir, "com.hyprnote.stable"),
            default_dir
        );
    }

    #[tokio::test]
    async fn store_reads_sessions_written_by_the_app_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILENAME);
        {
            let db = Db::connect_local_plain(&path).await.unwrap();
            anlg_db_app::prepare_schema(&db).await.unwrap();
            sqlx::raw_sql(
                "INSERT INTO sessions (id, workspace_id, title, created_at, event_json)
                 VALUES ('session-1', 'workspace-1', 'Weekly sync', '2026-09-01T09:00:00Z',
                         '{\"started_at\":\"2026-09-02T10:00:00Z\"}');
                 INSERT INTO sessions (id, workspace_id, title, created_at, deleted_at)
                 VALUES ('deleted', 'workspace-1', 'Gone', '2026-09-03T09:00:00Z', '2026-09-04T00:00:00Z');
                 INSERT INTO session_documents (id, workspace_id, session_id, kind, body)
                 VALUES (
                   'session-1', 'workspace-1', 'session-1', 'note',
                   '{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\",\"content\":[{\"type\":\"text\",\"text\":\"Decide on GPUI.\"}]}]}'
                 );
                 INSERT INTO session_documents (id, workspace_id, session_id, kind, title, body_format, body, sort_order)
                 VALUES ('summary-2', 'workspace-1', 'session-1', 'summary', 'Second', 'markdown', '## Later', 2),
                        ('summary-1', 'workspace-1', 'session-1', 'template_output', 'First', 'markdown', '## Sooner', 1);
                 INSERT INTO calendars (id, color) VALUES ('cal-1', '#ff0000');
                 INSERT INTO events (id, calendar_id, title, started_at, ended_at, tracking_id_event)
                 VALUES ('event-1', 'cal-1', 'Standup', '2099-01-01T09:00:00Z', '2099-01-01T09:15:00Z', 'track-1');",
            )
            .execute(db.pool())
            .await
            .unwrap();
        }

        let store = Store::open(tokio::runtime::Handle::current(), path)
            .await
            .unwrap();
        let (sessions, events) = store.list_timeline().await.unwrap().unwrap();
        assert_eq!(sessions.len(), 1, "deleted sessions stay hidden");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Standup");
        assert_eq!(events[0].calendar_color, "#ff0000");
        assert_eq!(sessions[0].title, "Weekly sync");
        assert_eq!(
            sessions[0].event_json,
            "{\"started_at\":\"2026-09-02T10:00:00Z\"}"
        );

        let note = store
            .load_note("session-1".to_string())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(note.session.title, "Weekly sync");
        assert_eq!(note.session.created_at, "2026-09-01T09:00:00Z");
        assert!(!note.has_transcript);
        assert_eq!(
            note.memo,
            vec![Block::Paragraph(vec![document::Span {
                text: "Decide on GPUI.".into(),
                ..document::Span::default()
            }])]
        );
        assert_eq!(
            note.enhanced
                .iter()
                .map(|doc| (doc.id.as_str(), doc.title.as_str(), doc.blocks.len()))
                .collect::<Vec<_>>(),
            [("summary-1", "First", 1), ("summary-2", "Second", 1)]
        );

        assert!(
            store
                .load_note("missing".to_string())
                .await
                .unwrap()
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn create_note_writes_the_same_rows_as_the_tauri_frontend() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILENAME);
        {
            let db = Db::connect_local_plain(&path).await.unwrap();
            anlg_db_app::prepare_schema(&db).await.unwrap();
        }
        let store = Store::open(tokio::runtime::Handle::current(), path)
            .await
            .unwrap();

        let session_id = store.create_note().await.unwrap().unwrap();
        let (sessions, _) = store.list_timeline().await.unwrap().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, session_id);
        assert_eq!(sessions[0].title, "");
        assert!(sessions[0].created_at.ends_with('Z'));

        let pool = store.db.pool();
        let (kind, body_format, body): (String, String, String) = sqlx::query_as(
            "SELECT kind, body_format, body FROM session_documents WHERE id = ? AND session_id = ?",
        )
        .bind(&session_id)
        .bind(&session_id)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(
            (kind.as_str(), body_format.as_str(), body.as_str()),
            ("note", "prosemirror_json", "")
        );
        let participants: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM session_participants WHERE session_id = ? AND source = 'manual'",
        )
        .bind(&session_id)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(participants, 1);

        let note = store
            .load_note(session_id.clone())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(note.memo.is_empty());

        store
            .update_title(session_id.clone(), "Weekly sync".to_string())
            .await
            .unwrap()
            .unwrap();
        let (title, bumped): (String, bool) =
            sqlx::query_as("SELECT title, updated_at > created_at FROM sessions WHERE id = ?")
                .bind(&session_id)
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(title, "Weekly sync");
        assert!(bumped);
    }

    #[test]
    fn provider_settings_follow_the_settings_parser() {
        let rows = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
            pairs
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect()
        };
        let none = ProviderSettings::from_rows(&rows(&[("ai_language", "\"en-US\"")]));
        assert!(!none.has_stt() && !none.has_llm());

        let direct = ProviderSettings::from_rows(&rows(&[
            ("current_stt_provider", "\"soniqo\""),
            ("current_stt_model", "\"soniqo-parakeet-streaming\""),
            ("current_llm_provider", "\"openai\""),
            ("current_llm_model", "\"gpt-4o\""),
        ]));
        assert!(
            direct.has_stt() && direct.has_llm() && !direct.has_pro_stt() && !direct.has_pro_llm()
        );

        let mismatched = ProviderSettings::from_rows(&rows(&[
            ("current_stt_provider", "\"soniqo\""),
            ("current_stt_model", "\"cloud\""),
        ]));
        assert!(!mismatched.has_stt(), "soniqo needs a soniqo- model");

        let legacy = ProviderSettings::from_rows(&rows(&[(
            "legacy_settings_document",
            r#"{"ai":{"current_stt_provider":"anarlog","current_stt_model":"cloud","current_llm_provider":"anarlog","current_llm_model":"auto"}}"#,
        )]));
        assert!(legacy.has_stt() && legacy.has_pro_stt() && legacy.has_pro_llm());

        // A synced row (sorted after the device row) wins.
        let synced = ProviderSettings::from_rows(&rows(&[
            ("current_stt_model", "\"am-parakeet-v3\""),
            ("current_stt_provider", "\"anarlog\""),
        ]));
        assert!(synced.has_stt() && !synced.has_pro_stt());
    }

    #[tokio::test]
    async fn closing_an_untouched_note_deletes_it_but_written_notes_survive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILENAME);
        {
            let db = Db::connect_local_plain(&path).await.unwrap();
            anlg_db_app::prepare_schema(&db).await.unwrap();
        }
        let store = Store::open(tokio::runtime::Handle::current(), path)
            .await
            .unwrap();

        let untouched = store.create_note().await.unwrap().unwrap();
        let titled = store.create_note().await.unwrap().unwrap();
        store
            .update_title(titled.clone(), "Kept".to_string())
            .await
            .unwrap()
            .unwrap();
        let written = store.create_note().await.unwrap().unwrap();
        store
            .update_memo(
                written.clone(),
                r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"hi"}]}]}"#.to_string(),
            )
            .await
            .unwrap()
            .unwrap();
        let blank_paragraph = store.create_note().await.unwrap().unwrap();
        store
            .update_memo(
                blank_paragraph.clone(),
                r#"{"type":"doc","content":[{"type":"paragraph"}]}"#.to_string(),
            )
            .await
            .unwrap()
            .unwrap();

        assert!(
            store
                .close_empty_session(untouched.clone())
                .await
                .unwrap()
                .unwrap()
        );
        assert!(
            !store
                .close_empty_session(titled.clone())
                .await
                .unwrap()
                .unwrap()
        );
        assert!(
            !store
                .close_empty_session(written.clone())
                .await
                .unwrap()
                .unwrap()
        );
        assert!(
            store
                .close_empty_session(blank_paragraph.clone())
                .await
                .unwrap()
                .unwrap()
        );
        // Already tombstoned: nothing to do.
        assert!(
            !store
                .close_empty_session(untouched.clone())
                .await
                .unwrap()
                .unwrap()
        );

        let (sessions, _) = store.list_timeline().await.unwrap().unwrap();
        let mut ids: Vec<&str> = sessions.iter().map(|s| s.id.as_str()).collect();
        ids.sort();
        let mut expected = vec![titled.as_str(), written.as_str()];
        expected.sort();
        assert_eq!(ids, expected);
        let pool = store.db.pool();
        let tombstoned_docs: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM session_documents WHERE session_id = ? AND deleted_at IS NOT NULL",
        )
        .bind(&untouched)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(
            tombstoned_docs, 1,
            "the memo row is tombstoned with the session"
        );
    }

    #[tokio::test]
    async fn opening_an_event_creates_its_session_once_with_participants() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILENAME);
        {
            let db = Db::connect_local_plain(&path).await.unwrap();
            anlg_db_app::prepare_schema(&db).await.unwrap();
            sqlx::raw_sql(
                "INSERT INTO humans (id, email, name) VALUES ('human-ada', 'ADA@example.com', 'Ada');
                 INSERT INTO events (id, calendar_id, title, started_at, ended_at, tracking_id_event, provider, meeting_link, participants_json)
                 VALUES ('event-1', 'cal-1', 'Standup', '2099-01-01T09:00:00Z', '2099-01-01T09:15:00Z', 'track-1', 'google', 'https://meet.google.com/x',
                         '[{\"name\":\"Ada\",\"email\":\"ada@example.com\"},{\"email\":\"bob@example.com\"},{\"name\":\"No email\"},{\"email\":\"ada@example.com\"}]');",
            )
            .execute(db.pool())
            .await
            .unwrap();
        }
        let store = Store::open(tokio::runtime::Handle::current(), path)
            .await
            .unwrap();

        let session_id = store
            .open_event_session("event-1".to_string())
            .await
            .unwrap()
            .unwrap();
        let again = store
            .open_event_session("event-1".to_string())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(session_id, again, "second open reuses the session");

        let pool = store.db.pool();
        let (title, event_id, external, provider, event_json): (String, String, String, String, String) =
            sqlx::query_as(
                "SELECT title, event_id, external_event_id, external_provider, event_json FROM sessions WHERE id = ?",
            )
            .bind(&session_id)
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(
            (
                title.as_str(),
                event_id.as_str(),
                external.as_str(),
                provider.as_str()
            ),
            ("Standup", "event-1", "track-1", "google")
        );
        assert_eq!(
            event_json,
            "{\"tracking_id\":\"track-1\",\"calendar_id\":\"cal-1\",\"title\":\"Standup\",\"started_at\":\"2099-01-01T09:00:00Z\",\"ended_at\":\"2099-01-01T09:15:00Z\",\"is_all_day\":false,\"has_recurrence_rules\":false,\"location\":\"\",\"meeting_link\":\"https://meet.google.com/x\",\"description\":\"\",\"recurrence_series_id\":\"\"}"
        );

        let participants: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT human_id, display_name, email, source FROM session_participants WHERE session_id = ? ORDER BY email",
        )
        .bind(&session_id)
        .fetch_all(pool)
        .await
        .unwrap();
        assert_eq!(
            participants.len(),
            2,
            "duplicates and email-less entries are skipped"
        );
        assert_eq!(
            participants[0].0, "human-ada",
            "existing humans are matched by email, case-insensitively"
        );
        assert_eq!(participants[0].3, "auto");
        assert_eq!(
            participants[1].1, "bob@example.com",
            "name falls back to the email"
        );
        let humans: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM humans WHERE deleted_at IS NULL")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(humans, 2);

        // The timeline now lists the session instead of the event.
        let (sessions, events) = store.list_timeline().await.unwrap().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(events.len(), 1);
        let now = "2099-01-01T08:00:00Z".parse().unwrap();
        let timeline = crate::timeline::build(&sessions, &events, now, &chrono::Utc);
        let ids: Vec<&str> = timeline
            .buckets
            .iter()
            .flat_map(|b| b.items.iter().map(|i| i.id.as_str()))
            .collect();
        assert_eq!(
            ids,
            [session_id.as_str()],
            "the session replaces the event by tracking id"
        );
    }

    #[tokio::test]
    async fn store_notices_commits_from_other_connections() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILENAME);
        let writer = Db::connect_local_plain(&path).await.unwrap();
        anlg_db_app::prepare_schema(&writer).await.unwrap();

        let store = Store::open(tokio::runtime::Handle::current(), path)
            .await
            .unwrap();
        let mut changes = store.changes();
        assert_eq!(*changes.borrow(), 0);

        sqlx::query("INSERT INTO sessions (id, workspace_id, title) VALUES ('s1', 'w1', 'New')")
            .execute(writer.pool())
            .await
            .unwrap();

        tokio::time::timeout(std::time::Duration::from_secs(5), changes.changed())
            .await
            .expect("watcher should tick after an external commit")
            .unwrap();
        assert_eq!(*changes.borrow(), 1);
        assert_eq!(store.list_timeline().await.unwrap().unwrap().0.len(), 1);
    }

    #[tokio::test]
    async fn store_refuses_to_create_a_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILENAME);
        let error = Store::open(tokio::runtime::Handle::current(), path.clone())
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("--db-path"));
        assert!(!path.exists());
    }
}
