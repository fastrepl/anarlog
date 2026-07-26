#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressPayload {
    pub model: crate::LocalModel,
    pub status: hypr_model_downloader::DownloadStatus,
}

#[derive(Debug)]
pub struct Connection {
    pub model: Option<String>,
    pub base_url: String,
    pub api_key: Option<String>,
}
