use napi_derive::napi;

use hypr_api::ProxyQueryMethod;

use crate::error::to_napi_error;
use crate::state::require_state;

/// Return value of [`execute_proxy`]. Matches the shape drizzle's
/// `sqlite-proxy` driver expects (positional rows, one array per row).
#[napi(object)]
pub struct ExecuteProxyResult {
    pub rows: Vec<serde_json::Value>,
}

/// Run a SQL query and return named-object rows. Matches `plugin:db|execute`
/// on the Tauri side.
#[napi]
pub async fn execute(
    sql: String,
    params: Vec<serde_json::Value>,
) -> napi::Result<Vec<serde_json::Value>> {
    let state = require_state().await?;
    hypr_api::execute(&state, sql, params)
        .await
        .map_err(to_napi_error)
}

/// Run a SQL query through drizzle's proxy adapter. Matches
/// `plugin:db|execute_proxy` on the Tauri side. `method` is one of
/// `"run" | "all" | "get" | "values"`; any other value is an error.
#[napi]
pub async fn execute_proxy(
    sql: String,
    params: Vec<serde_json::Value>,
    method: String,
) -> napi::Result<ExecuteProxyResult> {
    let method = method.parse::<ProxyQueryMethod>().map_err(to_napi_error)?;
    let state = require_state().await?;
    hypr_api::execute_proxy(&state, sql, params, method)
        .await
        .map(|result| ExecuteProxyResult { rows: result.rows })
        .map_err(to_napi_error)
}
