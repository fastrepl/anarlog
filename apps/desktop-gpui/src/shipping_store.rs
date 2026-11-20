//! The UI state the shipping app keeps in its `store.json`. A channel run reads and writes it
//! there, in place, so the update from the Tauri app and a rollback to it both keep it; a
//! sandbox profile keeps the same state in its own database and files.

use std::sync::Arc;

use anlg_desktop_identity::store_json::StoreJson;
use desktop_runtime::{Result, ServiceError};
use serde_json::Value;

/// `apps/desktop/src-tauri/src/store.rs`: the `desktop` scope.
pub const DESKTOP: &str = "desktop";
pub const ONBOARDING_NEEDED: &str = "OnboardingNeeded2";
pub const PINNED_TABS: &str = "PinnedTabs";
pub const RECENTLY_OPENED_SESSIONS: &str = "RecentlyOpenedSessions";
pub const DISMISSED_TOASTS: &str = "DismissedToasts";
/// `plugins/updater2/src/store.rs`.
pub const UPDATER: &str = "updater2";
pub const LAST_SEEN_VERSION: &str = "LastSeenVersion";
/// `plugins/tray/src/ext.rs`.
pub const TRAY: &str = "tray";
pub const SHOW_EVENTS_IN_MENU_BAR: &str = "show_events_in_menu_bar";

/// The channel run's store; absent in a sandbox profile.
#[derive(Clone, Default)]
pub struct ShippingStore(pub Option<Arc<StoreJson>>);

impl gpui::Global for ShippingStore {}

impl ShippingStore {
    pub fn get(cx: &gpui::App) -> Option<Arc<StoreJson>> {
        cx.try_global::<Self>().and_then(|store| store.0.clone())
    }
}

pub async fn read(
    store: Arc<StoreJson>,
    scope: &'static str,
    key: &'static str,
) -> Result<Option<Value>> {
    blocking(move || store.get(scope, key)).await
}

pub async fn write(
    store: Arc<StoreJson>,
    scope: &'static str,
    key: &'static str,
    value: Value,
) -> Result<()> {
    blocking(move || store.set(scope, key, value)).await
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> std::io::Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| ServiceError::Failed(error.to_string().into()))?
        .map_err(|error| ServiceError::Failed(format!("store.json: {error}").into()))
}
