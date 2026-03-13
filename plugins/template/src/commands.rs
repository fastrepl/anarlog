use crate::TemplatePluginExt;

#[tauri::command]
#[specta::specta]
pub async fn render<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    tpl: hypr_template_app::Template,
) -> Result<String, String> {
    hypr_template_app::render(tpl).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn render_custom<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    template_content: String,
    ctx: serde_json::Map<String, serde_json::Value>,
) -> Result<String, String> {
    app.template().render_custom(&template_content, ctx)
}

#[tauri::command]
#[specta::specta]
pub async fn render_support<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    tpl: hypr_template_support::SupportTemplate,
) -> Result<String, String> {
    hypr_template_support::render(tpl).map_err(|e| e.to_string())
}
