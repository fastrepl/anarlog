#[derive(serde::Serialize, Clone, specta::Type, tauri_specta::Event)]
pub struct NetworkStatusEvent {
    pub is_online: bool,
}
