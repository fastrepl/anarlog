//! The enhancer's reads and writes: `session/content-queries.ts`,
//! `services/enhancer/storage.ts`, and `session/content-mutations.ts`, with
//! the frontend's own statements.

use sqlx::SqlitePool;

use super::{Store, TranscriptRow};
use crate::enhancer::prompts::{PromptSettings, TemplateRecord};
use crate::enhancer::{
    EnhancedNote, PENDING_AUTO_ENHANCE_SETTING_PREFIX, PendingJob, SegmentPayload, Snapshot,
    SnapshotParticipant, SnapshotTranscript,
};

fn now() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

/// `SESSION_CONTENT_SQL`'s scalar columns; the JSON aggregates are separate
/// queries here.
const SESSION_CONTENT_SQL: &str = "
  SELECT
    session.id,
    session.owner_user_id,
    session.title,
    session.event_json,
    session.source_apps_json,
    session.created_at,
    COALESCE(session.event_id, '') AS event_id,
    COALESCE(note.id, '') AS raw_note_id,
    COALESCE(note.template_id, '') AS raw_template_id,
    COALESCE(note.body, '') AS raw_body,
    COALESCE(note.body_format, 'prosemirror_json') AS raw_body_format
  FROM sessions AS session
  LEFT JOIN session_documents AS note
    ON note.id = COALESCE(
      (
        SELECT canonical.id
        FROM session_documents AS canonical
        WHERE canonical.id = session.id
          AND canonical.session_id = session.id
          AND canonical.kind = 'note'
          AND canonical.deleted_at IS NULL
        LIMIT 1
      ),
      (
        SELECT fallback.id
        FROM session_documents AS fallback
        WHERE fallback.session_id = session.id
          AND fallback.kind = 'note'
          AND fallback.deleted_at IS NULL
        ORDER BY fallback.created_at, fallback.id
        LIMIT 1
      )
    )
  WHERE session.id = ? AND session.deleted_at IS NULL
  LIMIT 1
";

const ENHANCED_NOTES_SQL: &str = "
  SELECT id, title, body, body_format, COALESCE(template_id, ''), sort_order
  FROM session_documents
  WHERE session_id = ?
    AND kind IN ('summary', 'template_output')
    AND deleted_at IS NULL
  ORDER BY sort_order, id
";

const TRANSCRIPT_MEMOS_SQL: &str = "
  SELECT id, memo
  FROM transcripts
  WHERE session_id = ? AND deleted_at IS NULL
";

/// `participants_json` of `SESSION_CONTENT_SQL`.
const PARTICIPANTS_SQL: &str = "
  SELECT
    participant.human_id,
    COALESCE(NULLIF(human.name, ''), participant.display_name) AS name,
    COALESCE(human.job_title, '') AS job_title
  FROM session_participants AS participant
  LEFT JOIN humans AS human
    ON human.id = participant.human_id
    AND human.deleted_at IS NULL
  JOIN sessions AS session ON session.id = participant.session_id
  WHERE participant.session_id = ?
    AND participant.human_id <> ''
    AND participant.source <> 'excluded'
    AND participant.deleted_at IS NULL
    AND (
      participant.human_id = session.owner_user_id
      OR NULLIF(lower(COALESCE(NULLIF(human.email, ''), participant.email)), '') IS NULL
      OR NOT EXISTS (
        SELECT 1
        FROM humans AS self_human
        WHERE self_human.id = session.owner_user_id
          AND self_human.deleted_at IS NULL
          AND NULLIF(lower(self_human.email), '') IS NOT NULL
          AND lower(self_human.email) = lower(COALESCE(NULLIF(human.email, ''), participant.email))
      )
    )
";

/// `MEETING_CHAT_RECORDS_SQL`.
const MEETING_CHAT_SQL: &str = "
  SELECT body
  FROM (
    SELECT id, body, created_at, sort_order
    FROM session_documents
    WHERE session_id = ?
      AND kind = 'meeting_chat'
      AND deleted_at IS NULL
      AND length(CAST(body AS BLOB)) <= 16384
    ORDER BY sort_order DESC, created_at DESC, id DESC
    LIMIT 1000
  )
  ORDER BY sort_order, created_at, id
";

/// `loadPendingAutoEnhanceJobs`.
const PENDING_JOBS_SQL: &str = "
  SELECT
    substr(setting.id, ?) AS session_id,
    document.id AS note_id,
    COALESCE(document.template_id, '') AS template_id,
    document.body AS expected_body,
    document.body_format AS expected_content_format,
    json_extract(setting.value_json, '$.generation') AS generation
  FROM app_settings AS setting
  JOIN sessions AS session
    ON session.id = substr(setting.id, ?)
    AND session.deleted_at IS NULL
  JOIN session_documents AS document
    ON document.id = json_extract(setting.value_json, '$.noteId')
    AND document.session_id = session.id
    AND document.body = json_extract(setting.value_json, '$.body')
  WHERE setting.id LIKE ?
    AND json_valid(setting.value_json)
    AND json_type(setting.value_json, '$.noteId') = 'text'
    AND json_type(setting.value_json, '$.body') = 'text'
    AND json_type(setting.value_json, '$.generation') = 'text'
    AND json_extract(setting.value_json, '$.bodyFormat') = document.body_format
    AND document.kind IN ('summary', 'template_output')
    AND document.deleted_at IS NULL
    AND EXISTS (
      SELECT 1
      FROM transcripts AS transcript
      WHERE transcript.session_id = session.id
        AND transcript.deleted_at IS NULL
        AND json_valid(transcript.words_json)
        AND json_type(transcript.words_json) = 'array'
        AND json_array_length(transcript.words_json) > 0
    )
  ORDER BY setting.updated_at, session_id
";

const DISCARD_PENDING_SQL: &str = "
  DELETE FROM app_settings
  WHERE id = ?
    AND json_valid(value_json)
    AND json_extract(value_json, '$.noteId') = ?
    AND json_extract(value_json, '$.generation') = ?
    AND json_extract(value_json, '$.body') = ?
    AND json_extract(value_json, '$.bodyFormat') = ?
";

const UPSERT_PENDING_SQL: &str = "
  INSERT INTO app_settings (id, value_json, updated_at)
  VALUES (?, ?, ?)
  ON CONFLICT(id) DO UPDATE SET
    value_json = excluded.value_json,
    updated_at = excluded.updated_at
";

const INSERT_SUMMARY_SQL: &str = "
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

const REPLACE_TEMPLATE_SQL: &str = "
  UPDATE session_documents
  SET
    kind = ?,
    template_id = ?,
    title = ?,
    body_format = 'prosemirror_json',
    body = '',
    updated_by = COALESCE((
      SELECT owner_user_id FROM sessions
      WHERE sessions.id = ? AND sessions.deleted_at IS NULL
    ), updated_by),
    updated_at = ?
  WHERE id = ?
    AND session_id = ?
    AND kind IN ('summary', 'template_output')
    AND deleted_at IS NULL
    AND EXISTS (
      SELECT 1 FROM sessions
      WHERE sessions.id = ? AND sessions.deleted_at IS NULL
    )
";

const UPDATE_TITLE_IF_CURRENT_SQL: &str = "
  UPDATE session_documents
  SET title = ?, updated_at = ?
  WHERE id = ?
    AND session_id = ?
    AND kind IN ('summary', 'template_output')
    AND template_id = ?
    AND title = ?
    AND deleted_at IS NULL
";

const PERSIST_NOTE_SQL: &str = "
  UPDATE session_documents
  SET body = ?, body_format = 'prosemirror_json', updated_at = ?
  WHERE id = ?
    AND session_id = ?
    AND kind IN ('summary', 'template_output')
    AND body = ?
    AND body_format = ?
    AND deleted_at IS NULL
    AND EXISTS (
      SELECT 1 FROM sessions
      WHERE sessions.id = ? AND sessions.deleted_at IS NULL
    )
";

const PERSIST_NOTE_PENDING_SQL: &str = "
  UPDATE session_documents
  SET body = ?, body_format = 'prosemirror_json', updated_at = ?
  WHERE id = ?
    AND session_id = ?
    AND kind IN ('summary', 'template_output')
    AND body = ?
    AND body_format = ?
    AND deleted_at IS NULL
    AND EXISTS (
      SELECT 1 FROM sessions
      WHERE sessions.id = ? AND sessions.deleted_at IS NULL
    )
    AND EXISTS (
      SELECT 1
      FROM app_settings AS pending
      WHERE pending.id = ?
        AND json_valid(pending.value_json)
        AND json_extract(pending.value_json, '$.noteId') = ?
        AND json_extract(pending.value_json, '$.generation') = ?
        AND json_extract(pending.value_json, '$.body') = ?
        AND json_extract(pending.value_json, '$.bodyFormat') = ?
        AND session_documents.body = json_extract(pending.value_json, '$.body')
        AND session_documents.body_format = json_extract(pending.value_json, '$.bodyFormat')
    )
";

const CLEAR_PENDING_BY_BODY_SQL: &str = "
  DELETE FROM app_settings
  WHERE id = ?
    AND json_valid(value_json)
    AND json_extract(value_json, '$.noteId') = ?
    AND json_extract(value_json, '$.body') = ?
";

/// `updateEnhancedNoteContent`.
const UPDATE_ENHANCED_CONTENT_SQL: &str = "
  UPDATE session_documents
  SET body = ?, body_format = 'prosemirror_json', updated_at = ?
  WHERE id = ?
    AND kind IN ('summary', 'template_output')
    AND deleted_at IS NULL
";

const CLEAR_PENDING_UNLESS_BODY_SQL: &str = "
  DELETE FROM app_settings
  WHERE id = ?
    AND json_valid(value_json)
    AND json_extract(value_json, '$.noteId') = ?
    AND json_extract(value_json, '$.body') <> ?
";

const UPDATE_SESSION_TITLE_UNGUARDED_SQL: &str = "
  UPDATE sessions
  SET title = ?, updated_at = ?
  WHERE id = ? AND deleted_at IS NULL
";

const UPSERT_TAG_SQL: &str = "
  INSERT INTO tags (
    id, owner_user_id, name, created_at, updated_at, deleted_at
  ) VALUES (?, ?, ?, ?, ?, NULL)
  ON CONFLICT(id) DO UPDATE SET
    owner_user_id = excluded.owner_user_id,
    name = excluded.name,
    updated_at = excluded.updated_at,
    deleted_at = NULL
";

const UPSERT_SESSION_TAG_SQL: &str = "
  INSERT INTO session_tags (
    id, owner_user_id, session_id, tag_id,
    created_at, updated_at, deleted_at
  ) VALUES (?, ?, ?, ?, ?, ?, NULL)
  ON CONFLICT(id) DO UPDATE SET
    owner_user_id = excluded.owner_user_id,
    session_id = excluded.session_id,
    tag_id = excluded.tag_id,
    updated_at = excluded.updated_at,
    deleted_at = NULL
";

const UPDATE_SESSION_TITLE_SQL: &str = "
  UPDATE sessions
  SET title = ?, updated_at = ?
  WHERE id = ? AND title = ? AND deleted_at IS NULL
";

const UPDATE_DOCUMENT_BODY_SQL: &str = "
  UPDATE session_documents
  SET body = ?, body_format = 'prosemirror_json', updated_at = ?
  WHERE id = ?
    AND session_id = ?
    AND kind IN ('note', 'summary', 'template_output')
    AND body = ?
    AND body_format = ?
    AND deleted_at IS NULL
";

/// `SessionDocumentContentUpdate`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentUpdate {
    pub id: String,
    pub current_content: String,
    pub current_content_format: String,
    pub next_content: String,
}

/// `bodyToMarkdown` → `json2md`: ProseMirror's `MarkdownSerializer` with its
/// default `tightLists: false`, so list items are separated by blank lines,
/// and no trailing newline.
pub fn body_to_markdown(body: &str, format: &str) -> String {
    if body.is_empty() || format == "markdown" {
        return body.to_string();
    }
    let Ok(json) = serde_json::from_str::<serde_json::Value>(body) else {
        return body.to_string();
    };
    let Some(blocks) = json.get("content").and_then(|content| content.as_array()) else {
        return body.to_string();
    };
    // prosemirror-markdown's `MarkdownSerializerState`: a block's closing
    // `\n\n` is flushed by the next write, so the document never ends with
    // one, and an empty paragraph (a write of nothing) leaves the one newline
    // the flush adds when the output already ends at a blank.
    let mut out = String::new();
    let mut closed = false;
    for block in blocks {
        let rendered = serde_json::from_value::<serde_json::Value>(serde_json::json!({
            "type": "doc",
            "content": [block]
        }))
        .ok()
        .and_then(|doc| {
            let mut ast = anlg_tiptap::tiptap_json_to_mdast(&doc);
            spread_lists(&mut ast);
            anlg_tiptap::mdast_to_markdown(&ast).ok()
        })
        .map(|markdown| markdown.trim_end_matches('\n').to_string())
        .unwrap_or_default();
        if closed {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
        }
        out.push_str(&rendered);
        closed = true;
    }
    out
}

fn spread_lists(node: &mut markdown::mdast::Node) {
    if let markdown::mdast::Node::List(list) = node {
        list.spread = true;
    }
    if let Some(children) = node.children_mut() {
        for child in children {
            spread_lists(child);
        }
    }
}

async fn enhanced_notes(pool: &SqlitePool, session_id: &str) -> anyhow::Result<Vec<EnhancedNote>> {
    let rows =
        sqlx::query_as::<_, (String, String, String, String, String, i64)>(ENHANCED_NOTES_SQL)
            .bind(session_id)
            .fetch_all(pool)
            .await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, title, body, body_format, template_id, position)| EnhancedNote {
                id,
                title,
                content: body,
                content_format: body_format,
                template_id,
                position,
            },
        )
        .collect())
}

/// `SESSION_CONTENT_SQL`'s scalar columns.
type SessionContentRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
);

pub(crate) async fn load_snapshot(
    pool: &SqlitePool,
    session_id: &str,
) -> anyhow::Result<Option<Snapshot>> {
    let Some((
        id,
        owner_user_id,
        title,
        event_json,
        source_apps_json,
        created_at,
        event_id,
        raw_note_id,
        raw_template_id,
        raw_body,
        raw_body_format,
    )) = sqlx::query_as::<_, SessionContentRow>(SESSION_CONTENT_SQL)
        .bind(session_id)
        .fetch_optional(pool)
        .await?
    else {
        return Ok(None);
    };
    let enhanced_notes = enhanced_notes(pool, &id).await?;

    let rows: Vec<TranscriptRow> =
        sqlx::query_as::<_, TranscriptRow>(super::SESSION_TRANSCRIPTS_SQL)
            .bind(&id)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(TranscriptRow::materialize)
            .collect();
    let memos: std::collections::HashMap<String, String> =
        sqlx::query_as::<_, (String, String)>(TRANSCRIPT_MEMOS_SQL)
            .bind(&id)
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect();
    let mut transcripts: Vec<SnapshotTranscript> = rows
        .iter()
        .map(|row| SnapshotTranscript {
            id: row.id.clone(),
            started_at: row.started_at_ms,
            ended_at: row.ended_at_ms,
            memo: memos.get(&row.id).cloned().unwrap_or_default(),
            words: serde_json::from_str::<Vec<serde_json::Value>>(&row.words_json)
                .unwrap_or_default()
                .iter()
                .map(|word| {
                    word.get("text")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string()
                })
                .collect(),
        })
        .collect();
    transcripts.sort_by(|left, right| {
        left.started_at
            .cmp(&right.started_at)
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut participants: Vec<SnapshotParticipant> =
        sqlx::query_as::<_, (String, String, String)>(PARTICIPANTS_SQL)
            .bind(&id)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|(human_id, name, job_title)| SnapshotParticipant {
                human_id,
                name,
                job_title,
            })
            .collect();
    participants.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.human_id.cmp(&right.human_id))
    });

    // `getTranscriptSegments`: the same render the transcript tab uses, with
    // `humanIds = owner ∪ participants ∪ assigned`.
    let segments = if rows.is_empty() {
        Vec::new()
    } else {
        let participant_ids: Vec<String> = participants
            .iter()
            .map(|participant| participant.human_id.clone())
            .collect();
        let mut human_ids = vec![owner_user_id.clone()];
        human_ids.extend(participant_ids.iter().cloned());
        human_ids.extend(crate::transcript::assigned_human_ids(&rows));
        let humans = super::transcript_humans(pool, &human_ids).await?;
        let mut segments: Vec<SegmentPayload> =
            crate::transcript::render_transcripts(&rows, &participant_ids, &humans)
                .into_iter()
                .flat_map(|transcript| transcript.segments)
                .filter(|segment| !segment.words.is_empty())
                .map(|segment| SegmentPayload {
                    speaker_label: segment.export_speaker.clone(),
                    start_ms: segment.start_ms,
                    end_ms: segment.end_ms,
                    text: segment.export_text.clone(),
                })
                .collect();
        segments.sort_by_key(|segment| segment.start_ms);
        segments
    };

    let chat_bodies: Vec<String> = sqlx::query_scalar(MEETING_CHAT_SQL)
        .bind(&id)
        .fetch_all(pool)
        .await?;
    let meeting_chat = crate::enhancer::meeting_chat_markdown(&chat_bodies);
    let supplemental_context = [
        crate::enhancer::source_apps_context(&source_apps_json),
        crate::enhancer::meeting_chat_context(&chat_bodies),
    ]
    .into_iter()
    .filter(|value| !value.trim().is_empty())
    .collect::<Vec<_>>()
    .join("\n\n");

    Ok(Some(Snapshot {
        session_id: id,
        owner_user_id,
        title,
        created_at,
        event_id,
        event_json,
        meeting_chat,
        raw_note_id: Some(raw_note_id).filter(|id| !id.is_empty()),
        raw_template_id,
        raw_markdown: body_to_markdown(&raw_body, &raw_body_format),
        raw_content: raw_body,
        raw_content_format: raw_body_format,
        enhanced_notes,
        transcripts,
        participants,
        segments,
        supplemental_context,
    }))
}

fn pending_setting_id(session_id: &str) -> String {
    format!("{PENDING_AUTO_ENHANCE_SETTING_PREFIX}{session_id}")
}

fn pending_value(note_id: &str, body: &str, body_format: &str, generation: &str) -> String {
    serde_json::json!({
        "noteId": note_id,
        "body": body,
        "bodyFormat": body_format,
        "generation": generation,
    })
    .to_string()
}

impl Store {
    /// `useLLMConnection` → `createLanguageModel`: the selected provider and
    /// model with its base URL, credential-store key, and reasoning effort;
    /// `None` when the selection is incomplete or a required config value is
    /// missing (`getProviderSelectionBlockers`).
    pub fn llm_connection(
        &self,
        settings: &super::ProviderSettings,
    ) -> tokio::task::JoinHandle<Option<crate::llm_stream::Connection>> {
        let provider = settings.llm_provider.clone().unwrap_or_default();
        let model = settings.llm_model.clone().unwrap_or_default();
        let Some(entry) = crate::ai_providers::LLM_PROVIDERS
            .iter()
            .find(|entry| entry.id == provider)
        else {
            return self.runtime.spawn(async { None });
        };
        if model.is_empty() {
            return self.runtime.spawn(async { None });
        }
        let config = settings.ai_providers("llm").remove(&provider);
        let base_url = config
            .as_ref()
            .map(|config| config.base_url.trim().to_string())
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| entry.base_url.unwrap_or_default().trim().to_string());
        let requires_key = entry.requirements.iter().any(|requirement| {
            matches!(requirement, crate::ai_providers::Requirement::Config(fields) if fields.contains(&"api_key"))
        });
        let requires_base_url = entry.requirements.iter().any(|requirement| {
            matches!(requirement, crate::ai_providers::Requirement::Config(fields) if fields.contains(&"base_url"))
        });
        let reasoning_effort = settings
            .string_setting(
                "current_llm_reasoning_effort",
                &["ai", "current_llm_reasoning_effort"],
            )
            .unwrap_or_else(|| "default".to_string());
        let keys = self.ai_provider_api_keys("llm", vec![provider.clone()]);
        self.runtime.spawn(async move {
            let api_key = keys
                .await
                .ok()
                .and_then(|keys| keys.into_iter().next())
                .and_then(|(_, result)| result.ok().flatten())
                .map(|key| key.trim().to_string())
                .unwrap_or_default();
            if (requires_key && api_key.is_empty()) || (requires_base_url && base_url.is_empty()) {
                return None;
            }
            Some(crate::llm_stream::Connection {
                provider_id: provider,
                base_url,
                api_key,
                model_id: model,
                reasoning_effort: crate::ai_models::normalize_reasoning_effort(&reasoning_effort)
                    .to_string(),
            })
        })
    }

    /// `loadSessionContentSnapshot` plus the rendered transcript segments.
    pub fn enhancer_snapshot(
        &self,
        session_id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<Option<Snapshot>>> {
        let db = self.db.clone();
        self.runtime
            .spawn(async move { load_snapshot(db.pool(), &session_id).await })
    }

    /// `getTemplateById` reduced to the prompt's fields.
    pub fn enhancer_template(
        &self,
        template_id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<Option<TemplateRecord>>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            if template_id.is_empty() {
                return Ok(None);
            }
            Ok(crate::templates::get(db.pool(), &template_id)
                .await?
                .map(|template| TemplateRecord {
                    title: template.title,
                    description: Some(template.description).filter(|d| !d.trim().is_empty()),
                    sections: template
                        .sections
                        .into_iter()
                        .map(|section| anlg_template_app::TemplateSection {
                            title: section.title,
                            description: Some(section.description).filter(|d| !d.trim().is_empty()),
                        })
                        .collect(),
                }))
        })
    }

    /// The settings the transform reads, plus the selected default template.
    pub fn enhancer_settings(
        &self,
    ) -> tokio::task::JoinHandle<anyhow::Result<(PromptSettings, Option<String>)>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let rows = sqlx::query_as::<_, (String, String, i64)>(super::SETTING_ROWS_SQL)
                .fetch_all(db.pool())
                .await?
                .into_iter()
                .map(|(id, json, _)| (id, json))
                .collect::<Vec<_>>();
            let settings = super::ProviderSettings::from_rows(&rows);
            Ok((
                PromptSettings {
                    ai_language: settings
                        .string_setting("ai_language", &["language", "ai_language"])
                        .or_else(|| Some("en".to_string())),
                    auto_summary_prompt: settings
                        .string_setting("auto_summary_prompt", &["ai", "auto_summary_prompt"])
                        .unwrap_or_default(),
                    summary_length: settings
                        .string_setting("summary_length", &["ai", "summary_length"]),
                    dictionary_terms_json: settings
                        .string_setting(
                            "personalization_dictionary_terms",
                            &["personalization", "dictionary_terms"],
                        )
                        .unwrap_or_else(|| "[]".to_string()),
                },
                settings
                    .string_setting("selected_template_id", &["general", "selected_template_id"]),
            ))
        })
    }

    /// `ensureSummaryDocument` / `ensurePendingAutoEnhanceDocument`: the note
    /// for the template (created as `Summary` when missing) and, when asked,
    /// the durable pending marker for it.
    pub fn enhancer_ensure_summary(
        &self,
        session_id: String,
        template_id: Option<String>,
        pending: bool,
    ) -> tokio::task::JoinHandle<anyhow::Result<(EnhancedNote, Option<PendingJob>)>> {
        let db = self.db.clone();
        let lock = self.session_lock(&session_id);
        self.runtime.spawn(async move {
            let _guard = lock.lock().await;
            let pool = db.pool();
            let Some(snapshot) = load_snapshot(pool, &session_id).await? else {
                anyhow::bail!("Session {session_id} no longer exists");
            };
            let template_id = template_id.unwrap_or_default();
            let now = now();
            if let Some(existing) = snapshot.matching_enhanced_note(Some(&template_id)) {
                if !pending {
                    return Ok((existing.clone(), None));
                }
                let generation = uuid::Uuid::new_v4().to_string();
                sqlx::query(UPSERT_PENDING_SQL)
                    .bind(pending_setting_id(&session_id))
                    .bind(pending_value(
                        &existing.id,
                        &existing.content,
                        &existing.content_format,
                        &generation,
                    ))
                    .bind(&now)
                    .execute(pool)
                    .await?;
                return Ok((
                    existing.clone(),
                    Some(PendingJob {
                        session_id,
                        note_id: existing.id.clone(),
                        template_id: existing.template_id.clone(),
                        expected_body: existing.content.clone(),
                        expected_content_format: existing.content_format.clone(),
                        generation,
                    }),
                ));
            }

            let note_id = uuid::Uuid::new_v4().to_string();
            let generation = if pending {
                uuid::Uuid::new_v4().to_string()
            } else {
                String::new()
            };
            let position = snapshot
                .enhanced_notes
                .iter()
                .map(|note| note.position)
                .max()
                .unwrap_or(0)
                + 1;
            let mut tx = pool.begin().await?;
            let inserted = sqlx::query(INSERT_SUMMARY_SQL)
                .bind(&note_id)
                .bind(if template_id.is_empty() {
                    "summary"
                } else {
                    "template_output"
                })
                .bind(&template_id)
                .bind(position)
                .bind(&now)
                .bind(&now)
                .bind(&session_id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
            if inserted != 1 {
                anyhow::bail!("Session {session_id} no longer exists");
            }
            if pending {
                sqlx::query(UPSERT_PENDING_SQL)
                    .bind(pending_setting_id(&session_id))
                    .bind(pending_value(&note_id, "", "prosemirror_json", &generation))
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
            }
            tx.commit().await?;
            let note = EnhancedNote {
                id: note_id.clone(),
                title: "Summary".to_string(),
                content: String::new(),
                content_format: "prosemirror_json".to_string(),
                template_id: template_id.clone(),
                position,
            };
            Ok((
                note,
                pending.then(|| PendingJob {
                    session_id,
                    note_id,
                    template_id,
                    expected_body: String::new(),
                    expected_content_format: "prosemirror_json".to_string(),
                    generation,
                }),
            ))
        })
    }

    /// `loadPendingAutoEnhanceJobs`.
    pub fn enhancer_pending_jobs(
        &self,
    ) -> tokio::task::JoinHandle<anyhow::Result<Vec<PendingJob>>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let offset = PENDING_AUTO_ENHANCE_SETTING_PREFIX.len() as i64 + 1;
            let rows = sqlx::query_as::<_, (String, String, String, String, String, String)>(
                PENDING_JOBS_SQL,
            )
            .bind(offset)
            .bind(offset)
            .bind(format!("{PENDING_AUTO_ENHANCE_SETTING_PREFIX}%"))
            .fetch_all(db.pool())
            .await?;
            Ok(rows
                .into_iter()
                .map(
                    |(
                        session_id,
                        note_id,
                        template_id,
                        expected_body,
                        expected_content_format,
                        generation,
                    )| {
                        PendingJob {
                            session_id,
                            note_id,
                            template_id,
                            expected_body,
                            expected_content_format,
                            generation,
                        }
                    },
                )
                .collect())
        })
    }

    /// `discardPendingAutoEnhanceJob`.
    pub fn enhancer_discard_pending(
        &self,
        job: PendingJob,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            sqlx::query(DISCARD_PENDING_SQL)
                .bind(pending_setting_id(&job.session_id))
                .bind(&job.note_id)
                .bind(&job.generation)
                .bind(&job.expected_body)
                .bind(&job.expected_content_format)
                .execute(db.pool())
                .await?;
            Ok(())
        })
    }

    /// `replaceSummaryDocumentTemplate`.
    pub fn enhancer_replace_template(
        &self,
        session_id: String,
        note_id: String,
        template_id: Option<String>,
        title: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let template_id = template_id.unwrap_or_default();
            let affected = sqlx::query(REPLACE_TEMPLATE_SQL)
                .bind(if template_id.is_empty() {
                    "summary"
                } else {
                    "template_output"
                })
                .bind(&template_id)
                .bind(&title)
                .bind(&session_id)
                .bind(now())
                .bind(&note_id)
                .bind(&session_id)
                .bind(&session_id)
                .execute(db.pool())
                .await?
                .rows_affected();
            if affected != 1 {
                anyhow::bail!("Summary {note_id} no longer exists");
            }
            Ok(())
        })
    }

    /// `updateSummaryDocumentTitleIfCurrent`.
    pub fn enhancer_update_title_if_current(
        &self,
        session_id: String,
        note_id: String,
        template_id: String,
        current_title: String,
        next_title: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            sqlx::query(UPDATE_TITLE_IF_CURRENT_SQL)
                .bind(&next_title)
                .bind(now())
                .bind(&note_id)
                .bind(&session_id)
                .bind(&template_id)
                .bind(&current_title)
                .execute(db.pool())
                .await?;
            Ok(())
        })
    }

    /// `persistGeneratedEnhancedNote`: the body swap guarded by the current
    /// body (and the pending marker), the marker's removal, and the tags.
    pub fn enhancer_persist_note(
        &self,
        session_id: String,
        owner_user_id: String,
        note: DocumentUpdate,
        tag_names: Vec<String>,
        pending: Option<PendingJob>,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let now = now();
            let user_id = if owner_user_id.trim().is_empty() {
                super::DEFAULT_USER_ID.to_string()
            } else {
                owner_user_id
            };
            let setting_id = pending_setting_id(&session_id);
            let mut tx = db.pool().begin().await?;
            let affected = match &pending {
                Some(job) => sqlx::query(PERSIST_NOTE_PENDING_SQL)
                    .bind(&note.next_content)
                    .bind(&now)
                    .bind(&note.id)
                    .bind(&session_id)
                    .bind(&note.current_content)
                    .bind(&note.current_content_format)
                    .bind(&session_id)
                    .bind(&setting_id)
                    .bind(&note.id)
                    .bind(&job.generation)
                    .bind(&job.expected_body)
                    .bind(&job.expected_content_format)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected(),
                None => sqlx::query(PERSIST_NOTE_SQL)
                    .bind(&note.next_content)
                    .bind(&now)
                    .bind(&note.id)
                    .bind(&session_id)
                    .bind(&note.current_content)
                    .bind(&note.current_content_format)
                    .bind(&session_id)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected(),
            };
            if affected != 1 {
                anyhow::bail!("The summary changed while it was being generated");
            }
            match &pending {
                Some(job) => {
                    let removed = sqlx::query(DISCARD_PENDING_SQL)
                        .bind(&setting_id)
                        .bind(&note.id)
                        .bind(&job.generation)
                        .bind(&job.expected_body)
                        .bind(&job.expected_content_format)
                        .execute(&mut *tx)
                        .await?
                        .rows_affected();
                    if removed != 1 {
                        anyhow::bail!(
                            "The pending summary marker changed while it was being generated"
                        );
                    }
                }
                None => {
                    sqlx::query(CLEAR_PENDING_BY_BODY_SQL)
                        .bind(&setting_id)
                        .bind(&note.id)
                        .bind(&note.current_content)
                        .execute(&mut *tx)
                        .await?;
                }
            }
            let mut seen: Vec<&str> = Vec::new();
            for tag in tag_names.iter().filter(|tag| !tag.is_empty()) {
                if seen.contains(&tag.as_str()) {
                    continue;
                }
                seen.push(tag);
                sqlx::query(UPSERT_TAG_SQL)
                    .bind(tag)
                    .bind(&user_id)
                    .bind(tag)
                    .bind(&now)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query(UPSERT_SESSION_TAG_SQL)
                    .bind(format!("{session_id}:{tag}"))
                    .bind(&user_id)
                    .bind(&session_id)
                    .bind(tag)
                    .bind(&now)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
            }
            tx.commit().await?;
            Ok(())
        })
    }

    /// `updateEnhancedNoteContent`: the edited summary body, dropping a
    /// pending auto-summary marker that no longer matches, and the session
    /// title extracted from the first line when the editor asks for it.
    pub fn update_enhanced_note_content(
        &self,
        note_id: String,
        session_id: String,
        content: String,
        session_title: Option<String>,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let now = now();
            let mut tx = db.pool().begin().await?;
            sqlx::query(UPDATE_ENHANCED_CONTENT_SQL)
                .bind(&content)
                .bind(&now)
                .bind(&note_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query(CLEAR_PENDING_UNLESS_BODY_SQL)
                .bind(pending_setting_id(&session_id))
                .bind(&note_id)
                .bind(&content)
                .execute(&mut *tx)
                .await?;
            if let Some(title) = session_title {
                sqlx::query(UPDATE_SESSION_TITLE_UNGUARDED_SQL)
                    .bind(&title)
                    .bind(&now)
                    .bind(&session_id)
                    .execute(&mut *tx)
                    .await?;
            }
            tx.commit().await?;
            Ok(())
        })
    }

    /// `applyGeneratedSessionTitle`: the title, guarded by its current value,
    /// with the documents' `# Title` headings.
    pub fn enhancer_apply_generated_title(
        &self,
        session_id: String,
        current_title: String,
        next_title: String,
        documents: Vec<DocumentUpdate>,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let now = now();
            let mut tx = db.pool().begin().await?;
            let affected = sqlx::query(UPDATE_SESSION_TITLE_SQL)
                .bind(&next_title)
                .bind(&now)
                .bind(&session_id)
                .bind(&current_title)
                .execute(&mut *tx)
                .await?
                .rows_affected();
            if affected != 1 {
                anyhow::bail!("The session title changed while it was being generated");
            }
            for document in &documents {
                let affected = sqlx::query(UPDATE_DOCUMENT_BODY_SQL)
                    .bind(&document.next_content)
                    .bind(&now)
                    .bind(&document.id)
                    .bind(&session_id)
                    .bind(&document.current_content)
                    .bind(&document.current_content_format)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected();
                if affected != 1 {
                    anyhow::bail!("A document changed while its title was being generated");
                }
            }
            tx.commit().await?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::body_to_markdown;

    #[test]
    fn body_markdown_matches_json2md_loose_lists() {
        let body = r#"{"type":"doc","content":[
            {"type":"heading","attrs":{"level":1},"content":[{"type":"text","text":"Next Steps"}]},
            {"type":"bulletList","content":[
                {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Ship it."}]}]},
                {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Close bugs."}]}]}
            ]}
        ]}"#;
        assert_eq!(
            body_to_markdown(body, "tiptap"),
            "# Next Steps\n\n- Ship it.\n\n- Close bugs."
        );
        assert_eq!(body_to_markdown("- a\n- b", "markdown"), "- a\n- b");
        assert_eq!(body_to_markdown("", "tiptap"), "");
    }

    #[test]
    fn empty_paragraphs_leave_json2md_newlines() {
        let doc = |blocks: &str| format!(r#"{{"type":"doc","content":[{blocks}]}}"#);
        let para = |text: &str| {
            format!(r#"{{"type":"paragraph","content":[{{"type":"text","text":"{text}"}}]}}"#)
        };
        let empty = r#"{"type":"paragraph"}"#;
        assert_eq!(
            body_to_markdown(
                &doc(&format!("{},{empty},{}", para("a"), para("b"))),
                "tiptap"
            ),
            "a\n\n\nb"
        );
        assert_eq!(
            body_to_markdown(&doc(&format!("{empty},{}", para("a"))), "tiptap"),
            "\na"
        );
        assert_eq!(
            body_to_markdown(&doc(&format!("{},{empty},{empty}", para("a"))), "tiptap"),
            "a\n\n\n"
        );
        assert_eq!(body_to_markdown(&doc(empty), "tiptap"), "");
        // The trailing paragraph after an image is what `imageTrailingParagraph` keeps.
        let image = r#"{"type":"image","attrs":{"src":"asset://localhost/%2Fa%2Fattachments%2Fimage.png","alt":null,"title":null,"attachmentId":"image.png","sharedAttachmentId":null,"editorWidth":80}}"#;
        assert_eq!(
            body_to_markdown(
                &doc(&format!("{},{image},{empty}", para("delta beta"))),
                "tiptap"
            ),
            "delta beta\n\n![](asset://localhost/%2Fa%2Fattachments%2Fimage.png \"char-editor-width=80\")\n\n"
        );
    }
}
