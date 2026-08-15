use tauri::ipc::Channel;

use crate::{ExecuteProxyResult, ManagedState, QueryEvent, TransactionStatement};

const E2EE_SECRET_SCOPE: &str = "e2ee";
const E2EE_SECRET_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);
const E2EE_SECRET_READ_TIMEOUT_ERROR: &str = "E2EE secret read timed out";
static E2EE_DEVICE_IDENTITY_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn canonical_e2ee_account_user_id(account_user_id: &str) -> Result<String, String> {
    uuid::Uuid::parse_str(account_user_id.trim())
        .map(|account_user_id| account_user_id.to_string())
        .map_err(|_| "E2EE account ID is invalid".to_string())
}

fn canonical_e2ee_request_id(request_id: &str) -> Result<String, String> {
    uuid::Uuid::parse_str(request_id.trim())
        .map(|request_id| request_id.to_string())
        .map_err(|_| "E2EE enrollment request ID is invalid".to_string())
}

fn e2ee_recovery_key_name(account_user_id: &str) -> Result<String, String> {
    let account_user_id = canonical_e2ee_account_user_id(account_user_id)?;
    Ok(format!("account:{account_user_id}:recovery-v1"))
}

fn e2ee_device_key_name(account_user_id: &str) -> Result<String, String> {
    let account_user_id = canonical_e2ee_account_user_id(account_user_id)?;
    Ok(format!("account:{account_user_id}:device-enrollment-v1"))
}

async fn load_e2ee_recovery_key<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    account_user_id: &str,
) -> Result<Option<anlg_e2ee::RecoveryKey>, String> {
    let key = e2ee_recovery_key_name(account_user_id)?;
    read_e2ee_secret_with_timeout(
        E2EE_SECRET_READ_TIMEOUT,
        tauri_plugin_store2::read_secret(app, E2EE_SECRET_SCOPE.to_string(), key),
    )
    .await?
    .map(|value| anlg_e2ee::RecoveryKey::parse(&value).map_err(|error| error.to_string()))
    .transpose()
}

async fn read_e2ee_secret_with_timeout(
    timeout: std::time::Duration,
    read: impl std::future::Future<Output = Result<Option<String>, String>>,
) -> Result<Option<String>, String> {
    tokio::time::timeout(timeout, read)
        .await
        .map_err(|_| E2EE_SECRET_READ_TIMEOUT_ERROR.to_string())?
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
    state
        .rerun_legacy_import(dry_run)
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
    let recovery_key =
        anlg_e2ee::RecoveryKey::parse(&recovery_key).map_err(|error| error.to_string())?;
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

    let recovery_key = anlg_e2ee::RecoveryKey::generate().map_err(|error| error.to_string())?;
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
        anlg_e2ee::RecoveryKey::parse(&recovery_key).map_err(|error| error.to_string())?;
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
    let account_user_id = canonical_e2ee_account_user_id(&account_user_id)?;
    let workspace_id = uuid::Uuid::parse_str(workspace_id.trim())
        .map(|workspace_id| workspace_id.to_string())
        .map_err(|_| "E2EE workspace ID is invalid".to_string())?;
    if recipients.is_empty() || recipients.len() > 256 {
        return Err("workspace E2EE recipients are invalid".to_string());
    }

    let key = if rotate {
        anlg_e2ee::WorkspaceKey::generate().map_err(|error| error.to_string())
    } else {
        match state.workspace_key(&workspace_id) {
            Some(key) => Ok(key),
            None => {
                let source_grant = source_grant
                    .ok_or_else(|| "workspace E2EE source grant is invalid".to_string())?;
                let recovery_key = load_e2ee_recovery_key(app, &account_user_id)
                    .await?
                    .ok_or_else(|| "E2EE recovery key is not configured".to_string())?;
                open_workspace_e2ee_source_key(
                    &recovery_key,
                    &account_user_id,
                    &workspace_id,
                    source_grant,
                )
            }
        }
    }?;
    seal_workspace_e2ee_key(key, &account_user_id, &workspace_id, recipients)
}

fn open_workspace_e2ee_source_key(
    recovery_key: &anlg_e2ee::RecoveryKey,
    account_user_id: &str,
    workspace_id: &str,
    source_grant: crate::CloudsyncWorkspaceKeyGrant,
) -> Result<anlg_e2ee::WorkspaceKey, String> {
    if source_grant.workspace_id != workspace_id || !source_grant.is_active {
        return Err("workspace E2EE source grant is invalid".to_string());
    }
    recovery_key
        .member_identity_key()
        .and_then(|identity| {
            identity.open_workspace_key(workspace_id, account_user_id, &source_grant.into())
        })
        .map_err(|error| error.to_string())
}

fn seal_workspace_e2ee_key(
    key: anlg_e2ee::WorkspaceKey,
    account_user_id: &str,
    workspace_id: &str,
    recipients: Vec<crate::WorkspaceE2eeKeyRecipient>,
) -> Result<crate::SealedWorkspaceE2eeKey, String> {
    if recipients.is_empty() || recipients.len() > 256 {
        return Err("workspace E2EE recipients are invalid".to_string());
    }
    let mut recipient_ids = std::collections::HashSet::with_capacity(recipients.len());
    if recipients.iter().any(|recipient| {
        uuid::Uuid::parse_str(recipient.user_id.trim()).is_err()
            || !recipient_ids.insert(recipient.user_id.trim())
    }) || !recipient_ids.contains(account_user_id)
    {
        return Err("workspace E2EE recipients are invalid".to_string());
    }

    let key_id = key.key_id().to_string();
    let grants = recipients
        .into_iter()
        .map(|recipient| {
            let user_id = uuid::Uuid::parse_str(recipient.user_id.trim())
                .expect("recipient IDs were validated")
                .to_string();
            let grant = anlg_e2ee::seal_workspace_key_for_member(
                &key,
                &recipient.public_key,
                &workspace_id,
                &user_id,
            )
            .map_err(|error| error.to_string())?;
            Ok(crate::WorkspaceE2eeKeyGrantUpload {
                user_id,
                ephemeral_public_key: grant.ephemeral_public_key,
                nonce: grant.nonce,
                ciphertext: grant.ciphertext,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(crate::SealedWorkspaceE2eeKey { key_id, grants })
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
    workspace_key_grants: Option<Vec<crate::CloudsyncWorkspaceKeyGrant>>,
    e2ee_witness: crate::CloudsyncE2eeWitness,
) -> Result<crate::CloudsyncTokenConfigurationResult, String> {
    let auth_generation = state.begin_cloudsync_auth_configuration();
    let personal_workspace_id = workspace_projection
        .as_ref()
        .map(|projection| projection.personal_workspace_id.clone())
        .unwrap_or_else(|| workspace_id.clone());
    let recovery_key = load_e2ee_recovery_key(app, &workspace_id)
        .await?
        .ok_or_else(|| {
            "end-to-end encryption recovery key setup is required before CloudSync can start"
                .to_string()
        })?;
    let shared_workspace_ids = workspace_projection
        .as_ref()
        .into_iter()
        .flat_map(|projection| projection.workspaces.iter())
        .filter(|workspace| workspace.kind == "shared")
        .map(|workspace| workspace.id.clone())
        .collect();
    let shared_keyrings = open_shared_workspace_keyrings(
        &recovery_key,
        &workspace_id,
        shared_workspace_ids,
        workspace_key_grants.unwrap_or_default(),
    )?;
    state
        .configure_cloudsync_token_with_projection_at_generation(
            crate::runtime::CloudsyncTokenConfiguration::new(
                database_id,
                token,
                workspace_id,
                workspace_projection.map(Into::into),
                e2ee_witness,
            ),
            Some(crate::runtime::E2eeWorkspaceKeyConfiguration::new(
                personal_workspace_id,
                recovery_key,
                shared_keyrings,
            )),
            auth_generation,
        )
        .await
        .map_err(|error| error.to_string())
}

fn open_shared_workspace_keyrings(
    recovery_key: &anlg_e2ee::RecoveryKey,
    account_user_id: &str,
    shared_workspace_ids: std::collections::HashSet<String>,
    grants: Vec<crate::CloudsyncWorkspaceKeyGrant>,
) -> Result<std::collections::HashMap<String, anlg_e2ee::WorkspaceKeyring>, String> {
    struct PendingKeyring {
        active: Option<anlg_e2ee::WorkspaceKey>,
        retired: Vec<anlg_e2ee::WorkspaceKey>,
        key_ids: std::collections::HashSet<String>,
    }

    let member_identity = recovery_key
        .member_identity_key()
        .map_err(|error| error.to_string())?;
    let mut pending = std::collections::HashMap::<String, PendingKeyring>::new();
    for grant in grants {
        if !shared_workspace_ids.contains(&grant.workspace_id) {
            return Err("workspace E2EE grant targets an unavailable workspace".to_string());
        }
        let workspace_id = grant.workspace_id.clone();
        let is_active = grant.is_active;
        let key_id = grant.key_id.clone();
        let key = member_identity
            .open_workspace_key(&workspace_id, account_user_id, &grant.into())
            .map_err(|error| error.to_string())?;
        let keyring = pending
            .entry(workspace_id)
            .or_insert_with(|| PendingKeyring {
                active: None,
                retired: Vec::new(),
                key_ids: std::collections::HashSet::new(),
            });
        if !keyring.key_ids.insert(key_id) || (is_active && keyring.active.is_some()) {
            return Err("workspace E2EE grant generations are invalid".to_string());
        }
        if is_active {
            keyring.active = Some(key);
        } else {
            keyring.retired.push(key);
        }
    }

    let mut keyrings = std::collections::HashMap::with_capacity(shared_workspace_ids.len());
    for workspace_id in shared_workspace_ids {
        let Some(pending) = pending.remove(&workspace_id) else {
            return Err("shared workspace E2EE key is unavailable".to_string());
        };
        let Some(active) = pending.active else {
            return Err("shared workspace active E2EE key is unavailable".to_string());
        };
        let mut keyring = anlg_e2ee::WorkspaceKeyring::new(active);
        for key in pending.retired {
            keyring.insert_retired(key);
        }
        keyrings.insert(workspace_id, keyring);
    }
    Ok(keyrings)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn configure_e2ee_replica<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, ManagedState>,
    workspace_id: String,
    e2ee_witness: crate::CloudsyncE2eeWitness,
) -> Result<crate::CloudsyncTokenConfigurationResult, String> {
    let auth_generation = state.begin_cloudsync_auth_configuration();
    let recovery_key = load_e2ee_recovery_key(app, &workspace_id)
        .await?
        .ok_or_else(|| {
            "end-to-end encryption recovery key setup is required before sync can start".to_string()
        })?;
    state
        .configure_replica_transport_at_generation(
            workspace_id,
            e2ee_witness,
            recovery_key,
            auth_generation,
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
