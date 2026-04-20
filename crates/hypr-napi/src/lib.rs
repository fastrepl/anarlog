//! Node.js SDK for `hypr-api` via napi-rs.
//!
//! Exposes the generic data-access primitives that `hypr-api` owns:
//! - [`init`] — allocate the singleton [`hypr_api::AppState`].
//! - `execute` / `execute_proxy` — generic SQL entry points (see `db` module).
//!   `execute_proxy` is what `@hypr/db`'s drizzle proxy client binds to on
//!   the renderer side.
//! - `subscribe` — reactive watch primitive. One napi handle per query key;
//!   fanout to renderer windows is the Electron main process's job (see
//!   `apps/desktop2/electron/src/subscription-manager.ts`).
//!
//! Everything domain-specific (sessions, daily notes, tasks, …) is intentionally
//! absent. Typed access is the renderer's concern via drizzle + `@hypr/db`.

#![deny(clippy::all)]

mod db;
mod error;
mod live;
mod state;
mod subscription;

use napi_derive::napi;

/// Initialize the singleton [`hypr_api::AppState`]. Must be awaited before
/// any other SDK call; subsequent calls replace the previous state (useful
/// for tests / forced re-init).
#[napi]
pub async fn init() -> napi::Result<()> {
    let app_state = hypr_api::AppState::open()
        .await
        .map_err(error::to_napi_error)?;

    *state::state_slot().write().await = Some(app_state);
    Ok(())
}
