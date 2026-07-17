use tauri::ipc::Channel;

use crate::{ExecuteProxyResult, ManagedState, QueryEvent, TransactionStatement};

const E2EE_SECRET_SCOPE: &str = "e2ee";

fn e2ee_recovery_key_name(account_user_id: &str) -> Result<String, String> {
    let account_user_id = uuid::Uuid::parse_str(account_user_id.trim())
        .map_err(|_| "E2EE account ID is invalid".to_string())?;
    Ok(format!("account:{account_user_id}:recovery-v1"))
}

async fn load_e2ee_recovery_key<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    account_user_id: &str,
) -> Result<Option<hypr_e2ee::RecoveryKey>, String> {
    let key = e2ee_recovery_key_name(account_user_id)?;
    tauri_plugin_store2::read_secret(app, E2EE_SECRET_SCOPE.to_string(), key)
        .await?
        .map(|value| hypr_e2ee::RecoveryKey::parse(&value).map_err(|error| error.to_string()))
        .transpose()
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_meetings(
    state: tauri::State<'_, ManagedState>,
    input: hypr_agent_access::ListMeetingsInput,
) -> Result<hypr_agent_access::MeetingPage, String> {
    hypr_agent_access::list_meetings(state.pool(), input)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_meeting(
    state: tauri::State<'_, ManagedState>,
    input: hypr_agent_access::GetMeetingInput,
) -> Result<hypr_agent_access::Meeting, String> {
    hypr_agent_access::get_meeting(state.pool(), input)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_meeting_transcript(
    state: tauri::State<'_, ManagedState>,
    input: hypr_agent_access::GetMeetingTranscriptInput,
) -> Result<hypr_agent_access::TranscriptPage, String> {
    hypr_agent_access::get_meeting_transcript(state.pool(), input)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_recurring_meeting_history(
    state: tauri::State<'_, ManagedState>,
    input: hypr_agent_access::GetRecurringMeetingHistoryInput,
) -> Result<hypr_agent_access::MeetingPage, String> {
    hypr_agent_access::get_recurring_meeting_history(state.pool(), input)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn execute(
    state: tauri::State<'_, ManagedState>,
    sql: String,
    params: Vec<serde_json::Value>,
) -> Result<Vec<serde_json::Value>, String> {
    state
        .execute(sql, params)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn execute_transaction(
    state: tauri::State<'_, ManagedState>,
    statements: Vec<TransactionStatement>,
) -> Result<Vec<u64>, String> {
    state
        .execute_transaction(statements)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn execute_proxy(
    state: tauri::State<'_, ManagedState>,
    sql: String,
    params: Vec<serde_json::Value>,
    method: String,
) -> Result<ExecuteProxyResult, String> {
    let method = method
        .parse::<hypr_db_execute::ProxyQueryMethod>()
        .map_err(|error| error.to_string())?;
    state
        .execute_proxy(sql, params, method)
        .await
        .map(|result| ExecuteProxyResult { rows: result.rows })
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_legacy_import_report(
    state: tauri::State<'_, ManagedState>,
) -> Result<crate::LegacyImportReport, String> {
    crate::import::get_legacy_import_report(state.pool())
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_legacy_cleanup_status(
    state: tauri::State<'_, ManagedState>,
) -> Result<crate::LegacyCleanupStatus, String> {
    crate::import::get_legacy_cleanup_status(state.pool())
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn cleanup_legacy_files(
    state: tauri::State<'_, ManagedState>,
) -> Result<crate::LegacyCleanupResult, String> {
    state
        .cleanup_legacy_files()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn run_legacy_import(
    state: tauri::State<'_, ManagedState>,
    dry_run: bool,
) -> Result<String, String> {
    state
        .rerun_legacy_import(dry_run)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_e2ee_identity_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    account_user_id: String,
) -> Result<crate::E2eeIdentityStatus, String> {
    let recovery_key = load_e2ee_recovery_key(app, &account_user_id).await?;
    Ok(crate::E2eeIdentityStatus {
        configured: recovery_key.is_some(),
        key_id: recovery_key.map(|key| key.key_id()),
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) fn inspect_e2ee_recovery_key(
    recovery_key: String,
) -> Result<crate::E2eeRecoveryKeyIdentity, String> {
    let recovery_key =
        hypr_e2ee::RecoveryKey::parse(&recovery_key).map_err(|error| error.to_string())?;
    Ok(crate::E2eeRecoveryKeyIdentity {
        key_id: recovery_key.key_id(),
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn create_e2ee_identity<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    account_user_id: String,
) -> Result<String, String> {
    e2ee_recovery_key_name(&account_user_id)?;
    if load_e2ee_recovery_key(app.clone(), &account_user_id)
        .await?
        .is_some()
    {
        return Err("E2EE recovery key is already configured".to_string());
    }

    let recovery_key = hypr_e2ee::RecoveryKey::generate().map_err(|error| error.to_string())?;
    let recovery_code = recovery_key.expose_code();
    Ok(recovery_code.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn import_e2ee_identity<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    account_user_id: String,
    recovery_key: String,
) -> Result<(), String> {
    let key_name = e2ee_recovery_key_name(&account_user_id)?;
    if load_e2ee_recovery_key(app.clone(), &account_user_id)
        .await?
        .is_some()
    {
        return Err("E2EE recovery key is already configured".to_string());
    }

    let recovery_key =
        hypr_e2ee::RecoveryKey::parse(&recovery_key).map_err(|error| error.to_string())?;
    tauri_plugin_store2::write_secret(
        app,
        E2EE_SECRET_SCOPE.to_string(),
        key_name,
        recovery_key.expose_code().to_string(),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn subscribe(
    state: tauri::State<'_, ManagedState>,
    sql: String,
    params: Vec<serde_json::Value>,
    on_event: Channel<QueryEvent>,
) -> Result<hypr_db_reactive::SubscriptionRegistration, String> {
    state
        .subscribe(
            sql,
            params,
            crate::runtime::QueryEventChannel::new(on_event),
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn unsubscribe(
    state: tauri::State<'_, ManagedState>,
    subscription_id: String,
) -> Result<(), String> {
    state
        .unsubscribe(&subscription_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn configure_cloudsync(
    state: tauri::State<'_, ManagedState>,
    config_json: String,
) -> Result<(), String> {
    state
        .configure_cloudsync(config_json)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn configure_cloudsync_token<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, ManagedState>,
    database_id: String,
    token: String,
    workspace_id: String,
    workspace_projection: Option<crate::CloudsyncWorkspaceProjection>,
) -> Result<crate::CloudsyncTokenConfigurationResult, String> {
    let personal_workspace_id = workspace_projection
        .as_ref()
        .map(|projection| projection.personal_workspace_id.as_str())
        .unwrap_or(workspace_id.as_str());
    let recovery_key = load_e2ee_recovery_key(app, &workspace_id)
        .await?
        .ok_or_else(|| {
            "end-to-end encryption recovery key setup is required before CloudSync can start"
                .to_string()
        })?;
    state
        .set_e2ee_recovery_key(personal_workspace_id, &recovery_key)
        .map_err(|error| error.to_string())?;
    state
        .configure_cloudsync_token_with_projection(
            database_id,
            token,
            workspace_id,
            workspace_projection.map(Into::into),
        )
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn bind_cloudsync_account(
    state: tauri::State<'_, ManagedState>,
    account_user_id: String,
) -> Result<bool, String> {
    state
        .bind_cloudsync_account(account_user_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn start_cloudsync(state: tauri::State<'_, ManagedState>) -> Result<(), String> {
    state
        .start_cloudsync()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn stop_cloudsync(state: tauri::State<'_, ManagedState>) -> Result<(), String> {
    state
        .stop_cloudsync()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn suspend_cloudsync(state: tauri::State<'_, ManagedState>) -> Result<(), String> {
    state
        .suspend_cloudsync()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_cloudsync_status(
    state: tauri::State<'_, ManagedState>,
) -> Result<serde_json::Value, String> {
    state
        .cloudsync_status()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn sync_cloudsync_now(
    state: tauri::State<'_, ManagedState>,
) -> Result<serde_json::Value, String> {
    state
        .sync_cloudsync_now()
        .await
        .map_err(|error| error.to_string())
}
