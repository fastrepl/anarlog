use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptItem {
    pub speaker: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub items: Vec<TranscriptItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportMetadata {
    pub title: String,
    pub created_at: String,
    pub participants: Vec<String>,
    pub event_title: Option<String>,
    pub duration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportInput {
    pub enhanced_md: String,
    pub transcript: Option<Transcript>,
    pub metadata: Option<ExportMetadata>,
}
