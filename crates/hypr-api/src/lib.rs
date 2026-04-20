//! Thin orchestration crate for the desktop2 Electron data layer.
//!
//! Mirrors the surface of `plugins/db` (Tauri) so both transports share the
//! same generic primitives — generic SQL execution via `hypr_db_execute` and
//! reactive subscriptions via `hypr_db_reactive`. Domain typing lives in
//! `@hypr/db` (drizzle schemas + drizzle-zod) on the renderer side; Rust
//! intentionally does not hand-roll per-entity CRUD here.
//!
//! The one public surface is:
//! - [`AppState`] — DB + executor + live-query runtime.
//! - [`execute`] / [`execute_proxy`] — generic SQL entry points.
//! - [`live::subscribe`] — generic reactive watch.

#![forbid(unsafe_code)]

mod error;
pub mod live;
mod state;

pub use error::{Error, Result};
pub use hypr_db_execute::{ProxyQueryMethod, ProxyQueryResult};
pub use state::AppState;

/// Run a SQL query and return named-object rows. Matches
/// `plugins/db`'s `execute` command.
pub async fn execute(
    state: &AppState,
    sql: String,
    params: Vec<serde_json::Value>,
) -> Result<Vec<serde_json::Value>> {
    state
        .executor()
        .execute(sql, params)
        .await
        .map_err(Into::into)
}

/// Run a SQL query through the drizzle proxy adapter. Matches
/// `plugins/db`'s `execute_proxy` command; renderer consumes this via
/// `@hypr/db`'s `createDb(client)`.
pub async fn execute_proxy(
    state: &AppState,
    sql: String,
    params: Vec<serde_json::Value>,
    method: ProxyQueryMethod,
) -> Result<ProxyQueryResult> {
    state
        .executor()
        .execute_proxy(sql, params, method)
        .await
        .map_err(Into::into)
}
