use std::collections::HashMap;
use std::ffi::{c_void, CStr};

use sqlx::SqlitePool;
use tokio::sync::broadcast;

mod commands;
mod error;
mod explain;
mod ext;

pub use error::*;
pub use ext::*;

const PLUGIN_NAME: &str = "reactive-db";

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(tag = "event", content = "data")]
pub enum QueryEvent {
    #[serde(rename = "result")]
    Result(Vec<serde_json::Value>),
    #[serde(rename = "error")]
    Error(String),
}

#[derive(Debug, Clone)]
pub(crate) struct TableUpdate {
    pub table: String,
}

#[allow(dead_code)]
pub(crate) struct Subscription {
    pub sql: String,
    pub params: Vec<serde_json::Value>,
    pub tables: std::collections::HashSet<String>,
    pub channel: tauri::ipc::Channel<QueryEvent>,
}

pub struct State {
    pub(crate) pool: Option<SqlitePool>,
    pub(crate) updates_tx: Option<broadcast::Sender<TableUpdate>>,
    pub(crate) subscriptions: HashMap<String, Subscription>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            pool: None,
            updates_tx: None,
            subscriptions: HashMap::new(),
        }
    }
}

pub type ManagedState = tokio::sync::Mutex<State>;

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::subscribe::<tauri::Wry>,
            commands::unsubscribe::<tauri::Wry>,
            commands::execute::<tauri::Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(|app, _api| {
            use tauri::Manager;
            app.manage(ManagedState::default());
            Ok(())
        })
        .build()
}

// ── Query helpers ────────────────────────────────────────────────────────────

pub(crate) async fn execute_query(
    pool: &SqlitePool,
    sql: &str,
    params: &[serde_json::Value],
) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    let mut query = sqlx::query(sql);
    for param in params {
        query = match param {
            serde_json::Value::Null => query.bind(None::<String>),
            serde_json::Value::Bool(b) => query.bind(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    query.bind(i)
                } else {
                    query.bind(n.as_f64().unwrap_or_default())
                }
            }
            serde_json::Value::String(s) => query.bind(s.clone()),
            other => query.bind(other.to_string()),
        };
    }
    let rows = query.fetch_all(pool).await?;
    Ok(rows.iter().map(row_to_json).collect())
}

fn row_to_json(row: &sqlx::sqlite::SqliteRow) -> serde_json::Value {
    use sqlx::{Column, Row, TypeInfo, ValueRef};

    let mut map = serde_json::Map::new();
    for (idx, col) in row.columns().iter().enumerate() {
        let value = match row.try_get_raw(idx) {
            Ok(raw) if !raw.is_null() => match raw.type_info().name() {
                "TEXT" => row
                    .get::<Option<String>, _>(idx)
                    .map(serde_json::Value::String)
                    .unwrap_or(serde_json::Value::Null),
                "INTEGER" | "INT" | "BOOLEAN" => row
                    .get::<Option<i64>, _>(idx)
                    .map(|v| serde_json::json!(v))
                    .unwrap_or(serde_json::Value::Null),
                "REAL" => row
                    .get::<Option<f64>, _>(idx)
                    .map(|v| serde_json::json!(v))
                    .unwrap_or(serde_json::Value::Null),
                "BLOB" => row
                    .get::<Option<Vec<u8>>, _>(idx)
                    .map(|v| serde_json::json!(v))
                    .unwrap_or(serde_json::Value::Null),
                _ => row
                    .get::<Option<String>, _>(idx)
                    .map(serde_json::Value::String)
                    .unwrap_or(serde_json::Value::Null),
            },
            _ => serde_json::Value::Null,
        };
        map.insert(col.name().to_string(), value);
    }
    serde_json::Value::Object(map)
}

// ── C callback for sqlite3_update_hook ──────────────────────────────────────

unsafe extern "C" fn update_hook_callback(
    user_data: *mut c_void,
    op: std::os::raw::c_int,
    _db_name: *const std::os::raw::c_char,
    table_name: *const std::os::raw::c_char,
    _rowid: libsqlite3_sys::sqlite3_int64,
) {
    unsafe {
        let is_mutation = op == libsqlite3_sys::SQLITE_INSERT
            || op == libsqlite3_sys::SQLITE_UPDATE
            || op == libsqlite3_sys::SQLITE_DELETE;
        if !is_mutation {
            return;
        }

        let tx = &*(user_data as *const broadcast::Sender<TableUpdate>);
        let table = CStr::from_ptr(table_name).to_string_lossy().into_owned();
        let _ = tx.send(TableUpdate { table });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder::<tauri::Wry>()
            .export(
                specta_typescript::Typescript::default()
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                OUTPUT_FILE,
            )
            .unwrap();

        let content = std::fs::read_to_string(OUTPUT_FILE).unwrap();
        std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
    }
}
