use desktop_runtime::{Result, ServiceError};
use futures::future::BoxFuture;
use serde_json::json;

use super::{
    CloudServices,
    transport::{failure, uuid},
};
use crate::product::services::Scope;

#[derive(Clone)]
pub enum ManagementCommand {
    RenameWorkspace(String),
    RemoveMember(String),
    RevokeShareAccess(String),
}

impl CloudServices {
    pub fn manage(
        &self,
        scope: Scope,
        command: ManagementCommand,
    ) -> BoxFuture<'static, Result<()>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .service(move |_| async move {
                    let _guard = this
                        .core
                        .mutations
                        .try_lock()
                        .map_err(|_| ServiceError::Busy)?;
                    let lease = this.core.lease(&scope).await?;
                    let workspace = || {
                        uuid(
                            scope
                                .workspace_id
                                .as_deref()
                                .ok_or_else(|| failure("Select a workspace"))?,
                        )
                    };
                    let (rpc, body) = match command {
                        ManagementCommand::RenameWorkspace(name) => {
                            let name = name.trim();
                            if name.is_empty() || name.len() > 256 {
                                return Err(failure("Enter a workspace name up to 256 bytes"));
                            }
                            (
                                "rename_workspace",
                                json!({"p_workspace_id": workspace()?, "p_name": name}),
                            )
                        }
                        ManagementCommand::RemoveMember(user) => (
                            "revoke_workspace_membership",
                            json!({"p_workspace_id": workspace()?, "p_user_id": uuid(&user)?}),
                        ),
                        ManagementCommand::RevokeShareAccess(grant) => (
                            "revoke_session_access_grant",
                            json!({"p_grant_id": uuid(&grant)?}),
                        ),
                    };
                    this.core.rpc(&lease, rpc, body).await?;
                    Ok(())
                })?
                .receive()
                .await
        })
    }
}
