use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListSessions<'a> {
    /// Case-insensitive substring matched against the session title or id.
    pub query: Option<&'a str>,
    /// Exact recurring-series id match.
    pub series_id: Option<&'a str>,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct SessionListItem {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct SessionRow {
    pub id: String,
    pub workspace_id: String,
    pub owner_user_id: String,
    pub title: String,
    pub kind: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: String,
    pub ended_at: String,
    pub timezone: String,
    pub language: String,
    pub event_id: String,
    pub external_event_id: String,
    pub external_provider: String,
    pub series_id: String,
    pub source_apps_json: String,
    pub event_json: String,
    pub folder_path: String,
    pub slug: String,
    pub metadata_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct SessionDocumentRow {
    pub id: String,
    pub workspace_id: String,
    pub session_id: String,
    pub kind: String,
    pub template_id: String,
    pub title: String,
    pub body_format: String,
    pub body: String,
    pub source_hash: String,
    pub generation_metadata_json: String,
    pub sort_order: i64,
    pub created_by: String,
    pub updated_by: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct SessionTranscriptRow {
    pub id: String,
    pub workspace_id: String,
    pub owner_user_id: String,
    pub session_id: String,
    pub source: String,
    pub provider: String,
    pub model: String,
    pub language: String,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub audio_attachment_id: String,
    pub memo: String,
    pub words_json: String,
    pub speaker_hints_json: String,
    pub metadata_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct SessionParticipantRow {
    pub id: String,
    pub workspace_id: String,
    pub owner_user_id: String,
    pub session_id: String,
    pub human_id: String,
    pub display_name: String,
    pub email: String,
    pub role: String,
    pub source: String,
    pub job_title: String,
    pub organization_id: String,
    pub organization_name: String,
    pub metadata_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow)]
pub struct SessionActionItemRow {
    pub id: String,
    pub workspace_id: String,
    pub session_id: String,
    pub source_type: String,
    pub source_id: String,
    pub source_order: i64,
    pub assignee_human_id: String,
    pub status: String,
    pub text: String,
    pub body_json: String,
    pub due_at: String,
    pub completed_at: Option<String>,
    pub created_by: String,
    pub updated_by: String,
    pub metadata_json: String,
    pub created_at: String,
    pub updated_at: String,
}
