use std::sync::Arc;

use hypr_db_core::Db;

const DEV_BUNDLE_ID: &str = "com.hyprnote.dev";
const DB_FILENAME: &str = "app.db";

pub async fn open_desktop_db(identifier: &str) -> Arc<Db> {
    let db_path = desktop_db_dir(identifier).map(|dir| {
        std::fs::create_dir_all(&dir).expect("failed to create app data dir");
        dir.join(DB_FILENAME)
    });

    let db = tauri_plugin_db::open_app_db(db_path.as_deref())
        .await
        .expect("failed to open app database");

    Arc::new(db)
}

fn desktop_db_dir(identifier: &str) -> Option<std::path::PathBuf> {
    if identifier == DEV_BUNDLE_ID {
        return None;
    }

    let data_dir = dirs::data_dir().expect("data_dir must be available");
    let default_dir =
        hypr_storage::global::compute_default_base(identifier).expect("data_dir must be available");
    let identifier_dir = data_dir.join(identifier);

    if identifier_dir.join(DB_FILENAME).is_file() && !default_dir.join(DB_FILENAME).is_file() {
        Some(identifier_dir)
    } else {
        Some(default_dir)
    }
}
