#![forbid(unsafe_code)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use sqlx::SqlitePool;

pub const DEFAULT_LIST_LIMIT: u32 = 20;
pub const MAX_LIST_LIMIT: u32 = 200;
pub const DEFAULT_TRANSCRIPT_LIMIT: u32 = 200;
pub const MAX_TRANSCRIPT_LIMIT: u32 = 500;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0} not found")]
    NotFound(String),
    #[error("{action} failed: {source}")]
    Database {
        action: &'static str,
        #[source]
        source: sqlx::Error,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "snake_case")]
pub struct ListMeetingsInput {
    #[schemars(description = "Case-insensitive title or meeting id substring")]
    pub query: Option<String>,
    #[schemars(description = "Exact recurring series id")]
    pub series_id: Option<String>,
    #[schemars(description = "Maximum results; defaults to 20 and is capped at 200")]
    #[schemars(range(min = 1, max = 200))]
    pub limit: Option<u32>,
    #[schemars(description = "Number of results to skip; defaults to 0")]
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "snake_case")]
pub struct GetMeetingInput {
    #[schemars(description = "Anarlog meeting id")]
    pub meeting_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "snake_case")]
pub struct GetMeetingTranscriptInput {
    #[schemars(description = "Anarlog meeting id")]
    pub meeting_id: String,
    #[schemars(description = "Word offset; defaults to 0")]
    pub offset: Option<u32>,
    #[schemars(description = "Maximum words; defaults to 200 and is capped at 500")]
    #[schemars(range(min = 1, max = 500))]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "snake_case")]
pub struct GetRecurringMeetingHistoryInput {
    #[schemars(description = "A meeting id used to resolve its recurring series")]
    pub meeting_id: String,
    #[schemars(description = "Maximum meetings; defaults to 20 and is capped at 200")]
    #[schemars(range(min = 1, max = 200))]
    pub limit: Option<u32>,
    #[schemars(description = "Number of meetings to skip; defaults to 0")]
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub struct Pagination {
    pub offset: u32,
    pub limit: u32,
    pub returned: usize,
    pub total: Option<usize>,
    pub next_offset: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub struct MeetingListItem {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: String,
    pub ended_at: String,
    pub series_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub struct MeetingPage {
    pub meetings: Vec<MeetingListItem>,
    pub pagination: Pagination,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub struct TranscriptPage {
    pub meeting_id: String,
    pub text: String,
    pub words: Vec<Value>,
    pub pagination: Pagination,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub struct Document {
    pub id: String,
    pub kind: String,
    pub template_id: String,
    pub title: String,
    pub markdown: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub struct Participant {
    pub human_id: String,
    pub display_name: String,
    pub email: String,
    pub role: String,
    pub job_title: String,
    pub organization_id: String,
    pub organization_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub struct ActionItem {
    pub id: String,
    pub assignee_human_id: String,
    pub status: String,
    pub text: String,
    pub due_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub struct Meeting {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: String,
    pub ended_at: String,
    pub timezone: String,
    pub language: String,
    pub series_id: String,
    pub note: Option<Document>,
    pub summaries: Vec<Document>,
    pub participants: Vec<Participant>,
    pub action_items: Vec<ActionItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub struct Transcript {
    pub id: String,
    pub source: String,
    pub provider: String,
    pub model: String,
    pub language: String,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub memo: String,
    pub text: String,
    pub words: Vec<Value>,
    pub speaker_hints: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub struct MeetingExport {
    #[serde(flatten)]
    pub meeting: Meeting,
    pub transcripts: Vec<Transcript>,
}

pub async fn list_meetings(pool: &SqlitePool, input: ListMeetingsInput) -> Result<MeetingPage> {
    let limit = input
        .limit
        .unwrap_or(DEFAULT_LIST_LIMIT)
        .clamp(1, MAX_LIST_LIMIT);
    let offset = input.offset.unwrap_or(0);
    let mut meetings = hypr_db_app::list_sessions(
        pool,
        hypr_db_app::ListSessions {
            query: input.query.as_deref(),
            series_id: input.series_id.as_deref(),
            limit: limit + 1,
            offset,
        },
    )
    .await
    .map_err(|source| Error::Database {
        action: "list meetings",
        source,
    })?;
    let has_more = meetings.len() > limit as usize;
    meetings.truncate(limit as usize);
    let meetings = meetings
        .into_iter()
        .map(MeetingListItem::from)
        .collect::<Vec<_>>();
    let pagination = pagination(offset, limit, meetings.len(), None, has_more);

    Ok(MeetingPage {
        meetings,
        pagination,
    })
}

pub async fn get_meeting(pool: &SqlitePool, input: GetMeetingInput) -> Result<Meeting> {
    let meeting_id = input.meeting_id;
    let (session, note, documents, participants, action_items) = tokio::try_join!(
        hypr_db_app::get_session(pool, &meeting_id),
        hypr_db_app::get_session_note(pool, &meeting_id),
        hypr_db_app::list_session_documents(pool, &meeting_id),
        hypr_db_app::list_session_participants(pool, &meeting_id),
        hypr_db_app::list_session_action_items(pool, &meeting_id),
    )
    .map_err(|source| Error::Database {
        action: "load meeting",
        source,
    })?;

    let session = session.ok_or_else(|| Error::NotFound(format!("meeting '{meeting_id}'")))?;
    let summaries = documents
        .into_iter()
        .filter(|document| matches!(document.kind.as_str(), "summary" | "template_output"))
        .map(Document::from)
        .collect();

    Ok(Meeting {
        id: session.id,
        title: session.title,
        kind: session.kind,
        status: session.status,
        created_at: session.created_at,
        updated_at: session.updated_at,
        started_at: session.started_at,
        ended_at: session.ended_at,
        timezone: session.timezone,
        language: session.language,
        series_id: session.series_id,
        note: note.map(Document::from),
        summaries,
        participants: participants.into_iter().map(Participant::from).collect(),
        action_items: action_items.into_iter().map(ActionItem::from).collect(),
    })
}

pub async fn get_meeting_transcript(
    pool: &SqlitePool,
    input: GetMeetingTranscriptInput,
) -> Result<TranscriptPage> {
    let exists = hypr_db_app::get_session(pool, &input.meeting_id)
        .await
        .map_err(|source| Error::Database {
            action: "load meeting",
            source,
        })?
        .is_some();
    if !exists {
        return Err(Error::NotFound(format!("meeting '{}'", input.meeting_id)));
    }

    let transcripts = load_transcripts(pool, &input.meeting_id).await?;
    Ok(transcript_page(
        &input.meeting_id,
        &transcripts,
        input.offset.unwrap_or(0),
        input
            .limit
            .unwrap_or(DEFAULT_TRANSCRIPT_LIMIT)
            .clamp(1, MAX_TRANSCRIPT_LIMIT),
    ))
}

pub async fn get_recurring_meeting_history(
    pool: &SqlitePool,
    input: GetRecurringMeetingHistoryInput,
) -> Result<MeetingPage> {
    let meeting = hypr_db_app::get_session(pool, &input.meeting_id)
        .await
        .map_err(|source| Error::Database {
            action: "load meeting",
            source,
        })?
        .ok_or_else(|| Error::NotFound(format!("meeting '{}'", input.meeting_id)))?;
    let series_id = meeting.series_id.trim();
    let limit = input
        .limit
        .unwrap_or(DEFAULT_LIST_LIMIT)
        .clamp(1, MAX_LIST_LIMIT);
    let offset = input.offset.unwrap_or(0);
    if series_id.is_empty() {
        return Ok(MeetingPage {
            meetings: Vec::new(),
            pagination: pagination(offset, limit, 0, Some(0), false),
        });
    }

    list_meetings(
        pool,
        ListMeetingsInput {
            query: None,
            series_id: Some(series_id.to_string()),
            limit: Some(limit),
            offset: Some(offset),
        },
    )
    .await
}

pub async fn get_meeting_export(pool: &SqlitePool, meeting_id: String) -> Result<MeetingExport> {
    let (meeting, transcripts) = tokio::try_join!(
        get_meeting(
            pool,
            GetMeetingInput {
                meeting_id: meeting_id.clone(),
            }
        ),
        load_transcripts(pool, &meeting_id),
    )?;
    Ok(MeetingExport {
        meeting,
        transcripts,
    })
}

impl Meeting {
    pub fn to_markdown(&self) -> String {
        let title = if self.title.trim().is_empty() {
            "Untitled meeting"
        } else {
            self.title.trim()
        };
        let mut sections = vec![format!("# {title}"), self.metadata_markdown()];

        if let Some(note) = &self.note {
            push_section(&mut sections, "Notes", &note.markdown);
        }
        for summary in &self.summaries {
            let heading = if summary.title.trim().is_empty() {
                "Summary"
            } else {
                summary.title.trim()
            };
            push_section(&mut sections, heading, &summary.markdown);
        }
        if !self.action_items.is_empty() {
            let body = self
                .action_items
                .iter()
                .map(|item| {
                    let checked = matches!(item.status.as_str(), "done" | "completed");
                    format!("- [{}] {}", if checked { "x" } else { " " }, item.text)
                })
                .collect::<Vec<_>>()
                .join("\n");
            push_section(&mut sections, "Action items", &body);
        }

        sections.join("\n\n").trim().to_string()
    }

    fn metadata_markdown(&self) -> String {
        let occurred_at = if self.started_at.is_empty() {
            &self.created_at
        } else {
            &self.started_at
        };
        let mut lines = vec![
            format!("- ID: `{}`", self.id),
            format!("- Date: {occurred_at}"),
        ];
        if !self.series_id.is_empty() {
            lines.push(format!("- Series: `{}`", self.series_id));
        }
        let people = self
            .participants
            .iter()
            .filter_map(|participant| {
                let name = participant.display_name.trim();
                (!name.is_empty()).then_some(name)
            })
            .collect::<Vec<_>>();
        if !people.is_empty() {
            lines.push(format!("- Participants: {}", people.join(", ")));
        }
        lines.join("\n")
    }
}

impl MeetingExport {
    pub fn to_markdown(&self) -> String {
        let mut markdown = self.meeting.to_markdown();
        let transcript = render_transcripts(&self.transcripts);
        if !transcript.is_empty() {
            markdown.push_str("\n\n## Transcript\n\n");
            markdown.push_str(&transcript);
        }
        markdown
    }
}

impl From<hypr_db_app::SessionListItem> for MeetingListItem {
    fn from(value: hypr_db_app::SessionListItem) -> Self {
        Self {
            id: value.id,
            title: value.title,
            kind: value.kind,
            status: value.status,
            created_at: value.created_at,
            updated_at: value.updated_at,
            started_at: value.started_at,
            ended_at: value.ended_at,
            series_id: value.series_id,
        }
    }
}

impl From<hypr_db_app::SessionDocumentRow> for Document {
    fn from(value: hypr_db_app::SessionDocumentRow) -> Self {
        Self {
            id: value.id,
            kind: value.kind,
            template_id: value.template_id,
            title: value.title,
            markdown: body_to_markdown(&value.body, &value.body_format),
            sort_order: value.sort_order,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<hypr_db_app::SessionParticipantRow> for Participant {
    fn from(value: hypr_db_app::SessionParticipantRow) -> Self {
        Self {
            human_id: value.human_id,
            display_name: value.display_name,
            email: value.email,
            role: value.role,
            job_title: value.job_title,
            organization_id: value.organization_id,
            organization_name: value.organization_name,
        }
    }
}

impl From<hypr_db_app::SessionActionItemRow> for ActionItem {
    fn from(value: hypr_db_app::SessionActionItemRow) -> Self {
        Self {
            id: value.id,
            assignee_human_id: value.assignee_human_id,
            status: value.status,
            text: value.text,
            due_at: value.due_at,
            completed_at: value.completed_at,
        }
    }
}

impl From<hypr_db_app::SessionTranscriptRow> for Transcript {
    fn from(value: hypr_db_app::SessionTranscriptRow) -> Self {
        let words = json_array(&value.words_json);
        let text = transcript_text(&words);
        Self {
            id: value.id,
            source: value.source,
            provider: value.provider,
            model: value.model,
            language: value.language,
            started_at_ms: value.started_at_ms,
            ended_at_ms: value.ended_at_ms,
            memo: value.memo,
            text,
            words,
            speaker_hints: json_array(&value.speaker_hints_json),
        }
    }
}

async fn load_transcripts(pool: &SqlitePool, meeting_id: &str) -> Result<Vec<Transcript>> {
    hypr_db_app::list_session_transcripts(pool, meeting_id)
        .await
        .map(|rows| rows.into_iter().map(Transcript::from).collect())
        .map_err(|source| Error::Database {
            action: "load transcript",
            source,
        })
}

fn transcript_page(
    meeting_id: &str,
    transcripts: &[Transcript],
    offset: u32,
    limit: u32,
) -> TranscriptPage {
    let mut words = Vec::new();
    for transcript in transcripts {
        for word in &transcript.words {
            let mut word = word.clone();
            if let Some(object) = word.as_object_mut() {
                object.insert(
                    "transcript_id".to_string(),
                    Value::String(transcript.id.clone()),
                );
            }
            words.push(word);
        }
    }

    let total_words = words.len();
    let offset_usize = offset as usize;
    let limit = limit.clamp(1, MAX_TRANSCRIPT_LIMIT);
    let words = words
        .into_iter()
        .skip(offset_usize)
        .take(limit as usize)
        .collect::<Vec<_>>();
    let text = transcript_page_text(&words);
    let has_more = offset_usize.saturating_add(words.len()) < total_words;

    TranscriptPage {
        meeting_id: meeting_id.to_string(),
        text,
        pagination: pagination(offset, limit, words.len(), Some(total_words), has_more),
        words,
    }
}

fn render_transcripts(transcripts: &[Transcript]) -> String {
    transcripts
        .iter()
        .filter(|transcript| !transcript.text.trim().is_empty())
        .map(|transcript| transcript.text.trim())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn body_to_markdown(body: &str, format: &str) -> String {
    if format != "prosemirror_json" {
        return body.to_string();
    }
    serde_json::from_str(body)
        .ok()
        .and_then(|value| hypr_tiptap::tiptap_json_to_md(&value).ok())
        .map(|markdown| markdown.trim_end().to_string())
        .unwrap_or_else(|| body.to_string())
}

fn json_array(value: &str) -> Vec<Value> {
    serde_json::from_str(value).unwrap_or_default()
}

fn transcript_text(words: &[Value]) -> String {
    words
        .iter()
        .filter_map(|word| word.get("text").and_then(Value::as_str))
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn transcript_page_text(words: &[Value]) -> String {
    let mut segments = Vec::new();
    let mut transcript_id = None;
    let mut segment = Vec::new();

    for word in words {
        let next_transcript_id = word.get("transcript_id").and_then(Value::as_str);
        if transcript_id.is_some() && transcript_id != next_transcript_id {
            segments.push(segment.join(" "));
            segment.clear();
        }
        transcript_id = next_transcript_id;

        if let Some(text) = word.get("text").and_then(Value::as_str) {
            let text = text.trim();
            if !text.is_empty() {
                segment.push(text);
            }
        }
    }

    if !segment.is_empty() {
        segments.push(segment.join(" "));
    }

    segments.join("\n\n")
}

fn push_section(sections: &mut Vec<String>, title: &str, body: &str) {
    if !body.trim().is_empty() {
        sections.push(format!("## {title}\n\n{}", body.trim()));
    }
}

fn pagination(
    offset: u32,
    limit: u32,
    returned: usize,
    total: Option<usize>,
    has_more: bool,
) -> Pagination {
    Pagination {
        offset,
        limit,
        returned,
        total,
        next_offset: has_more.then(|| offset.saturating_add(returned as u32)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> hypr_db_core::Db {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        db
    }

    #[test]
    fn converts_prosemirror_and_tolerates_invalid_json() {
        let body = serde_json::json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": "Hello"}]
            }]
        })
        .to_string();
        assert_eq!(body_to_markdown(&body, "prosemirror_json"), "Hello");
        assert_eq!(body_to_markdown("{broken", "prosemirror_json"), "{broken");
    }

    #[test]
    fn transcript_text_uses_word_text() {
        let words = serde_json::json!([
            {"text": " Hello "},
            {"text": "world."},
            {"other": "ignored"}
        ]);
        assert_eq!(transcript_text(words.as_array().unwrap()), "Hello world.");
    }

    #[test]
    fn transcript_page_text_preserves_transcript_boundaries() {
        let words = serde_json::json!([
            {"text": "First segment", "transcript_id": "transcript-1"},
            {"text": "continues.", "transcript_id": "transcript-1"},
            {"text": "Second segment.", "transcript_id": "transcript-2"}
        ]);

        assert_eq!(
            transcript_page_text(words.as_array().unwrap()),
            "First segment continues.\n\nSecond segment."
        );
    }

    #[tokio::test]
    async fn operations_return_curated_meeting_data() {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO sessions
             (id, title, started_at, series_id, workspace_id, owner_user_id, metadata_json)
             VALUES
             ('meeting-1', 'Planning', '2026-07-13', 'series-1', 'workspace-1', 'owner-1', '{\"private\":true}'),
             ('meeting-2', 'Prior planning', '2026-07-06', 'series-1', 'workspace-1', 'owner-1', '{}')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_documents
             (id, session_id, kind, body_format, body, title)
             VALUES
             ('meeting-1', 'meeting-1', 'note', 'markdown', 'Launch decision', 'Notes'),
             ('summary-1', 'meeting-1', 'summary', 'markdown', 'Ship Tuesday', 'Summary')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_participants
             (id, session_id, human_id, display_name, email, role, metadata_json)
             VALUES ('participant-1', 'meeting-1', 'human-1', 'Alice', 'alice@example.com', 'attendee', '{\"private\":true}')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO action_items
             (id, session_id, assignee_human_id, status, text, due_at, metadata_json)
             VALUES ('action-1', 'meeting-1', 'human-1', 'open', 'Prepare launch', '2026-07-20', '{\"private\":true}')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transcripts
             (id, session_id, started_at_ms, words_json, memo, metadata_json)
             VALUES ('transcript-1', 'meeting-1', 0, '[{\"text\":\"one\"},{\"text\":\"two\"}]', 'internal memo', '{\"private\":true}')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let listed = list_meetings(
            db.pool(),
            ListMeetingsInput {
                query: Some("plan".to_string()),
                limit: Some(1),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(listed.meetings[0].id, "meeting-1");
        assert_eq!(listed.pagination.next_offset, Some(1));

        let meeting = get_meeting(
            db.pool(),
            GetMeetingInput {
                meeting_id: "meeting-1".to_string(),
            },
        )
        .await
        .unwrap();
        assert_eq!(meeting.note.as_ref().unwrap().markdown, "Launch decision");
        assert_eq!(meeting.participants[0].display_name, "Alice");
        assert_eq!(meeting.action_items[0].text, "Prepare launch");
        let serialized = serde_json::to_value(&meeting).unwrap();
        assert!(serialized.get("workspace_id").is_none());
        assert!(serialized.get("owner_user_id").is_none());
        assert!(serialized.get("metadata_json").is_none());
        assert!(serialized["participants"][0].get("metadata_json").is_none());

        let transcript = get_meeting_transcript(
            db.pool(),
            GetMeetingTranscriptInput {
                meeting_id: "meeting-1".to_string(),
                offset: Some(1),
                limit: Some(1),
            },
        )
        .await
        .unwrap();
        assert_eq!(transcript.text, "two");
        assert_eq!(transcript.pagination.total, Some(2));
        assert_eq!(transcript.words[0]["transcript_id"], "transcript-1");

        let history = get_recurring_meeting_history(
            db.pool(),
            GetRecurringMeetingHistoryInput {
                meeting_id: "meeting-1".to_string(),
                limit: None,
                offset: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            history
                .meetings
                .iter()
                .map(|meeting| meeting.id.as_str())
                .collect::<Vec<_>>(),
            vec!["meeting-1", "meeting-2"]
        );
    }
}
