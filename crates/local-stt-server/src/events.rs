#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "tauri-event", derive(tauri_specta::Event))]
#[serde(tag = "type")]
pub enum LocalServerEvent {
    #[serde(rename = "starting")]
    Starting,
    #[serde(rename = "ready")]
    Ready { base_url: String },
    #[serde(rename = "stopping")]
    Stopping,
    #[serde(rename = "stopped")]
    Stopped,
    #[serde(rename = "error")]
    Error { error: String },
}
