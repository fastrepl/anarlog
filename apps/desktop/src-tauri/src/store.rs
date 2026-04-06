use tauri_plugin_store2::ScopedStoreKey;

#[derive(serde::Deserialize, specta::Type, PartialEq, Eq, Hash, strum::Display)]
pub enum StoreKey {
    OnboardingNeeded2,
    DismissedToasts,
    OnboardingLocal,
    TinybaseValues,
    PinnedTabs,
    RecentlyOpenedSessions,
    CharV1p1Preview,
}

impl ScopedStoreKey for StoreKey {}
