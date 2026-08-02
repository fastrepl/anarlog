pub const VOICEPRINT_KEYRING_SCOPE: &str = "voiceprint_exemplars";
pub const VOICEPRINT_SYNC_SCOPE: &str = "local_only";

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct VoiceprintExemplar {
    pub id: String,
    pub workspace_id: String,
    pub human_id: String,
    pub keyring_scope: String,
    pub keyring_key: String,
    pub sync_scope: String,
    pub model_provider: String,
    pub model_version: String,
    pub capture_domain: String,
    pub confirmation_source: String,
    pub source_session_id: String,
    pub source_transcript_id: String,
    pub source_attachment_id: String,
    pub source_speaker_label: String,
    pub source_start_ms: i64,
    pub source_end_ms: i64,
    pub quality_score: f64,
    pub label_confidence: f64,
    pub confirmed_at: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

pub struct NewVoiceprintExemplar<'a> {
    pub id: &'a str,
    pub workspace_id: &'a str,
    pub human_id: &'a str,
    pub keyring_key: &'a str,
    pub model_provider: &'a str,
    pub model_version: &'a str,
    pub capture_domain: &'a str,
    pub confirmation_source: &'a str,
    pub source_session_id: &'a str,
    pub source_transcript_id: &'a str,
    pub source_attachment_id: &'a str,
    pub source_speaker_label: &'a str,
    pub source_start_ms: i64,
    pub source_end_ms: i64,
    pub quality_score: f64,
    pub label_confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct VoiceprintSecretRef {
    pub keyring_scope: String,
    pub keyring_key: String,
}

#[derive(Debug)]
pub enum VoiceprintExemplarError {
    InvalidField(&'static str),
    HumanNotFound,
    SourceNotFound,
    Sqlx(sqlx::Error),
}

impl std::fmt::Display for VoiceprintExemplarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidField(field) => write!(f, "invalid voiceprint exemplar {field}"),
            Self::HumanNotFound => write!(f, "voiceprint exemplar human does not exist"),
            Self::SourceNotFound => write!(f, "voiceprint exemplar source does not exist"),
            Self::Sqlx(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for VoiceprintExemplarError {}

impl From<sqlx::Error> for VoiceprintExemplarError {
    fn from(error: sqlx::Error) -> Self {
        Self::Sqlx(error)
    }
}
