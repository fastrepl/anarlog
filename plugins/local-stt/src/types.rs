#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CustomSttModelFormat {
    Ggml,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CustomSttModelInfo {
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    pub format: CustomSttModelFormat,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressPayload {
    pub model: crate::LocalModel,
    pub status: anlg_model_downloader::DownloadStatus,
}

#[derive(Debug)]
pub struct Connection {
    pub model: Option<String>,
    pub base_url: String,
    pub api_key: Option<String>,
}
