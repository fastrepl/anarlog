use std::path::PathBuf;
use std::sync::Arc;

use hypr_db_core2::Db3;

pub async fn open_desktop_db(_identifier: &str) -> Arc<Db3> {
    #[cfg(debug_assertions)]
    let db_path: Option<PathBuf> = None;

    #[cfg(not(debug_assertions))]
    let db_path: Option<PathBuf> = {
        let data_dir = dirs::data_dir()
            .expect("data_dir must be available")
            .join(_identifier);
        std::fs::create_dir_all(&data_dir).expect("failed to create app data dir");
        Some(data_dir.join("app.db"))
    };

    Arc::new(
        tauri_plugin_db::open_app_db(db_path.as_deref())
            .await
            .expect("failed to open app database"),
    )
}
