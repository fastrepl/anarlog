use crate::AgentPluginExt;

#[tauri::command]
#[specta::specta]
pub fn health_check<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<hypr_agent_core::HealthCheckResponse, String> {
    Ok(app.agent().health_check())
}

#[tauri::command]
#[specta::specta]
pub fn install_cli<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    payload: hypr_agent_core::InstallCliRequest,
) -> Result<hypr_agent_core::InstallCliResponse, String> {
    app.agent().install_cli(payload)
}

#[tauri::command]
#[specta::specta]
pub fn uninstall_cli<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    payload: hypr_agent_core::UninstallCliRequest,
) -> Result<hypr_agent_core::UninstallCliResponse, String> {
    app.agent().uninstall_cli(payload)
}
