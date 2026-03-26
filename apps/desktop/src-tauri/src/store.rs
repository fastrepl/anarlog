use tauri_plugin_store2::ScopedStoreKey;

#[derive(serde::Deserialize, specta::Type, PartialEq, Eq, Hash, strum::Display)]
pub enum StoreKey {
    OnboardingNeeded2,
    DismissedToasts,
    OnboardingLocal,
    TinybaseValues,
    PinnedTabs,
    RecentlyOpenedSessions,
}

impl ScopedStoreKey for StoreKey {}
