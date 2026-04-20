use std::sync::OnceLock;

use hypr_api::AppState;

use crate::error::to_napi_error;

pub(crate) fn state_slot() -> &'static tokio::sync::RwLock<Option<AppState>> {
    static SLOT: OnceLock<tokio::sync::RwLock<Option<AppState>>> = OnceLock::new();
    SLOT.get_or_init(|| tokio::sync::RwLock::new(None))
}

pub(crate) async fn require_state() -> napi::Result<AppState> {
    state_slot()
        .read()
        .await
        .clone()
        .ok_or_else(|| to_napi_error("hypr-sdk not initialized"))
}

/// Synchronous variant used by non-async `#[napi]` entry points (e.g. the
/// `subscribe_*` functions that need to build the ThreadsafeFunction on the
/// JS thread before spawning work).
pub(crate) fn require_state_blocking() -> napi::Result<AppState> {
    state_slot()
        .blocking_read()
        .clone()
        .ok_or_else(|| to_napi_error("hypr-sdk not initialized"))
}
