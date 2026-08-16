use std::collections::HashMap;

pub(super) use anlg_db_sync::E2eeSyncHook;
#[cfg(test)]
pub(super) use anlg_db_sync::ReplicaSyncOutcome;

pub(crate) struct CloudsyncTokenConfiguration {
    pub(super) database_id: String,
    pub(super) token: String,
    pub(super) account_user_id: String,
    pub(super) workspace_projection: Option<anlg_db_app::CloudsyncWorkspaceProjection>,
    pub(super) e2ee_witness: crate::CloudsyncE2eeWitness,
}

pub(crate) struct E2eeWorkspaceKeyConfiguration {
    pub(super) personal_workspace_id: String,
    pub(super) recovery_key: anlg_e2ee::RecoveryKey,
    pub(super) shared_keyrings: HashMap<String, anlg_e2ee::WorkspaceKeyring>,
}

impl E2eeWorkspaceKeyConfiguration {
    pub(crate) fn new(
        personal_workspace_id: String,
        recovery_key: anlg_e2ee::RecoveryKey,
        shared_keyrings: HashMap<String, anlg_e2ee::WorkspaceKeyring>,
    ) -> Self {
        Self {
            personal_workspace_id,
            recovery_key,
            shared_keyrings,
        }
    }
}

impl CloudsyncTokenConfiguration {
    pub(crate) fn new(
        database_id: String,
        token: String,
        account_user_id: String,
        workspace_projection: Option<anlg_db_app::CloudsyncWorkspaceProjection>,
        e2ee_witness: crate::CloudsyncE2eeWitness,
    ) -> Self {
        Self {
            database_id,
            token,
            account_user_id,
            workspace_projection,
            e2ee_witness,
        }
    }
}
