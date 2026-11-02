use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportTextFile {
    pub path: String,
    pub name: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meeting_audio: Option<MeetingPackageAudio>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MeetingPackageAudio {
    pub path: String,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedImportAuthorization {
    pub provider_id: String,
    pub authorization_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedImportCredentials {
    pub provider_id: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub token_json: String,
    pub token_received_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedImportSyncResult {
    pub files: Vec<ImportTextFile>,
    pub credentials: ConnectedImportCredentials,
    pub warnings: Vec<String>,
}
