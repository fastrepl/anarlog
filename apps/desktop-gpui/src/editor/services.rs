use desktop_runtime::{
    CancellationToken, DocumentId, DocumentSnapshot, DocumentWatch, RuntimeHandle, ServiceError,
};
use futures::{SinkExt, channel::mpsc};
use serde_json::json;

use super::menu::{MentionCandidate, MentionTarget};

pub(super) async fn mentions(
    runtime: RuntimeHandle,
    query: String,
    cancel: CancellationToken,
) -> Result<Vec<MentionCandidate>, String> {
    runtime.read(cancel, move |services| async move {
        let rows = services.executor.execute(
            "SELECT id, label, kind FROM (
              SELECT id, name AS label, 'human' AS kind FROM humans WHERE deleted_at IS NULL
              UNION ALL SELECT id, title AS label, 'session' AS kind FROM sessions WHERE deleted_at IS NULL
              UNION ALL SELECT id, name AS label, 'organization' AS kind FROM organizations WHERE deleted_at IS NULL
            ) WHERE instr(lower(label), lower(?)) > 0 ORDER BY label COLLATE NOCASE, kind, id LIMIT 5".into(),
            vec![json!(query)],
        ).await.map_err(|error| ServiceError::Failed(error.to_string().into()))?;
        Ok(rows.into_iter().filter_map(|row| {
            let id = row["id"].as_str()?.to_owned();
            let label = row["label"].as_str()?.to_owned();
            let target = match row["kind"].as_str()? {
                "human" => MentionTarget::Human(id),
                "session" => MentionTarget::Session(id),
                "organization" => MentionTarget::Organization(id),
                _ => return None,
            };
            Some(MentionCandidate { label, target })
        }).collect())
    }).map_err(|error| error.to_string())?.receive().await.map_err(|error| error.to_string())
}

/// The stored revision of `document`. A memo not saved yet has no row of its own; another
/// device may have created the session's memo meanwhile.
pub(super) async fn stored(
    runtime: RuntimeHandle,
    document: DocumentSnapshot,
) -> Result<Option<DocumentSnapshot>, ServiceError> {
    if document.is_unsaved() {
        return Ok(runtime
            .open_session(document.session_id, CancellationToken::new())?
            .receive()
            .await?
            .note);
    }
    let rows = runtime
        .read(CancellationToken::new(), move |services| async move {
            services
                .executor
                .execute(
                    "SELECT id, session_id, body_format, body, updated_at FROM session_documents WHERE id = ? AND session_id = ? AND deleted_at IS NULL".into(),
                    vec![json!(document.id), json!(document.session_id)],
                )
                .await
                .map_err(|error| ServiceError::Failed(error.to_string().into()))
        })?
        .receive()
        .await?;
    rows.into_iter()
        .next()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| ServiceError::Failed(error.to_string().into()))
}

pub(super) async fn watch(
    runtime: RuntimeHandle,
    document: DocumentId,
    cancel: CancellationToken,
    mut sender: mpsc::Sender<Result<Option<DocumentSnapshot>, String>>,
) {
    let result = async {
        // By document id, so a summary editor follows its own document, not the session memo.
        let watch = runtime
            .watch_query(
                "SELECT id, session_id, body_format, body, updated_at FROM session_documents WHERE id = ? AND deleted_at IS NULL".into(),
                vec![json!(document)],
            )?
            .receive()
            .await?;
        let mut watch = DocumentWatch(watch);
        loop {
            let snapshot = if let Some(error) = watch.0.terminal_error() {
                Err(error.to_string())
            } else {
                let rows = watch.0.snapshots.borrow_and_update().rows.clone();
                rows.first()
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(|error| error.to_string())
            };
            tokio::select! {
                biased;
                _ = cancel.cancelled() => break,
                sent = sender.send(snapshot) => if sent.is_err() { break; },
            }
            tokio::select! {
                biased;
                _ = cancel.cancelled() => break,
                error = watch.0.errors.recv() => {
                    if let Some(error) = error {
                        tokio::select! {
                            _ = cancel.cancelled() => {},
                            _ = sender.send(Err(error.to_string())) => {},
                        }
                    }
                    break;
                },
                changed = watch.0.snapshots.changed() => {
                    if changed.is_err() || watch.0.terminal_error().is_some() {
                        if let Some(error) = watch.0.terminal_error() {
                            tokio::select! {
                                _ = cancel.cancelled() => {},
                                _ = sender.send(Err(error.to_string())) => {},
                            }
                        }
                        break;
                    }
                },
            }
        }
        watch.unsubscribe().await
    }
    .await;
    if let Err(error) = result {
        tokio::select! {
            _ = cancel.cancelled() => {},
            _ = sender.send(Err(error.to_string())) => {},
        }
    }
}
