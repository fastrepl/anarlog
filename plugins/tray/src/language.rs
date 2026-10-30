#[derive(Debug, Clone, serde::Deserialize, specta::Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrayLanguageOption {
    pub code: String,
    pub label: String,
    pub selected: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, tauri_specta::Event)]
pub struct TrayLanguageSelected {
    pub code: String,
}
