use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionIngestEnvelope {
    pub schema_version: u32,
    pub source_id: String,
    pub revision: u64,
    pub finalized: bool,
    #[serde(default)]
    pub attach_to_existing: bool,
    pub workspace_id: String,
    pub owner_user_id: String,
    pub session: IngestSession,
    #[serde(default)]
    pub documents: Vec<IngestDocument>,
    #[serde(default)]
    pub transcripts: Vec<IngestTranscript>,
    #[serde(default)]
    pub participants: Vec<IngestParticipant>,
    #[serde(default)]
    pub attachments: Vec<IngestAttachment>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IngestSession {
    pub id: String,
    pub title: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub started_at: String,
    #[serde(default)]
    pub ended_at: String,
    #[serde(default)]
    pub timezone: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub external_event_id: String,
    #[serde(default)]
    pub external_provider: String,
    #[serde(default)]
    pub series_id: String,
    #[serde(default)]
    pub event: Value,
    #[serde(default)]
    pub metadata: Map<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    Note,
    Summary,
    TemplateOutput,
}

#[cfg(feature = "apply")]
impl DocumentKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::Summary => "summary",
            Self::TemplateOutput => "template_output",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentFormat {
    Markdown,
    ProsemirrorJson,
}

#[cfg(feature = "apply")]
impl DocumentFormat {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::ProsemirrorJson => "prosemirror_json",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IngestDocument {
    pub id: String,
    pub kind: DocumentKind,
    pub format: DocumentFormat,
    #[serde(default)]
    pub template_id: String,
    #[serde(default)]
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub source_hash: String,
    #[serde(default)]
    pub generation_metadata: Map<String, Value>,
    #[serde(default)]
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IngestTranscript {
    pub id: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    #[serde(default)]
    pub audio_attachment_id: String,
    #[serde(default)]
    pub memo: String,
    #[serde(default)]
    pub words: Vec<IngestWord>,
    #[serde(default)]
    pub speaker_hints: Vec<IngestSpeakerHint>,
    #[serde(default)]
    pub metadata: Map<String, Value>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IngestWord {
    pub id: String,
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
    #[serde(default)]
    pub channel: i64,
    pub speaker: Option<String>,
    #[serde(default)]
    pub metadata: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IngestSpeakerHint {
    pub id: String,
    pub word_id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IngestParticipant {
    pub id: String,
    #[serde(default)]
    pub human_id: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub metadata: Map<String, Value>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IngestAttachment {
    pub id: String,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub object_key: String,
    #[serde(default)]
    pub metadata: Map<String, Value>,
    pub created_at: String,
    pub updated_at: String,
}
