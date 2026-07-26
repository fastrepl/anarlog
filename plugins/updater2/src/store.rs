use tauri_plugin_store2::ScopedStoreKey;

#[derive(serde::Deserialize, specta::Type, PartialEq, Eq, Hash, strum::Display)]
pub enum StoreKey {
    AutomaticUpdatesEnabled,
    LastSeenVersion,
}

impl ScopedStoreKey for StoreKey {}
