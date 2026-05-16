use std::sync::Arc;

use hypr_db_core::Db;

pub async fn open_desktop_db(identifier: &str) -> Arc<Db> {
    let base = dirs::data_dir().expect("data_dir must be available");

    let db_path = match identifier {
        "com.hyprnote.dev" => None,
        "com.hyprnote.stable" => Some(base.join("hyprnote")),
        _ => Some(base.join(identifier)),
    }
    .map(|dir| {
        std::fs::create_dir_all(&dir).expect("failed to create app data dir");
        dir.join("app.db")
    });

    let db = tauri_plugin_db::open_app_db(db_path.as_deref())
        .await
        .expect("failed to open app database");

    Arc::new(db)
}
