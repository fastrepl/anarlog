pub mod external;
#[cfg(feature = "whisper-cpp")]
pub mod internal;
pub mod supervisor;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type,
)]
pub enum ServerType {
    #[serde(rename = "internal")]
    Internal,
    #[serde(rename = "external")]
    External,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type,
)]
#[serde(rename_all = "lowercase")]
pub enum ServerStatus {
    Unreachable,
    Loading,
    Ready,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ServerInfo {
    pub url: Option<String>,
    pub status: ServerStatus,
    pub model: Option<crate::LocalModel>,
}
