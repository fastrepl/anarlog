use crate::AppExt;
use crate::embedded_cli::EmbeddedCliStatus;
use crate::ext::OnboardingSurveyState;

#[tauri::command]
#[specta::specta]
pub async fn get_onboarding_needed<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<bool, String> {
    app.get_onboarding_needed().map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_onboarding_needed<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    v: bool,
) -> Result<(), String> {
    app.set_onboarding_needed(v).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_dismissed_toasts<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<String>, String> {
    app.get_dismissed_toasts()
}

#[tauri::command]
#[specta::specta]
pub async fn set_dismissed_toasts<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    v: Vec<String>,
) -> Result<(), String> {
    app.set_dismissed_toasts(v)
}

#[tauri::command]
#[specta::specta]
pub async fn get_env<R: tauri::Runtime>(_app: tauri::AppHandle<R>, key: String) -> String {
    std::env::var(&key).unwrap_or_default()
}

#[tauri::command]
#[specta::specta]
pub fn show_devtool() -> bool {
    if cfg!(debug_assertions) {
        return true;
    }

    #[cfg(feature = "devtools")]
    {
        return true;
    }

    #[cfg(not(feature = "devtools"))]
    {
        false
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_tinybase_values<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<String>, String> {
    app.get_tinybase_values()
}

#[tauri::command]
#[specta::specta]
pub async fn set_tinybase_values<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    v: String,
) -> Result<(), String> {
    app.set_tinybase_values(v)
}

#[tauri::command]
#[specta::specta]
pub async fn get_pinned_tabs<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<String>, String> {
    app.get_pinned_tabs()
}

#[tauri::command]
#[specta::specta]
pub async fn set_pinned_tabs<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    v: String,
) -> Result<(), String> {
    app.set_pinned_tabs(v)
}

#[tauri::command]
#[specta::specta]
pub async fn get_recently_opened_sessions<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<String>, String> {
    app.get_recently_opened_sessions()
}

#[tauri::command]
#[specta::specta]
pub async fn set_recently_opened_sessions<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    v: String,
) -> Result<(), String> {
    app.set_recently_opened_sessions(v)
}

#[tauri::command]
#[specta::specta]
pub async fn get_onboarding_survey_state<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<OnboardingSurveyState, String> {
    app.get_onboarding_survey_state()
}

#[tauri::command]
#[specta::specta]
pub async fn record_onboarding_survey_launch<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<OnboardingSurveyState, String> {
    app.record_onboarding_survey_launch()
}

#[tauri::command]
#[specta::specta]
pub async fn finish_onboarding_survey<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<OnboardingSurveyState, String> {
    app.finish_onboarding_survey()
}

#[tauri::command]
#[specta::specta]
pub async fn reset_onboarding_survey<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<OnboardingSurveyState, String> {
    app.reset_onboarding_survey()
}

#[tauri::command]
#[specta::specta]
pub async fn get_char_v1p1_preview<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<bool, String> {
    app.get_char_v1p1_preview()
}

#[tauri::command]
#[specta::specta]
pub async fn set_char_v1p1_preview<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    v: bool,
) -> Result<(), String> {
    app.set_char_v1p1_preview(v)
}

#[tauri::command]
#[specta::specta]
pub async fn check_embedded_cli<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<EmbeddedCliStatus, String> {
    Ok(crate::embedded_cli::check(&app))
}

#[tauri::command]
#[specta::specta]
pub async fn install_embedded_cli<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<EmbeddedCliStatus, String> {
    crate::embedded_cli::install(&app)
}

#[tauri::command]
#[specta::specta]
pub async fn uninstall_embedded_cli<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<EmbeddedCliStatus, String> {
    crate::embedded_cli::uninstall(&app)
}
