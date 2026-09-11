use tauri::ipc::Channel;

use crate::{ExecuteProxyResult, ManagedState, QueryEvent, TransactionStatement};
use anlg_desktop_db_runtime::cloudsync_config::{
    E2EE_SECRET_READ_TIMEOUT, E2EE_SECRET_SCOPE, E2eeSecretReader, E2eeSecretWriter,
    canonical_e2ee_account_user_id, e2ee_recovery_key_name,
    load_e2ee_recovery_key as load_e2ee_recovery_key_with_secrets, read_e2ee_secret_with_timeout,
};
#[cfg(test)]
use anlg_desktop_db_runtime::cloudsync_config::{
    E2EE_SECRET_READ_TIMEOUT_ERROR, open_shared_workspace_keyrings, open_workspace_e2ee_source_key,
    seal_workspace_e2ee_key,
};

static E2EE_DEVICE_IDENTITY_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn canonical_e2ee_request_id(request_id: &str) -> Result<String, String> {
    uuid::Uuid::parse_str(request_id.trim())
        .map(|request_id| request_id.to_string())
        .map_err(|_| "E2EE enrollment request ID is invalid".to_string())
}

fn e2ee_device_key_name(account_user_id: &str) -> Result<String, String> {
    let account_user_id = canonical_e2ee_account_user_id(account_user_id)?;
    Ok(format!("account:{account_user_id}:device-enrollment-v1"))
}

struct TauriE2eeSecrets<R: tauri::Runtime>(tauri::AppHandle<R>);

impl<R: tauri::Runtime> E2eeSecretReader for TauriE2eeSecrets<R> {
    fn read(
        &self,
        scope: &str,
        key: &str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Option<String>, String>> + Send + '_>,
    > {
        Box::pin(tauri_plugin_store2::read_secret(
            self.0.clone(),
            scope.to_string(),
            key.to_string(),
        ))
    }
}

impl<R: tauri::Runtime> E2eeSecretWriter for TauriE2eeSecrets<R> {
    fn write(
        &self,
        scope: &str,
        key: &str,
        value: &str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(tauri_plugin_store2::write_secret(
            self.0.clone(),
            scope.to_string(),
            key.to_string(),
            value.to_string(),
        ))
    }
}

async fn load_e2ee_recovery_key<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    account_user_id: &str,
) -> Result<Option<anlg_e2ee::RecoveryKey>, String> {
    load_e2ee_recovery_key_with_secrets(&TauriE2eeSecrets(app), account_user_id).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_meetings(
    state: tauri::State<'_, ManagedState>,
    input: anlg_agent_access::ListMeetingsInput,
) -> Result<anlg_agent_access::MeetingPage, String> {
    anlg_agent_access::list_meetings(state.pool(), input)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_meeting(
    state: tauri::State<'_, ManagedState>,
    input: anlg_agent_access::GetMeetingInput,
) -> Result<anlg_agent_access::Meeting, String> {
    anlg_agent_access::get_meeting(state.pool(), input)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_meeting_transcript(
    state: tauri::State<'_, ManagedState>,
    input: anlg_agent_access::GetMeetingTranscriptInput,
) -> Result<anlg_agent_access::TranscriptPage, String> {
    anlg_agent_access::get_meeting_transcript(state.pool(), input)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_recurring_meeting_history(
    state: tauri::State<'_, ManagedState>,
    input: anlg_agent_access::GetRecurringMeetingHistoryInput,
) -> Result<anlg_agent_access::MeetingPage, String> {
    anlg_agent_access::get_recurring_meeting_history(state.pool(), input)
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
        .parse::<anlg_db_execute::ProxyQueryMethod>()
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
    let _write_guard = state.synced_write_guard().await;
    crate::import::rerun_legacy_import(state.pool(), dry_run)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn apply_session_ingest(
    state: tauri::State<'_, ManagedState>,
    workspace_id: String,
    envelope: serde_json::Value,
) -> Result<crate::SessionIngestApplyResult, String> {
    let envelope = match serde_json::from_value(envelope) {
        Ok(envelope) => envelope,
        Err(error) => {
            tracing::warn!(%workspace_id, %error, "rejected malformed session ingest envelope");
            return Ok(crate::SessionIngestApplyResult::Rejected);
        }
    };
    match anlg_session_ingest::apply_session_envelope(state.pool(), &workspace_id, &envelope).await
    {
        Ok(outcome) => Ok(match outcome {
            anlg_session_ingest::ApplyOutcome::Applied => crate::SessionIngestApplyResult::Applied,
            anlg_session_ingest::ApplyOutcome::AlreadyApplied => {
                crate::SessionIngestApplyResult::AlreadyApplied
            }
        }),
        Err(error) if error.is_retryable() => Err(error.to_string()),
        Err(error) => {
            tracing::warn!(%workspace_id, %error, "rejected permanent session ingest envelope");
            Ok(crate::SessionIngestApplyResult::Rejected)
        }
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_e2ee_identity_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    account_user_id: String,
) -> Result<crate::E2eeIdentityStatus, String> {
    let recovery_key = load_e2ee_recovery_key(app, &account_user_id).await?;
    let (key_id, member_public_key) = match recovery_key {
        Some(recovery_key) => (
            Some(recovery_key.key_id()),
            Some(
                recovery_key
                    .member_identity_key()
                    .map_err(|error| error.to_string())?
                    .public_key(),
            ),
        ),
        None => (None, None),
    };
    Ok(crate::E2eeIdentityStatus {
        configured: key_id.is_some(),
        key_id,
        member_public_key,
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) fn inspect_e2ee_recovery_key(
    recovery_key: String,
) -> Result<crate::E2eeRecoveryKeyIdentity, String> {
    let key_id =
        anlg_desktop_db_runtime::cloudsync_config::inspect_e2ee_recovery_key(&recovery_key)?;
    Ok(crate::E2eeRecoveryKeyIdentity { key_id })
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

    anlg_desktop_db_runtime::cloudsync_config::create_e2ee_recovery_code()
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn import_e2ee_identity<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    account_user_id: String,
    recovery_key: String,
) -> Result<(), String> {
    anlg_desktop_db_runtime::cloudsync_config::import_e2ee_recovery_key(
        &TauriE2eeSecrets(app),
        &account_user_id,
        &recovery_key,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_or_create_e2ee_device_identity<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    account_user_id: String,
) -> Result<crate::E2eeDeviceIdentity, String> {
    let key_name = e2ee_device_key_name(&account_user_id)?;
    let _identity_guard = E2EE_DEVICE_IDENTITY_LOCK.lock().await;
    let existing = read_e2ee_secret_with_timeout(
        E2EE_SECRET_READ_TIMEOUT,
        tauri_plugin_store2::read_secret(
            app.clone(),
            E2EE_SECRET_SCOPE.to_string(),
            key_name.clone(),
        ),
    )
    .await?;
    let key = match existing {
        Some(value) => {
            anlg_e2ee::DeviceEnrollmentKey::parse(&value).map_err(|error| error.to_string())?
        }
        None => {
            let key =
                anlg_e2ee::DeviceEnrollmentKey::generate().map_err(|error| error.to_string())?;
            tauri_plugin_store2::write_secret(
                app,
                E2EE_SECRET_SCOPE.to_string(),
                key_name,
                key.expose_code().to_string(),
            )
            .await?;
            key
        }
    };
    Ok(crate::E2eeDeviceIdentity {
        public_key: key.public_key(),
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn seal_e2ee_recovery_key_for_device<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    account_user_id: String,
    request_id: String,
    recipient_public_key: String,
) -> Result<crate::E2eeDeviceEnrollmentPackage, String> {
    let account_user_id = canonical_e2ee_account_user_id(&account_user_id)?;
    let request_id = canonical_e2ee_request_id(&request_id)?;
    let recovery_key = load_e2ee_recovery_key(app, &account_user_id)
        .await?
        .ok_or_else(|| "E2EE recovery key is not configured".to_string())?;
    anlg_e2ee::seal_recovery_key_for_device(
        &recovery_key,
        &recipient_public_key,
        &account_user_id,
        &request_id,
    )
    .map(Into::into)
    .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn seal_workspace_e2ee_key_for_recipients<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, ManagedState>,
    account_user_id: String,
    workspace_id: String,
    recipients: Vec<crate::WorkspaceE2eeKeyRecipient>,
    rotate: bool,
    source_grant: Option<crate::CloudsyncWorkspaceKeyGrant>,
) -> Result<crate::SealedWorkspaceE2eeKey, String> {
    state
        .seal_workspace_e2ee_key_for_recipients(
            &TauriE2eeSecrets(app),
            &account_user_id,
            &workspace_id,
            recipients,
            rotate,
            source_grant,
        )
        .await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn import_e2ee_device_enrollment<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    account_user_id: String,
    request_id: String,
    package: crate::E2eeDeviceEnrollmentPackage,
) -> Result<crate::E2eeRecoveryKeyIdentity, String> {
    let account_user_id = canonical_e2ee_account_user_id(&account_user_id)?;
    let request_id = canonical_e2ee_request_id(&request_id)?;
    if load_e2ee_recovery_key(app.clone(), &account_user_id)
        .await?
        .is_some()
    {
        return Err("E2EE recovery key is already configured".to_string());
    }
    let key_name = e2ee_device_key_name(&account_user_id)?;
    let device_key = read_e2ee_secret_with_timeout(
        E2EE_SECRET_READ_TIMEOUT,
        tauri_plugin_store2::read_secret(app.clone(), E2EE_SECRET_SCOPE.to_string(), key_name),
    )
    .await?
    .ok_or_else(|| "E2EE device identity is not configured".to_string())?;
    let device_key =
        anlg_e2ee::DeviceEnrollmentKey::parse(&device_key).map_err(|error| error.to_string())?;
    let recovery_key = device_key
        .open_recovery_key(&account_user_id, &request_id, &package.clone().into())
        .map_err(|error| error.to_string())?;
    let key_id = recovery_key.key_id();
    tauri_plugin_store2::write_secret(
        app,
        E2EE_SECRET_SCOPE.to_string(),
        e2ee_recovery_key_name(&account_user_id)?,
        recovery_key.expose_code().to_string(),
    )
    .await?;
    Ok(crate::E2eeRecoveryKeyIdentity { key_id })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn subscribe(
    state: tauri::State<'_, ManagedState>,
    sql: String,
    params: Vec<serde_json::Value>,
    on_event: Channel<QueryEvent>,
) -> Result<anlg_db_reactive::SubscriptionRegistration, String> {
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
    let result = state
        .configure_cloudsync(config_json)
        .await
        .map_err(|error| error.to_string());
    state.record_cloudsync_configuration_result("configure", &result);
    result
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
    workspace_key_grants: Option<Vec<crate::CloudsyncWorkspaceKeyGrant>>,
    e2ee_witness: crate::CloudsyncE2eeWitness,
) -> Result<crate::CloudsyncTokenConfigurationResult, String> {
    state
        .configure_cloudsync_token_with_keys(
            &TauriE2eeSecrets(app),
            database_id,
            token,
            workspace_id,
            workspace_projection,
            workspace_key_grants,
            e2ee_witness,
        )
        .await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn configure_e2ee_replica<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, ManagedState>,
    workspace_id: String,
    e2ee_witness: crate::CloudsyncE2eeWitness,
    workspace_projection: Option<crate::CloudsyncWorkspaceProjection>,
    workspace_key_grants: Option<Vec<crate::CloudsyncWorkspaceKeyGrant>>,
) -> Result<crate::CloudsyncTokenConfigurationResult, String> {
    state
        .configure_e2ee_replica_with_keys(
            &TauriE2eeSecrets(app),
            workspace_id,
            e2ee_witness,
            workspace_projection,
            workspace_key_grants,
        )
        .await
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
    let result = state
        .start_cloudsync()
        .await
        .map_err(|error| error.to_string());
    state.record_cloudsync_configuration_result("start", &result);
    result
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
pub(crate) async fn suspend_cloudsync_for_sign_out(
    state: tauri::State<'_, ManagedState>,
) -> Result<(), String> {
    state
        .suspend_cloudsync_for_sign_out()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn suspend_cloudsync_after_auth_loss(
    state: tauri::State<'_, ManagedState>,
) -> Result<(), String> {
    state
        .suspend_cloudsync_after_auth_loss()
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

#[tauri::command]
#[specta::specta]
pub(crate) async fn begin_cloudsync_activity(
    state: tauri::State<'_, ManagedState>,
    activity: String,
    key: String,
) -> Result<(), String> {
    state
        .begin_cloudsync_activity(activity, key)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn end_cloudsync_activity(
    state: tauri::State<'_, ManagedState>,
    activity: String,
    key: String,
) -> Result<(), String> {
    state
        .end_cloudsync_activity(activity, key)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn get_startup_status(state: tauri::State<'_, ManagedState>) -> crate::StartupStatus {
    state.startup_status()
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn wait_until_ready(state: tauri::State<'_, ManagedState>) -> Result<(), String> {
    state
        .wait_until_ready()
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant(
        recovery_key: &anlg_e2ee::RecoveryKey,
        workspace_id: &str,
        account_user_id: &str,
        key: &anlg_e2ee::WorkspaceKey,
        is_active: bool,
    ) -> crate::CloudsyncWorkspaceKeyGrant {
        let sealed = anlg_e2ee::seal_workspace_key_for_member(
            key,
            &recovery_key.member_identity_key().unwrap().public_key(),
            workspace_id,
            account_user_id,
        )
        .unwrap();
        crate::CloudsyncWorkspaceKeyGrant {
            workspace_id: workspace_id.to_string(),
            key_id: sealed.key_id,
            ephemeral_public_key: sealed.ephemeral_public_key,
            nonce: sealed.nonce,
            ciphertext: sealed.ciphertext,
            is_active,
        }
    }

    #[tokio::test]
    async fn e2ee_secret_read_timeout_is_bounded() {
        let error = read_e2ee_secret_with_timeout(
            std::time::Duration::ZERO,
            std::future::pending::<Result<Option<String>, String>>(),
        )
        .await
        .unwrap_err();

        assert_eq!(error, E2EE_SECRET_READ_TIMEOUT_ERROR);
    }

    #[test]
    fn enrollment_ids_are_canonicalized_before_use() {
        assert_eq!(
            canonical_e2ee_account_user_id(" 550E8400-E29B-41D4-A716-446655440000 ").unwrap(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
        assert_eq!(
            canonical_e2ee_request_id(" 6BA7B810-9DAD-11D1-80B4-00C04FD430C8 ").unwrap(),
            "6ba7b810-9dad-11d1-80b4-00c04fd430c8"
        );
    }

    #[test]
    fn opens_active_and_retired_shared_workspace_grants() {
        let recovery_key = anlg_e2ee::RecoveryKey::generate().unwrap();
        let retired = anlg_e2ee::WorkspaceKey::generate().unwrap();
        let active = anlg_e2ee::WorkspaceKey::generate().unwrap();
        let retired_key_id = retired.key_id().to_string();
        let active_key_id = active.key_id().to_string();

        let keyrings = open_shared_workspace_keyrings(
            &recovery_key,
            "user-a",
            std::collections::HashSet::from(["workspace-shared".to_string()]),
            vec![
                grant(&recovery_key, "workspace-shared", "user-a", &retired, false),
                grant(&recovery_key, "workspace-shared", "user-a", &active, true),
            ],
        )
        .unwrap();

        let keyring = &keyrings["workspace-shared"];
        assert_eq!(keyring.active().key_id(), active_key_id);
        assert!(keyring.get(&retired_key_id).is_some());
    }

    #[test]
    fn shared_workspace_grants_fail_closed() {
        let recovery_key = anlg_e2ee::RecoveryKey::generate().unwrap();
        let key = anlg_e2ee::WorkspaceKey::generate().unwrap();
        let workspace_ids = std::collections::HashSet::from(["workspace-shared".to_string()]);

        assert!(
            open_shared_workspace_keyrings(
                &recovery_key,
                "user-a",
                workspace_ids.clone(),
                Vec::new(),
            )
            .is_err()
        );
        assert!(
            open_shared_workspace_keyrings(
                &recovery_key,
                "user-a",
                workspace_ids.clone(),
                vec![grant(
                    &recovery_key,
                    "workspace-shared",
                    "user-a",
                    &key,
                    false,
                )],
            )
            .is_err()
        );
        assert!(
            open_shared_workspace_keyrings(
                &recovery_key,
                "user-b",
                workspace_ids,
                vec![grant(
                    &recovery_key,
                    "workspace-shared",
                    "user-a",
                    &key,
                    true,
                )],
            )
            .is_err()
        );
    }

    #[test]
    fn seals_one_workspace_key_for_each_recipient_identity() {
        const OWNER: &str = "11111111-1111-4111-8111-111111111111";
        const MEMBER: &str = "22222222-2222-4222-8222-222222222222";
        const WORKSPACE: &str = "33333333-3333-4333-8333-333333333333";
        let owner_recovery = anlg_e2ee::RecoveryKey::generate().unwrap();
        let member_recovery = anlg_e2ee::RecoveryKey::generate().unwrap();
        let key = anlg_e2ee::WorkspaceKey::generate().unwrap();
        let expected_key_id = key.key_id().to_string();

        let sealed = seal_workspace_e2ee_key(
            key,
            OWNER,
            WORKSPACE,
            vec![
                crate::WorkspaceE2eeKeyRecipient {
                    user_id: OWNER.to_string(),
                    public_key: owner_recovery.member_identity_key().unwrap().public_key(),
                },
                crate::WorkspaceE2eeKeyRecipient {
                    user_id: MEMBER.to_string(),
                    public_key: member_recovery.member_identity_key().unwrap().public_key(),
                },
            ],
        )
        .unwrap();

        assert_eq!(sealed.key_id, expected_key_id);
        for (recovery_key, user_id, upload) in [
            (&owner_recovery, OWNER, &sealed.grants[0]),
            (&member_recovery, MEMBER, &sealed.grants[1]),
        ] {
            let opened = recovery_key
                .member_identity_key()
                .unwrap()
                .open_workspace_key(
                    WORKSPACE,
                    user_id,
                    &anlg_e2ee::WorkspaceKeyGrant {
                        key_id: sealed.key_id.clone(),
                        ephemeral_public_key: upload.ephemeral_public_key.clone(),
                        nonce: upload.nonce.clone(),
                        ciphertext: upload.ciphertext.clone(),
                    },
                )
                .unwrap();
            assert_eq!(opened.key_id(), expected_key_id);
        }
    }

    #[test]
    fn sealing_workspace_keys_requires_the_issuer_and_unique_recipients() {
        const OWNER: &str = "11111111-1111-4111-8111-111111111111";
        const MEMBER: &str = "22222222-2222-4222-8222-222222222222";
        const WORKSPACE: &str = "33333333-3333-4333-8333-333333333333";
        let recovery_key = anlg_e2ee::RecoveryKey::generate().unwrap();
        let recipient = crate::WorkspaceE2eeKeyRecipient {
            user_id: MEMBER.to_string(),
            public_key: recovery_key.member_identity_key().unwrap().public_key(),
        };

        assert!(
            seal_workspace_e2ee_key(
                anlg_e2ee::WorkspaceKey::generate().unwrap(),
                OWNER,
                WORKSPACE,
                vec![recipient.clone()],
            )
            .is_err()
        );
        assert!(
            seal_workspace_e2ee_key(
                anlg_e2ee::WorkspaceKey::generate().unwrap(),
                MEMBER,
                WORKSPACE,
                vec![recipient.clone(), recipient],
            )
            .is_err()
        );
    }

    #[test]
    fn opens_an_active_source_grant_after_runtime_keys_are_lost() {
        const OWNER: &str = "11111111-1111-4111-8111-111111111111";
        const WORKSPACE: &str = "33333333-3333-4333-8333-333333333333";
        let recovery_key = anlg_e2ee::RecoveryKey::generate().unwrap();
        let key = anlg_e2ee::WorkspaceKey::generate().unwrap();
        let expected_key_id = key.key_id().to_string();
        let grant = anlg_e2ee::seal_workspace_key_for_member(
            &key,
            &recovery_key.member_identity_key().unwrap().public_key(),
            WORKSPACE,
            OWNER,
        )
        .unwrap();
        let source_grant = crate::CloudsyncWorkspaceKeyGrant {
            workspace_id: WORKSPACE.to_string(),
            key_id: grant.key_id,
            ephemeral_public_key: grant.ephemeral_public_key,
            nonce: grant.nonce,
            ciphertext: grant.ciphertext,
            is_active: true,
        };

        assert_eq!(
            open_workspace_e2ee_source_key(&recovery_key, OWNER, WORKSPACE, source_grant.clone(),)
                .unwrap()
                .key_id(),
            expected_key_id,
        );
        assert!(
            open_workspace_e2ee_source_key(
                &recovery_key,
                "22222222-2222-4222-8222-222222222222",
                WORKSPACE,
                source_grant,
            )
            .is_err()
        );
    }
}
