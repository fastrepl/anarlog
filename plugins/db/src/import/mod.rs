mod templates;

use std::path::PathBuf;

use sqlx::SqlitePool;

use templates::import_legacy_templates_from_path;

const TEMPLATES_FILENAME: &str = "templates.json";

pub async fn import_legacy_templates<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let vault_base = resolve_startup_vault_base(app)?;
    import_legacy_templates_from_path(pool, &vault_base.join(TEMPLATES_FILENAME)).await
}

fn resolve_startup_vault_base<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> crate::Result<PathBuf> {
    let bundle_id: &str = app.config().identifier.as_ref();
    let settings_base = hypr_storage::global::compute_default_base(bundle_id)
        .ok_or(std::io::Error::other("settings base unavailable"))?;
    std::fs::create_dir_all(&settings_base)?;

    Ok(hypr_storage::vault::resolve_base(
        &settings_base,
        &settings_base,
    ))
}
