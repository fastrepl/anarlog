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
    pub(super) fn table_name(&self) -> &'static str {
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

    pub(super) fn id(&self) -> &str {
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

    pub(super) fn existing_sqlite_is_authoritative(&self) -> bool {
        matches!(self, Self::Calendar(_) | Self::Event(_) | Self::Template(_))
            || matches!(self, Self::Session(row) if row.recovery_status.is_some())
    }

    pub(super) fn recovery_status(&self) -> Option<&str> {
        match self {
            Self::Session(row) => row.recovery_status.as_deref(),
            Self::Document(row) => row.recovery_status.as_deref(),
            Self::Transcript(row) => row.recovery_status.as_deref(),
            _ => None,
        }
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
    pub metadata_json: String,
    pub folder_path: String,
    pub recovery_status: Option<String>,
}

#[derive(Debug, Clone)]
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
    pub generation_metadata_json: String,
    pub recovery_status: Option<String>,
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
    pub metadata_json: String,
    pub recovery_status: Option<String>,
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
