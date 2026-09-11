use std::{
    collections::{HashMap, HashSet},
    future::Future,
    pin::Pin,
    time::Duration,
};

use crate::{
    CloudsyncE2eeWitness, CloudsyncTokenConfigurationResult, CloudsyncWorkspaceKeyGrant,
    CloudsyncWorkspaceProjection, QueryEventSink,
};

pub const E2EE_SECRET_SCOPE: &str = "e2ee";
pub const E2EE_SECRET_READ_TIMEOUT: Duration = Duration::from_secs(25);
pub const E2EE_SECRET_READ_TIMEOUT_ERROR: &str = "E2EE secret read timed out";

pub trait E2eeSecretReader: Send + Sync {
    fn read(
        &self,
        scope: &str,
        key: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>, String>> + Send + '_>>;
}

pub trait E2eeSecretWriter: E2eeSecretReader {
    fn write(
        &self,
        scope: &str,
        key: &str,
        value: &str,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>>;
}

pub fn canonical_e2ee_account_user_id(account_user_id: &str) -> Result<String, String> {
    uuid::Uuid::parse_str(account_user_id.trim())
        .map(|id| id.to_string())
        .map_err(|_| "E2EE account ID is invalid".to_string())
}

pub fn e2ee_recovery_key_name(account_user_id: &str) -> Result<String, String> {
    Ok(format!(
        "account:{}:recovery-v1",
        canonical_e2ee_account_user_id(account_user_id)?
    ))
}

pub async fn read_e2ee_secret_with_timeout(
    timeout: Duration,
    read: impl Future<Output = Result<Option<String>, String>>,
) -> Result<Option<String>, String> {
    tokio::time::timeout(timeout, read)
        .await
        .map_err(|_| E2EE_SECRET_READ_TIMEOUT_ERROR.to_string())?
}

pub async fn load_e2ee_recovery_key(
    secrets: &dyn E2eeSecretReader,
    account_user_id: &str,
) -> Result<Option<anlg_e2ee::RecoveryKey>, String> {
    let key = e2ee_recovery_key_name(account_user_id)?;
    read_e2ee_secret_with_timeout(
        E2EE_SECRET_READ_TIMEOUT,
        secrets.read(E2EE_SECRET_SCOPE, &key),
    )
    .await?
    .map(|value| anlg_e2ee::RecoveryKey::parse(&value).map_err(|error| error.to_string()))
    .transpose()
}

pub fn create_e2ee_recovery_code() -> Result<String, String> {
    anlg_e2ee::RecoveryKey::generate()
        .map(|key| key.expose_code().to_string())
        .map_err(|error| error.to_string())
}

pub fn inspect_e2ee_recovery_key(code: &str) -> Result<String, String> {
    anlg_e2ee::RecoveryKey::parse(code)
        .map(|key| key.key_id())
        .map_err(|error| error.to_string())
}

pub async fn import_e2ee_recovery_key(
    secrets: &dyn E2eeSecretWriter,
    account_user_id: &str,
    code: &str,
) -> Result<(), String> {
    let key_name = e2ee_recovery_key_name(account_user_id)?;
    if load_e2ee_recovery_key(secrets, account_user_id)
        .await?
        .is_some()
    {
        return Err("E2EE recovery key is already configured".to_string());
    }
    let recovery_key = anlg_e2ee::RecoveryKey::parse(code).map_err(|error| error.to_string())?;
    secrets
        .write(
            E2EE_SECRET_SCOPE,
            &key_name,
            recovery_key.expose_code().as_str(),
        )
        .await
}

pub fn shared_workspace_ids(
    workspace_projection: Option<&CloudsyncWorkspaceProjection>,
) -> HashSet<String> {
    workspace_projection
        .into_iter()
        .flat_map(|projection| projection.workspaces.iter())
        .filter(|workspace| workspace.kind == "shared")
        .map(|workspace| workspace.id.clone())
        .collect()
}

pub fn open_shared_workspace_keyrings(
    recovery_key: &anlg_e2ee::RecoveryKey,
    account_user_id: &str,
    shared_workspace_ids: HashSet<String>,
    grants: Vec<CloudsyncWorkspaceKeyGrant>,
) -> Result<HashMap<String, anlg_e2ee::WorkspaceKeyring>, String> {
    struct Pending {
        active: Option<anlg_e2ee::WorkspaceKey>,
        retired: Vec<anlg_e2ee::WorkspaceKey>,
        key_ids: HashSet<String>,
    }
    let member = recovery_key
        .member_identity_key()
        .map_err(|e| e.to_string())?;
    let mut pending = HashMap::<String, Pending>::new();
    for grant in grants {
        if !shared_workspace_ids.contains(&grant.workspace_id) {
            return Err("workspace E2EE grant targets an unavailable workspace".into());
        }
        let workspace_id = grant.workspace_id.clone();
        let active = grant.is_active;
        let key_id = grant.key_id.clone();
        let key = member
            .open_workspace_key(&workspace_id, account_user_id, &grant.into())
            .map_err(|e| e.to_string())?;
        let entry = pending.entry(workspace_id).or_insert_with(|| Pending {
            active: None,
            retired: Vec::new(),
            key_ids: HashSet::new(),
        });
        if !entry.key_ids.insert(key_id) || (active && entry.active.is_some()) {
            return Err("workspace E2EE grant generations are invalid".into());
        }
        if active {
            entry.active = Some(key);
        } else {
            entry.retired.push(key);
        }
    }
    let mut keyrings = HashMap::with_capacity(shared_workspace_ids.len());
    for workspace_id in shared_workspace_ids {
        let Some(entry) = pending.remove(&workspace_id) else {
            return Err("shared workspace E2EE key is unavailable".into());
        };
        let Some(active) = entry.active else {
            return Err("shared workspace active E2EE key is unavailable".into());
        };
        let mut keyring = anlg_e2ee::WorkspaceKeyring::new(active);
        for key in entry.retired {
            keyring.insert_retired(key);
        }
        keyrings.insert(workspace_id, keyring);
    }
    Ok(keyrings)
}

impl<S: QueryEventSink> crate::DesktopDbRuntime<S> {
    pub async fn seal_workspace_e2ee_key_for_recipients(
        &self,
        secrets: &dyn E2eeSecretReader,
        account_user_id: &str,
        workspace_id: &str,
        recipients: Vec<crate::WorkspaceE2eeKeyRecipient>,
        rotate: bool,
        source_grant: Option<CloudsyncWorkspaceKeyGrant>,
    ) -> std::result::Result<crate::SealedWorkspaceE2eeKey, String> {
        let account_user_id = canonical_e2ee_account_user_id(account_user_id)?;
        let workspace_id = uuid::Uuid::parse_str(workspace_id.trim())
            .map(|workspace_id| workspace_id.to_string())
            .map_err(|_| "E2EE workspace ID is invalid".to_string())?;
        if recipients.is_empty() || recipients.len() > 256 {
            return Err("workspace E2EE recipients are invalid".to_string());
        }

        let key = if rotate {
            anlg_e2ee::WorkspaceKey::generate().map_err(|error| error.to_string())
        } else {
            match self.workspace_key(&workspace_id) {
                Some(key) => Ok(key),
                None => {
                    let source_grant = source_grant
                        .ok_or_else(|| "workspace E2EE source grant is invalid".to_string())?;
                    let recovery_key = load_e2ee_recovery_key(secrets, &account_user_id)
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

    #[allow(clippy::too_many_arguments)]
    pub async fn configure_cloudsync_token_with_keys(
        &self,
        secrets: &dyn E2eeSecretReader,
        database_id: String,
        token: String,
        workspace_id: String,
        workspace_projection: Option<CloudsyncWorkspaceProjection>,
        workspace_key_grants: Option<Vec<CloudsyncWorkspaceKeyGrant>>,
        e2ee_witness: CloudsyncE2eeWitness,
    ) -> Result<CloudsyncTokenConfigurationResult, String> {
        let result = async {
            let generation = self.begin_cloudsync_auth_configuration();
            let personal = workspace_projection
                .as_ref()
                .map(|p| p.personal_workspace_id.clone())
                .unwrap_or_else(|| workspace_id.clone());
            let recovery = load_e2ee_recovery_key(secrets, &workspace_id)
                .await?
                .ok_or_else(|| "end-to-end encryption recovery key setup is required before CloudSync can start".to_string())?;
            let keyrings = open_shared_workspace_keyrings(
                &recovery,
                &workspace_id,
                shared_workspace_ids(workspace_projection.as_ref()),
                workspace_key_grants.unwrap_or_default(),
            )?;
            self.configure_cloudsync_token_with_projection_at_generation(
                crate::runtime::CloudsyncTokenConfiguration::new(
                    database_id,
                    token,
                    workspace_id,
                    workspace_projection.map(Into::into),
                    e2ee_witness,
                ),
                Some(crate::runtime::E2eeWorkspaceKeyConfiguration::new(
                    personal, recovery, keyrings,
                )),
                generation,
            )
            .await
            .map_err(|e| e.to_string())
        }.await;
        self.record_cloudsync_configuration_result("configure_token", &result);
        result
    }

    pub async fn configure_e2ee_replica_with_keys(
        &self,
        secrets: &dyn E2eeSecretReader,
        workspace_id: String,
        e2ee_witness: CloudsyncE2eeWitness,
        workspace_projection: Option<CloudsyncWorkspaceProjection>,
        workspace_key_grants: Option<Vec<CloudsyncWorkspaceKeyGrant>>,
    ) -> Result<CloudsyncTokenConfigurationResult, String> {
        let result = async {
            let generation = self.begin_cloudsync_auth_configuration();
            let recovery = load_e2ee_recovery_key(secrets, &workspace_id)
                .await?
                .ok_or_else(|| {
                    "end-to-end encryption recovery key setup is required before sync can start"
                        .to_string()
                })?;
            let keyrings = open_shared_workspace_keyrings(
                &recovery,
                &workspace_id,
                shared_workspace_ids(workspace_projection.as_ref()),
                workspace_key_grants.unwrap_or_default(),
            )?;
            self.configure_replica_transport_at_generation(
                workspace_id.clone(),
                e2ee_witness,
                crate::runtime::E2eeWorkspaceKeyConfiguration::new(
                    workspace_id,
                    recovery,
                    keyrings,
                ),
                workspace_projection.map(Into::into),
                generation,
            )
            .await
            .map_err(|e| e.to_string())
        }
        .await;
        self.record_cloudsync_configuration_result("configure_replica", &result);
        result
    }
}

pub fn open_workspace_e2ee_source_key(
    recovery_key: &anlg_e2ee::RecoveryKey,
    account_user_id: &str,
    workspace_id: &str,
    source_grant: CloudsyncWorkspaceKeyGrant,
) -> std::result::Result<anlg_e2ee::WorkspaceKey, String> {
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

pub fn seal_workspace_e2ee_key(
    key: anlg_e2ee::WorkspaceKey,
    account_user_id: &str,
    workspace_id: &str,
    recipients: Vec<crate::WorkspaceE2eeKeyRecipient>,
) -> std::result::Result<crate::SealedWorkspaceE2eeKey, String> {
    if recipients.is_empty() || recipients.len() > 256 {
        return Err("workspace E2EE recipients are invalid".to_string());
    }
    let mut recipient_ids = HashSet::with_capacity(recipients.len());
    if recipients.iter().any(|recipient| {
        uuid::Uuid::parse_str(recipient.user_id.trim()).is_err()
            || !recipient_ids.insert(recipient.user_id.trim().to_string())
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
                workspace_id,
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
        .collect::<std::result::Result<Vec<_>, String>>()?;
    Ok(crate::SealedWorkspaceE2eeKey { key_id, grants })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct TestSecrets(Mutex<Option<String>>);

    impl E2eeSecretReader for TestSecrets {
        fn read(
            &self,
            _scope: &str,
            _key: &str,
        ) -> Pin<Box<dyn Future<Output = Result<Option<String>, String>> + Send + '_>> {
            let value = self.0.lock().unwrap().clone();
            Box::pin(async move { Ok(value) })
        }
    }

    impl E2eeSecretWriter for TestSecrets {
        fn write(
            &self,
            _scope: &str,
            _key: &str,
            value: &str,
        ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
            *self.0.lock().unwrap() = Some(value.to_string());
            Box::pin(async { Ok(()) })
        }
    }

    #[test]
    fn inspect_round_trips_generated_recovery_key() {
        let code = create_e2ee_recovery_code().unwrap();
        let key = anlg_e2ee::RecoveryKey::parse(&code).unwrap();
        assert_eq!(inspect_e2ee_recovery_key(&code).unwrap(), key.key_id());
    }

    #[tokio::test]
    async fn import_rejects_invalid_recovery_key() {
        let secrets = TestSecrets(Mutex::new(None));
        let error =
            import_e2ee_recovery_key(&secrets, "00000000-0000-0000-0000-000000000000", "garbage")
                .await
                .unwrap_err();
        assert!(!error.is_empty());
    }

    #[tokio::test]
    async fn import_rejects_existing_recovery_key() {
        let code = create_e2ee_recovery_code().unwrap();
        let secrets = TestSecrets(Mutex::new(Some(code)));
        let error =
            import_e2ee_recovery_key(&secrets, "00000000-0000-0000-0000-000000000000", "garbage")
                .await
                .unwrap_err();
        assert_eq!(error, "E2EE recovery key is already configured");
    }
}
