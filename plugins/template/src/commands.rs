use crate::TemplatePluginExt;
use std::str::FromStr;
use tauri::Manager;

#[derive(serde::Deserialize, specta::Type)]
pub struct DominantLanguageRequest {
    pub texts: Vec<String>,
    pub candidates: Vec<String>,
}

#[tauri::command]
#[specta::specta]
pub fn summary_length_policy(
    request: anlg_summary::SummaryLengthPolicyRequest,
) -> Result<Option<anlg_summary::SummaryLengthPolicy>, String> {
    Ok(anlg_summary::summary_length_policy_for_texts(
        &request.transcript_texts,
        request.mode,
        request.template_section_count as usize,
    ))
}

#[tauri::command]
#[specta::specta]
pub fn dominant_language(request: DominantLanguageRequest) -> Result<Option<String>, String> {
    let Ok(languages) = request
        .candidates
        .iter()
        .map(|candidate| anlg_language::Language::from_str(candidate))
        .collect::<Result<Vec<_>, _>>()
    else {
        return Ok(None);
    };
    let candidate_codes = languages
        .iter()
        .map(anlg_language::Language::iso639)
        .collect::<Vec<_>>();
    let dominant = anlg_language::dominant_language(
        request.texts.iter().map(String::as_str),
        &candidate_codes,
    );

    Ok(dominant
        .and_then(|dominant| {
            languages
                .iter()
                .position(|candidate| candidate.iso639() == dominant)
        })
        .map(|index| request.candidates[index].clone()))
}

#[tauri::command]
#[specta::specta]
pub fn prepare_generated_summary(
    request: anlg_summary::PrepareGeneratedSummaryRequest,
) -> Result<Option<anlg_summary::PreparedGeneratedSummary>, String> {
    Ok(anlg_summary::prepare_generated_summary(request))
}

#[tauri::command]
#[specta::specta]
pub fn compose_generated_summary(
    request: anlg_summary::ComposeGeneratedSummaryRequest,
) -> Result<String, String> {
    Ok(anlg_summary::compose_generated_summary(request))
}

#[tauri::command]
#[specta::specta]
pub async fn save_generated_summary<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: crate::generated_summary::SaveGeneratedSummaryRequest,
) -> Result<(), String> {
    let runtime = app
        .try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.inner().clone())
        .ok_or_else(|| "database is not ready yet".to_string())?;
    let _guard = runtime.synced_write_guard().await;
    crate::generated_summary::save_generated_summary(runtime.pool(), request).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_generated_title<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: crate::session_content::SaveGeneratedTitleRequest,
) -> Result<(), String> {
    let runtime = app
        .try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.inner().clone())
        .ok_or_else(|| "database is not ready yet".to_string())?;
    let _guard = runtime.synced_write_guard().await;
    crate::session_content::save_generated_title(runtime.pool(), request).await
}

#[tauri::command]
#[specta::specta]
pub async fn apply_session_content_corrections<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: crate::session_content::SessionContentCorrectionsRequest,
) -> Result<(), String> {
    let runtime = app
        .try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.inner().clone())
        .ok_or_else(|| "database is not ready yet".to_string())?;
    let _guard = runtime.synced_write_guard().await;
    crate::session_content::apply_session_content_corrections(runtime.pool(), request).await
}

#[tauri::command]
#[specta::specta]
pub async fn render<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    tpl: anlg_template_app::Template,
) -> Result<String, String> {
    anlg_template_app::render(tpl).map_err(|e| e.to_string())
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
pub async fn get_template_source<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    template: anlg_template_app::EditableTemplate,
) -> Result<String, String> {
    Ok(anlg_template_app::template_source(template).to_string())
}
