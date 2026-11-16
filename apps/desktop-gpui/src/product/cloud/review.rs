use std::sync::Arc;

use desktop_runtime::{Result, ServiceError};
use futures::future::BoxFuture;
use serde_json::Value;

use super::{
    CloudServices, host,
    panels::operation,
    transport::{failure, text},
};
use crate::product::services::{Action, Mutation, Outcome, Request, Scope, Surface};

#[derive(Clone)]
pub struct ConflictReview {
    pub remote_revision: u64,
    pub local_title: String,
    pub remote_title: String,
    pub local_preview: Arc<[String]>,
    pub remote_preview: Arc<[String]>,
}

#[derive(Clone, Copy)]
pub enum ConflictDecision {
    KeepRemote,
    PublishLocal,
}

impl CloudServices {
    pub fn import_recovery_file(&self, request: Request) -> BoxFuture<'static, Result<Outcome>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .service(move |services| async move {
                    let _guard = this
                        .core
                        .mutations
                        .try_lock()
                        .map_err(|_| ServiceError::Busy)?;
                    let code = host::import_recovery().await?;
                    if request.cancel.is_cancelled() {
                        return Err(ServiceError::Cancelled);
                    }
                    this.core
                        .perform(
                            Surface::CloudSync,
                            &request,
                            Mutation {
                                operation: operation(
                                    Action::ImportRecoveryKey,
                                    "Import recovery key",
                                ),
                                fields: Arc::from([(
                                    Arc::from("recovery_key"),
                                    Arc::from(code.as_str()),
                                )]),
                            },
                            &services,
                        )
                        .await
                })?
                .receive()
                .await
        })
    }

    pub fn conflict_review(
        &self,
        scope: Scope,
    ) -> BoxFuture<'static, Result<Option<ConflictReview>>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .service(move |services| async move {
                    let lease = this.core.lease(&scope).await?;
                    let session = scope
                        .session_id
                        .as_ref()
                        .ok_or_else(|| failure("Select a meeting"))?;
                    let Some(state) = this.core.share_state(&services, &lease, session).await?
                    else {
                        return Ok(None);
                    };
                    let Some(remote) = state.conflict else {
                        return Ok(None);
                    };
                    let source = this.core.share_source(&services, &lease, &scope).await?;
                    Ok(Some(ConflictReview {
                        remote_revision: remote["contentRevision"]
                            .as_u64()
                            .ok_or_else(|| failure("Invalid remote revision"))?,
                        local_title: source.title,
                        remote_title: text(&remote, "title")?.into(),
                        local_preview: preview(&source.body)?,
                        remote_preview: preview(&remote["body"])?,
                    }))
                })?
                .receive()
                .await
        })
    }

    pub fn resolve_conflict(
        &self,
        scope: Scope,
        reviewed_revision: u64,
        decision: ConflictDecision,
    ) -> BoxFuture<'static, Result<()>> {
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .service(move |services| async move {
                    let _guard = this
                        .core
                        .mutations
                        .try_lock()
                        .map_err(|_| ServiceError::Busy)?;
                    let lease = this.core.lease(&scope).await?;
                    let session = scope
                        .session_id
                        .as_ref()
                        .ok_or_else(|| failure("Select a meeting"))?;
                    let mut state = this
                        .core
                        .share_state(&services, &lease, session)
                        .await?
                        .ok_or(ServiceError::Conflict)?;
                    let remote = state.conflict.as_ref().ok_or(ServiceError::Conflict)?;
                    if remote["contentRevision"].as_u64() != Some(reviewed_revision) {
                        return Err(ServiceError::Conflict);
                    }
                    super::sharing::persist_snapshot(
                        &services,
                        super::account(&lease)?,
                        session,
                        &state.workspace_id,
                        remote,
                    )
                    .await?;
                    state.revision = reviewed_revision;
                    state.attachments = remote["attachments"]
                        .as_array()
                        .cloned()
                        .ok_or_else(|| failure("Invalid attachment list"))?;
                    state.pending = None;
                    state.conflict = None;
                    this.core
                        .save_share_state(&services, &lease, session, &state)
                        .await?;
                    if matches!(decision, ConflictDecision::PublishLocal) {
                        this.core.publish(&services, &lease, &scope, state).await?;
                    }
                    Ok(())
                })?
                .receive()
                .await
        })
    }
}

fn preview(body: &Value) -> Result<Arc<[String]>> {
    let json =
        serde_json::to_string_pretty(body).map_err(|_| failure("Cannot display publication"))?;
    Ok(json
        .lines()
        .flat_map(|line| {
            let chars: Vec<char> = line.chars().collect();
            if chars.is_empty() {
                vec![String::new()]
            } else {
                chars
                    .chunks(160)
                    .map(|chunk| chunk.iter().collect())
                    .collect()
            }
        })
        .collect::<Vec<_>>()
        .into())
}
