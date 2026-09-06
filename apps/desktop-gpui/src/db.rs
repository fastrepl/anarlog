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
      COALESCE(calendar.color, '') AS calendar_color,
      COALESCE(event.calendar_id, '') AS calendar_id,
      COALESCE(event.recurrence_series_id, '') AS recurrence_series_id,
      COALESCE(event.location, '') AS location,
      COALESCE(event.description, '') AS description
    FROM events AS event
    LEFT JOIN calendars AS calendar
      ON calendar.id = event.calendar_id AND calendar.deleted_at IS NULL
    WHERE event.deleted_at IS NULL
    ORDER BY event.started_at, event.id
";

// apps/desktop/src/session/queries/enhanced-notes.ts, `useEnhancedNoteRecords`.
const ENHANCED_NOTES_SQL: &str = "
    SELECT id, title, body, body_format, COALESCE(template_id, '')
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
    /// The stored body (TipTap JSON) for exports and content checks.
    pub body: String,
    /// `template_id` (`""` for Auto).
    pub template_id: String,
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
    /// `theme` (`general.theme` in the legacy document), default `system`.
    pub theme: String,
    /// Every stored row (`id` → `value_json`) for the settings pages.
    pub raw: std::collections::HashMap<String, String>,
    /// `legacy_settings_document`, already parsed.
    pub legacy: serde_json::Value,
}

/// A credential-store lookup: the key, none, or the store's error message.
pub type ApiKeyResult = Result<Option<String>, String>;

/// `AiProviderConfig` without the `type` discriminator.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AiProviderConfig {
    pub base_url: String,
    pub api_key: String,
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
        let theme = direct
            .get("theme")
            .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
            .and_then(|value| value.as_str().map(str::to_string))
            .or_else(|| {
                legacy
                    .get("general")
                    .and_then(|general| general.get("theme"))
                    .and_then(|value| value.as_str().map(str::to_string))
            })
            .filter(|theme| matches!(theme.as_str(), "light" | "dark" | "system"))
            .unwrap_or_else(|| "system".to_string());
        Self {
            llm_provider: read("current_llm_provider"),
            llm_model: read("current_llm_model"),
            stt_provider: read("current_stt_provider"),
            stt_model: read("current_stt_model"),
            theme,
            raw: rows
                .iter()
                .map(|(id, json)| (id.clone(), json.clone()))
                .collect(),
            legacy,
        }
    }

    /// A stored value: the direct row, else the legacy document at `path`.
    /// `parseAiProviders`: `ai_provider:<type>:<id>` rows over the legacy
    /// `ai.<type>.<id>` document entries, keyed by provider id. API keys are
    /// whatever plaintext the row still carries; the credential store wins.
    pub fn ai_providers(&self, kind: &str) -> std::collections::HashMap<String, AiProviderConfig> {
        let mut result = std::collections::HashMap::new();
        let normalize = |value: &serde_json::Value| -> Option<AiProviderConfig> {
            let object = value.as_object()?;
            if object.is_empty() {
                return None;
            }
            let text = |key: &str| {
                object
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_string()
            };
            Some(AiProviderConfig {
                base_url: text("base_url"),
                api_key: text("api_key"),
            })
        };
        if let Some(legacy) = self
            .legacy
            .get("ai")
            .and_then(|ai| ai.get(kind))
            .and_then(serde_json::Value::as_object)
        {
            for (provider_id, value) in legacy {
                if let Some(config) = normalize(value) {
                    result.insert(provider_id.clone(), config);
                }
            }
        }
        let prefix = format!("ai_provider:{kind}:");
        for (id, json) in &self.raw {
            let Some(provider_id) = id.strip_prefix(&prefix) else {
                continue;
            };
            if provider_id.is_empty() {
                continue;
            }
            if let Some(config) = serde_json::from_str::<serde_json::Value>(json)
                .ok()
                .as_ref()
                .and_then(normalize)
            {
                result.insert(provider_id.to_string(), config);
            }
        }
        result
    }

    pub fn value(&self, key: &str, legacy_path: &[&str]) -> Option<serde_json::Value> {
        if let Some(json) = self.raw.get(key)
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(json)
        {
            return Some(value);
        }
        let mut node = &self.legacy;
        for segment in legacy_path {
            node = node.get(segment)?;
        }
        Some(node.clone())
    }

    /// `resolveConfigValue` for a boolean setting with a schema default.
    pub fn bool_setting(&self, key: &str, legacy_path: &[&str], default: bool) -> bool {
        self.value(key, legacy_path)
            .and_then(|value| value.as_bool())
            .unwrap_or(default)
    }

    /// `resolveConfigValue` for a string setting; `None` when unset or blank.
    pub fn string_setting(&self, key: &str, legacy_path: &[&str]) -> Option<String> {
        self.value(key, legacy_path)
            .and_then(|value| value.as_str().map(str::to_string))
            .filter(|value| !value.is_empty())
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

/// `buildSessionTombstoneStatements`: sets (or, when restoring, clears)
/// `deleted_at` on the session and its rows in one transaction and returns
/// how many session rows changed.
async fn apply_tombstone(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    tombstone: &str,
    restore: bool,
) -> anyhow::Result<u64> {
    let value: Option<&str> = if restore { None } else { Some(tombstone) };
    let predicate = if restore {
        "deleted_at = ?"
    } else {
        "deleted_at IS NULL"
    };
    let mut transaction = pool.begin().await?;
    for table in TOMBSTONE_TABLES {
        let mut query = sqlx::query(sqlx::AssertSqlSafe(format!(
            "UPDATE {table} SET deleted_at = ?, updated_at = ? WHERE session_id = ? AND {predicate}"
        )))
        .bind(value)
        .bind(tombstone)
        .bind(session_id);
        if restore {
            query = query.bind(tombstone);
        }
        query.execute(&mut *transaction).await?;
    }
    let mut query = sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE entity_mentions
         SET deleted_at = ?, updated_at = ?
         WHERE (
           (source_type = 'session' AND source_id = ?)
           OR (target_type = 'session' AND target_id = ?)
         ) AND {predicate}"
    )))
    .bind(value)
    .bind(tombstone)
    .bind(session_id)
    .bind(session_id);
    if restore {
        query = query.bind(tombstone);
    }
    query.execute(&mut *transaction).await?;
    let mut query = sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE sessions SET deleted_at = ?, updated_at = ? WHERE id = ? AND {predicate}"
    )))
    .bind(value)
    .bind(tombstone)
    .bind(session_id);
    if restore {
        query = query.bind(tombstone);
    }
    let result = query.execute(&mut *transaction).await?;
    transaction.commit().await?;
    Ok(result.rows_affected())
}

/// `hasNoteContent`: a note counts as written once its Markdown rendering has
/// anything but whitespace or a bare `&nbsp;`.
pub(crate) fn has_note_content(body: &str, format: &str) -> bool {
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
// The same statement with `raw_template_id` set (`hasTemplateChange`).
const UPSERT_MEMO_WITH_TEMPLATE_SQL: &str = "
    INSERT INTO session_documents (
      id, workspace_id, session_id, kind, template_id, body_format, body,
      created_by, updated_by, created_at, updated_at, deleted_at
    )
    SELECT ?, workspace_id, id, 'note', ?, 'prosemirror_json', ?,
      owner_user_id, owner_user_id, ?, ?, NULL
    FROM sessions
    WHERE id = ? AND deleted_at IS NULL
    ON CONFLICT(id) DO UPDATE SET
      template_id = excluded.template_id,
      body_format = excluded.body_format,
      body = excluded.body,
      updated_by = excluded.updated_by,
      updated_at = excluded.updated_at,
      deleted_at = NULL
";

// apps/desktop/src/templates/queries.ts, `useUserTemplates`.
const TEMPLATES_SQL: &str = "
    SELECT id, title, pinned, pin_order, icon_json, sections_json
    FROM templates
    ORDER BY id
";

/// A `templates` row as `mapTemplateLiveRows` shapes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    pub id: String,
    pub title: String,
    pub pinned: bool,
    pub pin_order: Option<i64>,
    /// `normalizeTemplateIcon`: `(type, value, color)`.
    pub icon: TemplateIcon,
    /// Section titles, trimmed and non-empty.
    pub section_titles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateIcon {
    Emoji(String),
    Icon { name: String, color: String },
}

impl TemplateIcon {
    /// `DEFAULT_TEMPLATE_ICON`
    pub fn default_template() -> Self {
        Self::Icon {
            name: "notebook-tabs".to_string(),
            color: "#9ca3af".to_string(),
        }
    }

    /// `DEFAULT_FOLDER_ICON`
    pub fn default_folder() -> Self {
        Self::Icon {
            name: "folder".to_string(),
            color: "#9ca3af".to_string(),
        }
    }

    /// `normalizeTemplateIcon` on an already-parsed value: `None` when the
    /// shape is not an icon object.
    pub fn from_json(icon: &serde_json::Value) -> Option<Self> {
        let kind = icon.get("type")?.as_str()?;
        let value = icon.get("value")?.as_str()?.to_string();
        Some(if kind == "emoji" {
            Self::Emoji(value)
        } else if kind == "icon" {
            Self::Icon {
                name: value,
                color: icon
                    .get("color")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("#9ca3af")
                    .to_string(),
            }
        } else {
            return None;
        })
    }

    /// `isExplicitTemplateIcon`
    pub fn is_explicit(&self) -> bool {
        match self {
            Self::Emoji(value) => !value.trim().is_empty(),
            Self::Icon { name, .. } => !name.trim().is_empty(),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Emoji(value) => serde_json::json!({ "type": "emoji", "value": value }),
            Self::Icon { name, color } => {
                serde_json::json!({ "type": "icon", "value": name, "color": color })
            }
        }
    }
}

impl Template {
    fn from_row(
        (id, title, pinned, pin_order, icon_json, sections_json): (
            String,
            String,
            i64,
            Option<i64>,
            String,
            String,
        ),
    ) -> Self {
        let icon = serde_json::from_str::<serde_json::Value>(&icon_json)
            .ok()
            .and_then(|icon| TemplateIcon::from_json(&icon))
            .unwrap_or_else(TemplateIcon::default_template);
        let section_titles = serde_json::from_str::<serde_json::Value>(&sections_json)
            .ok()
            .and_then(|sections| sections.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|section| {
                section
                    .get("title")?
                    .as_str()
                    .map(str::trim)
                    .map(str::to_string)
            })
            .filter(|title| !title.is_empty())
            .collect();
        Self {
            id,
            title,
            pinned: pinned != 0,
            pin_order,
            icon,
            section_titles,
        }
    }
}

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
/// `useSessionParticipants`
const SESSION_PARTICIPANTS_SQL: &str = "
    SELECT
        participant.id,
        participant.human_id,
        participant.source,
        COALESCE(NULLIF(human.name, ''), participant.display_name) AS name,
        COALESCE(NULLIF(human.email, ''), participant.email) AS email
    FROM session_participants AS participant
    LEFT JOIN humans AS human
        ON human.id = participant.human_id AND human.deleted_at IS NULL
    WHERE participant.session_id = ?
        AND participant.deleted_at IS NULL
    ORDER BY name, email, participant.id
";

/// `useHumans`
const HUMANS_SQL: &str = "
    SELECT id, name, email, phone, job_title, organization_id
    FROM humans
    WHERE deleted_at IS NULL
    ORDER BY name, email, id
";

/// `createHuman`
const CREATE_HUMAN_SQL: &str = "
    INSERT INTO humans (
        id, workspace_id, owner_user_id, organization_id, name, email,
        phone, job_title, linkedin_username, memo, pinned, pin_order,
        metadata_json, created_at, updated_at, deleted_at
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
            ), ''),
            '00000000-0000-0000-0000-000000000000'
        ), '', ?, ?, '', '', '', '', 0, NULL, '{}', ?, ?, NULL
    )
";

/// `addSessionParticipant`, first statement.
const REVIVE_EXCLUDED_PARTICIPANT_SQL: &str = "
    UPDATE session_participants
    SET source = ?, updated_at = ?
    WHERE id = (
        SELECT id
        FROM session_participants
        WHERE session_id = ?
            AND human_id = ?
            AND source = 'excluded'
            AND deleted_at IS NULL
            AND ? <> 'auto'
        ORDER BY created_at, id
        LIMIT 1
    )
";

/// `addSessionParticipant`, second statement.
const INSERT_MANUAL_PARTICIPANT_SQL: &str = "
    INSERT INTO session_participants (
        id, workspace_id, owner_user_id, session_id, human_id,
        display_name, email, role, source, metadata_json, created_at,
        updated_at, deleted_at
    )
    SELECT ?, session.workspace_id, session.owner_user_id, session.id, human.id,
        human.name, human.email, '', ?, '{}', ?, ?, NULL
    FROM sessions AS session
    JOIN humans AS human ON human.id = ? AND human.deleted_at IS NULL
    WHERE session.id = ?
        AND session.deleted_at IS NULL
        AND NOT EXISTS (
            SELECT 1
            FROM session_participants AS existing
            WHERE existing.session_id = session.id
                AND existing.human_id = human.id
                AND existing.deleted_at IS NULL
        )
";

/// `removeSessionParticipant`
const REMOVE_PARTICIPANT_SQL: &str = "
    UPDATE session_participants
    SET
        source = CASE WHEN source = 'auto' THEN 'excluded' ELSE source END,
        deleted_at = CASE WHEN source = 'auto' THEN NULL ELSE ? END,
        updated_at = ?
    WHERE id = ? AND deleted_at IS NULL
";

/// `parseAssignedHumanId`: the hint value is JSON (or a JSON string) with a
/// `human_id`.
fn assigned_human_id(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    let parsed = match value {
        serde_json::Value::String(text) => serde_json::from_str::<serde_json::Value>(text).ok()?,
        other => other.clone(),
    };
    parsed
        .get("human_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

/// A `session_participants` row as the metadata panel lists it.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SessionParticipant {
    pub id: String,
    pub human_id: String,
    pub source: String,
    pub name: String,
    pub email: String,
}

/// A `humans` row as the participant picker searches it.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Human {
    pub id: String,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub job_title: String,
    pub organization_id: String,
}

/// Who to add: an existing contact or a name to create one for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParticipantTarget {
    Existing(String),
    New(String),
}

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

// apps/desktop/src/stt/queries.ts, `useSessionTranscripts` (stored rows;
// pending live deltas only exist while a capture is running).
const SESSION_TRANSCRIPTS_SQL: &str = "
    SELECT id, owner_user_id, started_at_ms, ended_at_ms, words_json, speaker_hints_json
    FROM transcripts AS transcript
    WHERE transcript.session_id = ? AND transcript.deleted_at IS NULL
    ORDER BY transcript.started_at_ms, transcript.created_at, transcript.id
";

// apps/desktop/src/stt/queries.ts, `useSessionParticipantHumanIds`.
const PARTICIPANT_HUMAN_IDS_SQL: &str = "
    SELECT DISTINCT participant.human_id
    FROM session_participants AS participant
    LEFT JOIN humans AS human
      ON human.id = participant.human_id
      AND human.deleted_at IS NULL
    WHERE participant.session_id = ?
      AND participant.human_id <> ''
      AND participant.source <> 'excluded'
      AND participant.deleted_at IS NULL
      AND participant.human_id <> COALESCE((
        SELECT session.owner_user_id
        FROM sessions AS session
        WHERE session.id = participant.session_id
      ), '')
      AND (
        NULLIF(lower(COALESCE(NULLIF(human.email, ''), participant.email)), '') IS NULL
        OR NOT EXISTS (
          SELECT 1
          FROM humans AS self_human
          JOIN sessions AS session
            ON session.owner_user_id = self_human.id
          WHERE session.id = participant.session_id
            AND self_human.deleted_at IS NULL
            AND NULLIF(lower(self_human.email), '') IS NOT NULL
            AND lower(self_human.email) = lower(COALESCE(NULLIF(human.email, ''), participant.email))
        )
      )
    ORDER BY participant.human_id
";

// apps/desktop/src/services/enhancer/storage.ts, `ensureSummaryDocument`.
const INSERT_SUMMARY_DOCUMENT_SQL: &str = "
    INSERT INTO session_documents (
      id, workspace_id, session_id, kind, template_id, title,
      body_format, body, sort_order, created_by, updated_by,
      created_at, updated_at, deleted_at
    )
    SELECT
      ?, workspace_id, id, ?, ?, 'Summary', 'prosemirror_json', '', ?,
      owner_user_id, owner_user_id, ?, ?, NULL
    FROM sessions
    WHERE id = ? AND deleted_at IS NULL
";

const ENHANCED_POSITIONS_SQL: &str = "
    SELECT template_id, sort_order
    FROM session_documents
    WHERE session_id = ?
      AND kind IN ('summary', 'template_output')
      AND deleted_at IS NULL
";

// `replaceSummaryDocumentTemplate`, run by `hydrateTemplateTitle`.
const HYDRATE_TEMPLATE_TITLE_SQL: &str = "
    UPDATE session_documents
    SET template_id = ?, title = ?, updated_at = ?
    WHERE id = ? AND session_id = ? AND deleted_at IS NULL
";

/// A `transcripts` row as `useSessionTranscripts` reads it.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct TranscriptRow {
    pub id: String,
    pub owner_user_id: String,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub words_json: String,
    pub speaker_hints_json: String,
}

/// `useTranscriptHumans`: names for the given ids, in id order.
async fn transcript_humans(
    pool: &sqlx::SqlitePool,
    human_ids: &[String],
) -> anyhow::Result<Vec<(String, String)>> {
    let mut ids: Vec<&String> = human_ids.iter().filter(|id| !id.is_empty()).collect();
    ids.sort();
    ids.dedup();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; ids.len()].join(", ");
    let sql = format!(
        "SELECT id, name FROM humans WHERE id IN ({placeholders}) AND name <> '' AND deleted_at IS NULL ORDER BY id"
    );
    let mut query = sqlx::query_as::<_, (String, String)>(sqlx::AssertSqlSafe(sql));
    for id in ids {
        query = query.bind(id.clone());
    }
    Ok(query.fetch_all(pool).await?)
}

#[derive(Debug, Clone, PartialEq)]
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
    /// Stored transcripts, segmented for the Transcript tab.
    pub transcripts: Vec<crate::transcript::RenderedTranscript>,
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
    /// The Tauri bundle identifier whose data (and credential-store entries)
    /// this shell shares.
    identifier: String,
}

const CHANGE_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(750);

impl Store {
    pub async fn open(
        runtime: tokio::runtime::Handle,
        path: PathBuf,
        identifier: String,
    ) -> anyhow::Result<Self> {
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
            identifier,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn runtime(&self) -> &tokio::runtime::Handle {
        &self.runtime
    }

    pub fn identifier(&self) -> &str {
        &self.identifier
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

    /// `setSettingValues`: one row per key, `JSON.stringify`'d, in
    /// `synced_preferences` for synced keys and `app_settings` otherwise.
    pub fn set_setting(
        &self,
        key: String,
        value: serde_json::Value,
        synced: bool,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let now = chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();
            let json = serde_json::to_string(&value)?;
            let sql = if synced {
                "INSERT INTO synced_preferences (id, workspace_id, value_json, updated_at)
                 VALUES (?, NULLIF((
                   SELECT json_extract(value_json, '$.workspace_id')
                   FROM app_settings
                   WHERE id = 'cloudsync_workspace_binding'
                 ), ''), ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                   workspace_id = excluded.workspace_id,
                   value_json = excluded.value_json,
                   updated_at = excluded.updated_at"
            } else {
                "INSERT INTO app_settings (id, value_json, updated_at)
                 VALUES (?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                   value_json = excluded.value_json,
                   updated_at = excluded.updated_at"
            };
            sqlx::query(sql)
                .bind(&key)
                .bind(&json)
                .bind(&now)
                .execute(db.pool())
                .await?;
            Ok(())
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
            let row =
                sqlx::query_as::<_, (String, String, String, String, i64, i64, i64, i64, i64)>(
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
            Ok(apply_tombstone(pool, &session_id, &tombstone, false).await? == 1)
        })
    }

    /// `softDeleteSession`: tombstones a live session and returns the
    /// tombstone the undo toast needs to restore it.
    pub fn soft_delete_session(
        &self,
        session_id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<Option<String>>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let tombstone = chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();
            let affected = apply_tombstone(db.pool(), &session_id, &tombstone, false).await?;
            Ok((affected == 1).then_some(tombstone))
        })
    }

    /// `restoreDeletedSession`: lifts exactly the rows that tombstone touched.
    pub fn restore_session(
        &self,
        session_id: String,
        tombstone: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            apply_tombstone(db.pool(), &session_id, &tombstone, true).await?;
            Ok(())
        })
    }

    /// `fs-sync`'s `session_dir`: `<vault>/sessions/<id>`, where the vault is
    /// the folder holding `app.db`.
    pub fn session_dir(&self, session_id: &str) -> PathBuf {
        let base = self
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        crate::workspace::find_session_dir(&base.join("sessions"), session_id)
    }

    /// `updateSession(sessionId, { raw_md })`: the memo upsert.
    /// `updateSession({ raw_md, raw_template_id })` after applying a template.
    pub fn update_memo_with_template(
        &self,
        session_id: String,
        body: String,
        template_id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let now = chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();
            sqlx::query(UPSERT_MEMO_WITH_TEMPLATE_SQL)
                .bind(&session_id)
                .bind(&template_id)
                .bind(&body)
                .bind(&now)
                .bind(&now)
                .bind(&session_id)
                .execute(db.pool())
                .await?;
            Ok(())
        })
    }

    /// `useCreateTemplate`'s insert for the memo's "New template" button:
    /// an untitled, unpinned template with the default icon and no sections.
    pub fn create_template(&self) -> tokio::task::JoinHandle<anyhow::Result<String>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO templates (id, title, description, pinned, category, icon_json, targets_json, sections_json, created_at, updated_at)
                 VALUES (?, 'New Template', '', 0, NULL, ?, NULL, '[]', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
            )
            .bind(&id)
            .bind(r##"{"type":"icon","value":"notebook-tabs","color":"#9ca3af"}"##)
            .execute(db.pool())
            .await?;
            Ok(id)
        })
    }

    /// `loadSecureAiProviderApiKeys`: the credential-store key for each
    /// provider id, or the store's error message.
    pub fn ai_provider_api_keys(
        &self,
        kind: &'static str,
        provider_ids: Vec<String>,
    ) -> tokio::task::JoinHandle<Vec<(String, ApiKeyResult)>> {
        let identifier = self.identifier.clone();
        self.runtime.spawn_blocking(move || {
            provider_ids
                .into_iter()
                .map(|provider_id| {
                    let key = format!("{kind}:{provider_id}");
                    let result = crate::secrets::read(
                        &identifier,
                        crate::secrets::PROVIDER_SECRET_SCOPE,
                        &key,
                    );
                    (provider_id, result)
                })
                .collect()
        })
    }

    /// `setAiProvider`: the API key goes to the credential store, the row keeps
    /// `{type, base_url, api_key: ""}`; a legacy document entry is redacted.
    pub fn set_ai_provider(
        &self,
        kind: &'static str,
        provider_id: String,
        base_url: Option<String>,
        api_key: Option<String>,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        let identifier = self.identifier.clone();
        self.runtime.spawn(async move {
            let pool = db.pool();
            let storage_id = format!("ai_provider:{kind}:{provider_id}");
            let secret_key = format!("{kind}:{provider_id}");
            let rows = sqlx::query_as::<_, (String, String)>(
                "SELECT id, value_json FROM app_settings WHERE id IN (?, ?)",
            )
            .bind(&storage_id)
            .bind("legacy_settings_document")
            .fetch_all(pool)
            .await?;
            let settings = ProviderSettings::from_rows(&rows);
            let current = settings.ai_providers(kind).remove(provider_id.as_str());
            let direct = rows.iter().find(|(id, _)| *id == storage_id).cloned();
            let previous_key = {
                let identifier = identifier.clone();
                let secret_key = secret_key.clone();
                tokio::task::spawn_blocking(move || {
                    crate::secrets::read(&identifier, crate::secrets::PROVIDER_SECRET_SCOPE, &secret_key)
                })
                .await?
                .map_err(anyhow::Error::msg)?
            };
            let next_base_url = base_url
                .or_else(|| current.as_ref().map(|c| c.base_url.clone()))
                .unwrap_or_default();
            let next_api_key = api_key
                .or_else(|| previous_key.clone())
                .or_else(|| current.as_ref().map(|c| c.api_key.clone()))
                .unwrap_or_default();
            {
                let identifier = identifier.clone();
                let secret_key = secret_key.clone();
                let value = next_api_key.clone();
                tokio::task::spawn_blocking(move || {
                    crate::secrets::write(&identifier, crate::secrets::PROVIDER_SECRET_SCOPE, &secret_key, &value)
                })
                .await?
                .map_err(anyhow::Error::msg)?;
            }
            // `JSON.stringify({ type, base_url, api_key: "" })`
            let persisted = format!(
                "{{\"type\":{},\"base_url\":{},\"api_key\":\"\"}}",
                serde_json::Value::String(kind.to_string()),
                serde_json::Value::String(next_base_url)
            );
            let now = chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();
            let updated = match &direct {
                Some((_, existing)) => {
                    sqlx::query("UPDATE app_settings SET value_json = ?, updated_at = ? WHERE id = ? AND value_json = ?")
                        .bind(&persisted)
                        .bind(&now)
                        .bind(&storage_id)
                        .bind(existing)
                        .execute(pool)
                        .await?
                        .rows_affected()
                }
                None => {
                    sqlx::query("INSERT INTO app_settings (id, value_json, updated_at) VALUES (?, ?, ?) ON CONFLICT(id) DO NOTHING")
                        .bind(&storage_id)
                        .bind(&persisted)
                        .bind(&now)
                        .execute(pool)
                        .await?
                        .rows_affected()
                }
            };
            if updated != 1 {
                anyhow::bail!("Provider {kind}:{provider_id} changed too frequently");
            }
            // `redactLegacyProviderApiKey`
            if let Some((_, legacy_json)) = rows.iter().find(|(id, _)| id == "legacy_settings_document")
                && let Ok(mut legacy) = serde_json::from_str::<serde_json::Value>(legacy_json)
                && let Some(entry) = legacy
                    .get_mut("ai")
                    .and_then(|ai| ai.get_mut(kind))
                    .and_then(|providers| providers.get_mut(&provider_id))
                    .and_then(serde_json::Value::as_object_mut)
                && entry.get("api_key").and_then(serde_json::Value::as_str).is_some_and(|key| !key.is_empty())
            {
                entry.insert("api_key".to_string(), serde_json::Value::String(String::new()));
                sqlx::query("UPDATE app_settings SET value_json = ?, updated_at = ? WHERE id = ?")
                    .bind(legacy.to_string())
                    .bind(&now)
                    .bind("legacy_settings_document")
                    .execute(pool)
                    .await?;
            }
            Ok(())
        })
    }

    /// `clearAiProvider`: delete the credential-store key, the row, and the
    /// legacy document entry.
    pub fn clear_ai_provider(
        &self,
        kind: &'static str,
        provider_id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        let identifier = self.identifier.clone();
        self.runtime.spawn(async move {
            let pool = db.pool();
            let storage_id = format!("ai_provider:{kind}:{provider_id}");
            let secret_key = format!("{kind}:{provider_id}");
            tokio::task::spawn_blocking(move || {
                crate::secrets::write(
                    &identifier,
                    crate::secrets::PROVIDER_SECRET_SCOPE,
                    &secret_key,
                    "",
                )
            })
            .await?
            .map_err(anyhow::Error::msg)?;
            let rows = sqlx::query_as::<_, (String, String)>(
                "SELECT id, value_json FROM app_settings WHERE id IN (?, ?)",
            )
            .bind(&storage_id)
            .bind("legacy_settings_document")
            .fetch_all(pool)
            .await?;
            if let Some((_, existing)) = rows.iter().find(|(id, _)| *id == storage_id) {
                sqlx::query("DELETE FROM app_settings WHERE id = ? AND value_json = ?")
                    .bind(&storage_id)
                    .bind(existing)
                    .execute(pool)
                    .await?;
            }
            // `removeLegacyProvider`
            if let Some((_, legacy_json)) =
                rows.iter().find(|(id, _)| id == "legacy_settings_document")
                && let Ok(mut legacy) = serde_json::from_str::<serde_json::Value>(legacy_json)
                && let Some(providers) = legacy
                    .get_mut("ai")
                    .and_then(|ai| ai.get_mut(kind))
                    .and_then(serde_json::Value::as_object_mut)
                && providers.remove(&provider_id).is_some()
            {
                let now = chrono::Utc::now()
                    .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                    .to_string();
                sqlx::query("UPDATE app_settings SET value_json = ?, updated_at = ? WHERE id = ?")
                    .bind(legacy.to_string())
                    .bind(&now)
                    .bind("legacy_settings_document")
                    .execute(pool)
                    .await?;
            }
            Ok(())
        })
    }

    /// `useSessionParticipants`: active and excluded mappings with the human's
    /// current name/email over the mapping's own.
    pub fn list_session_participants(
        &self,
        session_id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<Vec<SessionParticipant>>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            Ok(
                sqlx::query_as::<_, SessionParticipant>(SESSION_PARTICIPANTS_SQL)
                    .bind(&session_id)
                    .fetch_all(db.pool())
                    .await?,
            )
        })
    }

    /// `useHumans` (the columns the participant picker searches).
    pub fn list_humans(&self) -> tokio::task::JoinHandle<anyhow::Result<Vec<Human>>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            Ok(sqlx::query_as::<_, Human>(HUMANS_SQL)
                .fetch_all(db.pool())
                .await?)
        })
    }

    /// `createHuman` + `addSessionParticipant` for a typed name, or just the
    /// latter for an existing contact.
    pub fn add_session_participant(
        &self,
        session_id: String,
        human: ParticipantTarget,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let pool = db.pool();
            let now = chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();
            let human_id = match human {
                ParticipantTarget::Existing(id) => id,
                ParticipantTarget::New(name) => {
                    let owner: Option<String> =
                        sqlx::query_scalar("SELECT owner_user_id FROM sessions WHERE id = ?")
                            .bind(&session_id)
                            .fetch_optional(pool)
                            .await?;
                    let Some(owner) = owner else {
                        anyhow::bail!("session {session_id} does not exist");
                    };
                    let human_id = uuid::Uuid::new_v4().to_string();
                    sqlx::query(CREATE_HUMAN_SQL)
                        .bind(&human_id)
                        .bind(&owner)
                        .bind(&name)
                        .bind("")
                        .bind(&now)
                        .bind(&now)
                        .execute(pool)
                        .await?;
                    human_id
                }
            };
            let participant_id = uuid::Uuid::new_v4().to_string();
            let mut tx = pool.begin().await?;
            sqlx::query(REVIVE_EXCLUDED_PARTICIPANT_SQL)
                .bind("manual")
                .bind(&now)
                .bind(&session_id)
                .bind(&human_id)
                .bind("manual")
                .execute(&mut *tx)
                .await?;
            sqlx::query(INSERT_MANUAL_PARTICIPANT_SQL)
                .bind(&participant_id)
                .bind("manual")
                .bind(&now)
                .bind(&now)
                .bind(&human_id)
                .bind(&session_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            Ok(())
        })
    }

    /// `removeHumanSpeakerAssignments` + `removeSessionParticipant`: drop the
    /// human's speaker hints from the session's stored transcripts, then
    /// exclude (auto) or tombstone (manual) the mapping.
    pub fn remove_session_participant(
        &self,
        session_id: String,
        mapping_id: String,
        human_id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let pool = db.pool();
            let now = chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();
            let transcripts = sqlx::query_as::<_, (String, String, i64, i64)>(
                "SELECT transcript.id, transcript.speaker_hints_json, transcript.content_revision,
                    (SELECT COUNT(*) FROM transcript_live_deltas AS delta WHERE delta.transcript_id = transcript.id)
                 FROM transcripts AS transcript
                 WHERE transcript.session_id = ? AND transcript.deleted_at IS NULL
                 ORDER BY transcript.started_at_ms, transcript.created_at, transcript.id",
            )
            .bind(&session_id)
            .fetch_all(pool)
            .await?;
            for (transcript_id, hints_json, revision, pending_deltas) in transcripts {
                if pending_deltas > 0 {
                    // Live deltas need the full snapshot materialisation the
                    // recording flow owns; stored transcripts have none.
                    tracing::warn!(%transcript_id, "skipping speaker hint removal with pending live deltas");
                    continue;
                }
                let Ok(serde_json::Value::Array(hints)) = serde_json::from_str::<serde_json::Value>(&hints_json)
                else {
                    continue;
                };
                let filtered: Vec<serde_json::Value> = hints
                    .iter()
                    .filter(|hint| {
                        let kind = hint.get("type").and_then(serde_json::Value::as_str).unwrap_or("");
                        if kind != "automatic_speaker_assignment" && kind != "user_speaker_assignment" {
                            return true;
                        }
                        assigned_human_id(hint.get("value")).as_deref() != Some(human_id.as_str())
                    })
                    .cloned()
                    .collect();
                if filtered.len() == hints.len() {
                    continue;
                }
                sqlx::query(
                    "UPDATE transcripts
                     SET speaker_hints_json = ?, content_revision = content_revision + 1, updated_at = ?
                     WHERE id = ? AND content_revision = ? AND deleted_at IS NULL",
                )
                .bind(serde_json::Value::Array(filtered).to_string())
                .bind(&now)
                .bind(&transcript_id)
                .bind(revision)
                .execute(pool)
                .await?;
            }
            sqlx::query(REMOVE_PARTICIPANT_SQL)
                .bind(&now)
                .bind(&now)
                .bind(&mapping_id)
                .execute(pool)
                .await?;
            Ok(())
        })
    }

    /// `updateSession({ created_at })` from the metadata date editor.
    pub fn update_created_at(
        &self,
        session_id: String,
        created_at: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let now = chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();
            sqlx::query("UPDATE sessions SET created_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL")
                .bind(&created_at)
                .bind(&now)
                .bind(&session_id)
                .execute(db.pool())
                .await?;
            Ok(())
        })
    }

    /// The vault mirror for folder operations (`fs-sync`'s base directory is
    /// the folder holding `app.db`).
    fn vault(&self) -> crate::folders::Vault {
        crate::folders::Vault::new(
            self.path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
        )
    }

    pub fn load_folder_catalog(
        &self,
    ) -> tokio::task::JoinHandle<anyhow::Result<crate::folders::Catalog>> {
        let db = self.db.clone();
        self.runtime
            .spawn(async move { crate::folders::load_catalog(db.pool()).await })
    }

    pub fn load_folder_details(
        &self,
        path: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<(String, Vec<crate::folders::Material>)>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            Ok((
                crate::folders::load_instructions(db.pool(), &path).await?,
                crate::folders::load_materials(db.pool(), &path).await?,
            ))
        })
    }

    pub fn create_folder(&self, path: String) -> tokio::task::JoinHandle<anyhow::Result<String>> {
        let db = self.db.clone();
        let vault = self.vault();
        self.runtime
            .spawn(async move { crate::folders::create_folder(db.pool(), &vault, &path).await })
    }

    pub fn rename_folder(
        &self,
        old_path: String,
        new_path: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<String>> {
        let db = self.db.clone();
        let vault = self.vault();
        self.runtime.spawn(async move {
            crate::folders::rename_folder(db.pool(), &vault, &old_path, &new_path).await
        })
    }

    pub fn delete_folder(&self, path: String) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        let vault = self.vault();
        self.runtime
            .spawn(async move { crate::folders::delete_folder(db.pool(), &vault, &path).await })
    }

    pub fn update_folder_instructions(
        &self,
        path: String,
        instructions: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            crate::folders::update_instructions(db.pool(), &path, &instructions).await
        })
    }

    /// `useActivity` for the signed-out shell (`DEFAULT_USER_ID`).
    pub fn load_activity(
        &self,
    ) -> tokio::task::JoinHandle<anyhow::Result<Vec<crate::stats::ActivityRecord>>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            Ok(
                sqlx::query_as::<_, crate::stats::ActivityRecord>(crate::stats::ACTIVITY_SQL)
                    .bind(DEFAULT_USER_ID)
                    .fetch_all(db.pool())
                    .await?,
            )
        })
    }

    pub fn update_folder_icon(
        &self,
        path: String,
        icon: TemplateIcon,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime
            .spawn(async move { crate::folders::update_icon(db.pool(), &path, &icon).await })
    }

    pub fn add_folder_material(
        &self,
        path: String,
        file: PathBuf,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        let vault = self.vault();
        self.runtime.spawn(async move {
            crate::folders::add_material(db.pool(), &vault, &path, &file).await
        })
    }

    pub fn remove_folder_material(
        &self,
        path: String,
        attachment_id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        let vault = self.vault();
        self.runtime.spawn(async move {
            crate::folders::remove_material(db.pool(), &vault, &path, &attachment_id).await
        })
    }

    pub fn list_user_templates(
        &self,
    ) -> tokio::task::JoinHandle<anyhow::Result<Vec<crate::templates::UserTemplate>>> {
        let db = self.db.clone();
        self.runtime
            .spawn(async move { crate::templates::list(db.pool()).await })
    }

    pub fn create_user_template(
        &self,
        draft: crate::templates::Draft,
    ) -> tokio::task::JoinHandle<anyhow::Result<String>> {
        let db = self.db.clone();
        self.runtime
            .spawn(async move { crate::templates::create(db.pool(), &draft).await })
    }

    pub fn save_user_template(
        &self,
        template: crate::templates::UserTemplate,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime
            .spawn(async move { crate::templates::save(db.pool(), &template).await })
    }

    pub fn delete_user_template(&self, id: String) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime
            .spawn(async move { crate::templates::delete(db.pool(), &id).await })
    }

    pub fn toggle_template_favorite(
        &self,
        id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime
            .spawn(async move { crate::templates::toggle_favorite(db.pool(), &id).await })
    }

    pub fn list_contacts(
        &self,
    ) -> tokio::task::JoinHandle<
        anyhow::Result<(
            Vec<crate::contacts::Human>,
            Vec<crate::contacts::Organization>,
        )>,
    > {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            Ok((
                crate::contacts::list_humans(db.pool()).await?,
                crate::contacts::list_organizations(db.pool()).await?,
            ))
        })
    }

    /// `useHumanSessions`
    pub fn human_sessions(
        &self,
        human_id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<Vec<crate::contacts::HumanSession>>> {
        let db = self.db.clone();
        self.runtime
            .spawn(async move { crate::contacts::human_sessions(db.pool(), &human_id).await })
    }

    /// `updateHuman` for one column.
    pub fn update_human_field(
        &self,
        human_id: String,
        column: &'static str,
        value: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            crate::contacts::update_human_field(db.pool(), &human_id, column, &value).await
        })
    }

    /// `toggleContactPin`
    pub fn toggle_contact_pin(
        &self,
        table: &'static str,
        id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime
            .spawn(async move { crate::contacts::toggle_pin(db.pool(), table, &id).await })
    }

    /// `deleteHuman` / `deleteOrganization`
    pub fn delete_contact(
        &self,
        table: &'static str,
        id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime
            .spawn(async move { crate::contacts::soft_delete(db.pool(), table, &id).await })
    }

    /// `createHuman({ name })` from the contacts sidebar (default owner).
    pub fn create_contact_human(
        &self,
        name: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<String>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let human_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();
            sqlx::query(CREATE_HUMAN_SQL)
                .bind(&human_id)
                .bind("00000000-0000-0000-0000-000000000000")
                .bind(&name)
                .bind("")
                .bind(&now)
                .bind(&now)
                .execute(db.pool())
                .await?;
            Ok(human_id)
        })
    }

    /// `createOrganization({ name })`
    pub fn create_organization(
        &self,
        name: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<String>> {
        let db = self.db.clone();
        self.runtime
            .spawn(async move { crate::contacts::create_organization(db.pool(), &name).await })
    }

    /// `listWebhooks`
    pub fn list_webhooks(
        &self,
    ) -> tokio::task::JoinHandle<anyhow::Result<Vec<crate::developers::Webhook>>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            Ok(anlg_db_app::list_webhook_endpoints(db.pool())
                .await?
                .into_iter()
                .map(crate::developers::Webhook::from)
                .collect())
        })
    }

    /// `createWebhook(url, [])`: the endpoint and its one-time secret.
    pub fn create_webhook(
        &self,
        url: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<(crate::developers::Webhook, String)>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            crate::developers::create_webhook(db.pool(), &url)
                .await
                .map_err(anyhow::Error::msg)
        })
    }

    /// `deleteWebhook`
    pub fn delete_webhook(&self, id: String) -> tokio::task::JoinHandle<anyhow::Result<bool>> {
        let db = self.db.clone();
        self.runtime
            .spawn(async move { Ok(anlg_db_app::delete_webhook_endpoint(db.pool(), &id).await?) })
    }

    /// `setWebhookActive`
    pub fn set_webhook_active(
        &self,
        id: String,
        active: bool,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            anlg_db_app::set_webhook_endpoint_active(db.pool(), &id, active).await?;
            Ok(())
        })
    }

    /// `testWebhook`: `(delivered, status)`.
    pub fn test_webhook(
        &self,
        id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<(bool, String)>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let endpoint = anlg_db_app::get_webhook_endpoint(db.pool(), &id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("webhook not found"))?;
            crate::developers::send_test_webhook(db.pool(), &endpoint)
                .await
                .map_err(anyhow::Error::msg)
        })
    }

    pub fn list_templates(&self) -> tokio::task::JoinHandle<anyhow::Result<Vec<Template>>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let rows = sqlx::query_as::<_, (String, String, i64, Option<i64>, String, String)>(
                TEMPLATES_SQL,
            )
            .fetch_all(db.pool())
            .await?;
            Ok(rows.into_iter().map(Template::from_row).collect())
        })
    }

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

    /// `updateSession(sessionId, { folder_id })`: `folder_path = ?`.
    pub fn update_folder(
        &self,
        session_id: String,
        folder_path: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let now = chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();
            sqlx::query(
                "UPDATE sessions SET folder_path = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL",
            )
            .bind(&folder_path)
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

    /// `useEnsureDefaultSummary` -> `enhancer.ensureNote(sessionId, templateId)`:
    /// a session with a transcript and no enhanced note gets a `Summary`
    /// document (kind `summary`, or `template_output` when a template is
    /// selected, whose title is then hydrated from the template). Returns
    /// whether a row was inserted.
    pub fn ensure_summary_document(
        &self,
        session_id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<bool>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let pool = db.pool();
            if anlg_db_app::get_session(pool, &session_id).await?.is_none() {
                return Ok(false);
            }
            // `templateId = memoTemplateId || selectedTemplateId`, where the memo
            // template is `COALESCE(note.template_id, '')` of the `note` document.
            let memo_template: Option<String> = sqlx::query_scalar(
                "SELECT template_id FROM session_documents WHERE session_id = ? AND kind = 'note' AND deleted_at IS NULL LIMIT 1",
            )
            .bind(&session_id)
            .fetch_optional(pool)
            .await?;
            let rows = sqlx::query_as::<_, (String, String, i64)>(SETTING_ROWS_SQL)
                .fetch_all(pool)
                .await?
                .into_iter()
                .map(|(id, json, _)| (id, json))
                .collect::<Vec<_>>();
            let settings = ProviderSettings::from_rows(&rows);
            let template_id = memo_template
                .filter(|id| !id.is_empty())
                .or_else(|| {
                    settings.string_setting("selected_template_id", &["general", "selected_template_id"])
                })
                .unwrap_or_default();
            let existing = sqlx::query_as::<_, (String, i64)>(ENHANCED_POSITIONS_SQL)
                .bind(&session_id)
                .fetch_all(pool)
                .await?;
            if existing.iter().any(|(template, _)| *template == template_id) {
                return Ok(false);
            }
            let position = existing.iter().map(|(_, order)| *order).max().unwrap_or(0) + 1;
            let now = chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();
            let note_id = uuid::Uuid::new_v4().to_string();
            let inserted = sqlx::query(INSERT_SUMMARY_DOCUMENT_SQL)
                .bind(&note_id)
                .bind(if template_id.is_empty() { "summary" } else { "template_output" })
                .bind(&template_id)
                .bind(position)
                .bind(&now)
                .bind(&now)
                .bind(&session_id)
                .execute(pool)
                .await?
                .rows_affected();
            if inserted == 1 && !template_id.is_empty() {
                let title: Option<String> =
                    sqlx::query_scalar("SELECT title FROM templates WHERE id = ?")
                        .bind(&template_id)
                        .fetch_optional(pool)
                        .await?;
                if let Some(title) = title.map(|title| title.trim().to_string()).filter(|title| !title.is_empty()) {
                    sqlx::query(HYDRATE_TEMPLATE_TITLE_SQL)
                        .bind(&template_id)
                        .bind(&title)
                        .bind(&now)
                        .bind(&note_id)
                        .bind(&session_id)
                        .execute(pool)
                        .await?;
                }
            }
            Ok(inserted == 1)
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
                sqlx::query_as::<_, (String, String, String, String, String)>(ENHANCED_NOTES_SQL)
                    .bind(&session_id)
                    .fetch_all(db.pool())
                    .await?
                    .into_iter()
                    .map(|(id, title, body, body_format, template_id)| NoteDocument {
                        id,
                        title,
                        blocks: document::from_body(&body_format, &body),
                        body,
                        template_id,
                    })
                    .collect();
            let has_transcript: bool = sqlx::query_scalar(HAS_TRANSCRIPT_SQL)
                .bind(&session_id)
                .fetch_one(db.pool())
                .await?;
            let transcripts = if has_transcript {
                let rows = sqlx::query_as::<_, TranscriptRow>(SESSION_TRANSCRIPTS_SQL)
                    .bind(&session_id)
                    .fetch_all(db.pool())
                    .await?;
                let participants: Vec<String> = sqlx::query_scalar(PARTICIPANT_HUMAN_IDS_SQL)
                    .bind(&session_id)
                    .fetch_all(db.pool())
                    .await?;
                // `humanIds = participants ∪ assigned ∪ self`.
                let mut human_ids = participants.clone();
                human_ids.extend(crate::transcript::assigned_human_ids(&rows));
                if let Some(owner) = rows.first().map(|row| row.owner_user_id.clone()) {
                    human_ids.push(owner);
                }
                let humans = transcript_humans(db.pool(), &human_ids).await?;
                crate::transcript::render_transcripts(&rows, &participants, &humans)
            } else {
                Vec::new()
            };
            Ok(Some(NotePreview {
                has_transcript,
                transcripts,
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

        let store = Store::open(
            tokio::runtime::Handle::current(),
            path,
            "com.hyprnote.dev".to_string(),
        )
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
        let store = Store::open(
            tokio::runtime::Handle::current(),
            path,
            "com.hyprnote.dev".to_string(),
        )
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
        assert_eq!(none.theme, "system");
        let dark = ProviderSettings::from_rows(&rows(&[("theme", "\"dark\"")]));
        assert_eq!(dark.theme, "dark");
        let bogus = ProviderSettings::from_rows(&rows(&[("theme", "\"neon\"")]));
        assert_eq!(bogus.theme, "system");

        let general = ProviderSettings::from_rows(&rows(&[
            ("autostart", "true"),
            (
                "legacy_settings_document",
                r#"{"general":{"show_tray_icon":false,"ai_language":"ko","timezone":""}}"#,
            ),
        ]));
        assert!(general.bool_setting("autostart", &["general", "autostart"], false));
        assert!(general.bool_setting("automatic_updates", &["general", "automatic_updates"], true));
        assert!(
            !general.bool_setting("show_tray_icon", &["general", "show_tray_icon"], true),
            "legacy document fallback"
        );
        assert_eq!(
            general
                .string_setting("ai_language", &["general", "ai_language"])
                .as_deref(),
            Some("ko")
        );
        assert_eq!(
            general.string_setting("timezone", &["general", "timezone"]),
            None,
            "blank counts as unset"
        );

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
        let store = Store::open(
            tokio::runtime::Handle::current(),
            path,
            "com.hyprnote.dev".to_string(),
        )
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
        let store = Store::open(
            tokio::runtime::Handle::current(),
            path,
            "com.hyprnote.dev".to_string(),
        )
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

        let store = Store::open(
            tokio::runtime::Handle::current(),
            path,
            "com.hyprnote.dev".to_string(),
        )
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
        let error = Store::open(
            tokio::runtime::Handle::current(),
            path.clone(),
            "com.hyprnote.dev".to_string(),
        )
        .await
        .err()
        .unwrap();
        assert!(error.to_string().contains("--db-path"));
        assert!(!path.exists());
    }
}
