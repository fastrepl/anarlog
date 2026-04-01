#[derive(serde::Serialize, Clone, specta::Type)]
#[cfg_attr(feature = "tauri-event", derive(tauri_specta::Event))]
#[serde(tag = "type")]
pub enum AudioImportEvent {
    #[serde(rename = "audioImportStarted")]
    Started { session_id: String },
    #[serde(rename = "audioImportProgress")]
    Progress { session_id: String, percentage: f64 },
    #[serde(rename = "audioImportCompleted")]
    Completed { session_id: String },
    #[serde(rename = "audioImportFailed")]
    Failed { session_id: String, error: String },
}

pub trait AudioImportRuntime: Send + Sync {
    fn emit(&self, event: AudioImportEvent);
}
