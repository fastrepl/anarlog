use desktop_runtime::{
    CancellationToken, DocumentSnapshot, DocumentWatch, RuntimeHandle, ServiceError, SessionId,
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

pub(super) async fn watch(
    runtime: RuntimeHandle,
    session: SessionId,
    cancel: CancellationToken,
    mut sender: mpsc::Sender<Result<Option<DocumentSnapshot>, String>>,
) {
    let result = async {
        let watch = runtime.watch_document(session)?.receive().await?;
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
