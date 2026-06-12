use std::sync::Arc;

use hypr_db_core::Db;

const DEV_BUNDLE_ID: &str = "com.hyprnote.dev";
const DB_FILENAME: &str = "app.db";

pub async fn open_desktop_db(identifier: &str) -> Result<Arc<Db>, String> {
    let db_path = desktop_db_dir(identifier)?.map(|dir| {
        std::fs::create_dir_all(&dir)
            .map(|_| dir.join(DB_FILENAME))
            .map_err(|error| format!("failed to create app data dir: {error}"))
    });
    let db_path = match db_path {
        Some(path) => Some(path?),
        None => None,
    };

    let db = tauri_plugin_db::open_app_db(db_path.as_deref())
        .await
        .map_err(|error| format!("failed to open app database: {error}"))?;

    Ok(Arc::new(db))
}

fn desktop_db_dir(identifier: &str) -> Result<Option<std::path::PathBuf>, String> {
    if identifier == DEV_BUNDLE_ID {
        return Ok(None);
    }

    let data_dir = dirs::data_dir().ok_or_else(|| "data_dir must be available".to_string())?;
    let default_dir = hypr_storage::global::compute_default_base(identifier)
        .ok_or_else(|| "failed to compute default data dir".to_string())?;
    let identifier_dir = data_dir.join(identifier);

    if identifier_dir.join(DB_FILENAME).is_file() && !default_dir.join(DB_FILENAME).is_file() {
        Ok(Some(identifier_dir))
    } else {
        Ok(Some(default_dir))
    }
}
