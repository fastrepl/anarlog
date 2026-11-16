use std::sync::{Arc, Mutex};

use desktop_runtime::{
    CancellationToken, DocumentSnapshot, Result, RuntimeHandle, SaveDocument, ServiceError,
};
use serde_json::{Value, json};
use tokio::sync::oneshot;

use super::{
    ai::{MeetingTool, ToolExecutor},
    model::{MAX_TEXT, failure},
    store::TranscriptStore,
};

#[derive(Clone)]
pub struct Proposal {
    pub id: Arc<str>,
    pub before: Arc<str>,
    pub after: Arc<str>,
}

struct Pending {
    proposal: Proposal,
    reply: oneshot::Sender<bool>,
}

#[derive(Clone, Default)]
pub struct Approvals(Arc<Mutex<Option<Pending>>>);

impl Approvals {
    pub fn pending(&self) -> Option<Proposal> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(|p| p.proposal.clone())
    }

    pub fn decide(&self, id: &str, approved: bool) {
        let mut pending = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if pending
            .as_ref()
            .is_some_and(|p| p.proposal.id.as_ref() == id)
            && let Some(pending) = pending.take()
        {
            let _ = pending.reply.send(approved);
        }
    }

    async fn request(
        &self,
        before: String,
        after: String,
        cancel: &CancellationToken,
    ) -> Result<()> {
        if before.len() + after.len() > MAX_TEXT {
            return Err(failure("Proposed edit exceeds review limit."));
        }
        let id: Arc<str> = uuid::Uuid::new_v4().to_string().into();
        let (send, receive) = oneshot::channel();
        {
            let mut pending = self.0.lock().map_err(failure)?;
            if pending.is_some() {
                return Err(ServiceError::Busy);
            }
            *pending = Some(Pending {
                proposal: Proposal {
                    id: id.clone(),
                    before: before.into(),
                    after: after.into(),
                },
                reply: send,
            });
        }
        let result = tokio::select! {
            _ = cancel.cancelled() => Err(ServiceError::Cancelled),
            approved = receive => match approved {
                Ok(true) => Ok(()),
                Ok(false) => Err(failure("The user declined this edit.")),
                Err(_) => Err(ServiceError::Closed),
            }
        };
        self.decide(&id, false);
        result
    }
}

pub fn executor(runtime: RuntimeHandle, approvals: Approvals) -> ToolExecutor {
    Arc::new(move |tool, cancellation| {
        let runtime = runtime.clone();
        let approvals = approvals.clone();
        Box::pin(async move { execute(&runtime, &approvals, tool, cancellation).await })
    })
}

async fn execute(
    runtime: &RuntimeHandle,
    approvals: &Approvals,
    tool: MeetingTool,
    cancellation: CancellationToken,
) -> Result<Value> {
    if cancellation.is_cancelled() {
        return Err(ServiceError::Cancelled);
    }
    match tool {
        MeetingTool::EditMemo {
            session_id,
            expected,
            body,
        } => {
            let note = runtime
                .open_session(session_id.into(), cancellation.clone())?
                .receive()
                .await?
                .note
                .ok_or(ServiceError::Conflict)?;
            edit(runtime, approvals, note, expected, body, cancellation).await
        }
        MeetingTool::EditSummary {
            document_id,
            expected,
            body,
        } => {
            let note = runtime.read(cancellation.clone(), move |services| async move {
                let rows = services.executor.execute(
                    "SELECT d.id, d.session_id, d.body, d.body_format, d.updated_at FROM session_documents d JOIN sessions s ON s.id = d.session_id WHERE d.id = ? AND d.kind IN ('summary', 'template_output') AND d.deleted_at IS NULL AND s.deleted_at IS NULL AND s.locked = 0".into(),
                    vec![json!(document_id)]).await.map_err(failure)?;
                super::context::document_snapshot(rows.first().ok_or(ServiceError::Conflict)?)
            })?.receive().await?;
            edit(runtime, approvals, note, expected, body, cancellation).await
        }
        MeetingTool::CorrectTranscript {
            transcript_id,
            word_id,
            expected,
            text,
        } => {
            let store = TranscriptStore(runtime.clone());
            let id: Arc<str> = transcript_id.into();
            let snapshot = store.snapshot(id.clone())?.receive().await?;
            let word = snapshot
                .words
                .into_iter()
                .find(|word| word.id == word_id && word.text == expected)
                .ok_or(ServiceError::Conflict)?;
            approvals
                .request(expected, text.clone(), &cancellation)
                .await?;
            if cancellation.is_cancelled() {
                return Err(ServiceError::Cancelled);
            }
            store.edit_word(id, word, Some(text))?.receive().await?;
            Ok(json!({"applied": true}))
        }
        MeetingTool::Transcript { session_id } => {
            let snapshot = TranscriptStore(runtime.clone())
                .load(session_id.into(), cancellation)?
                .receive()
                .await?;
            let text = snapshot
                .segments
                .iter()
                .map(|s| format!("{}: {}", s.speaker_label, s.text))
                .collect::<Vec<_>>()
                .join("\n");
            if text.len() > MAX_TEXT {
                return Err(failure("Transcript exceeds tool output limit."));
            }
            Ok(json!({"transcript": text}))
        }
        tool => {
            let (sql, params) = match tool {
                MeetingTool::Meeting { session_id } => (
                    "SELECT s.id, s.title, s.created_at, d.id AS document_id, d.kind, d.body_format, d.body FROM sessions s LEFT JOIN session_documents d ON d.session_id = s.id AND d.deleted_at IS NULL WHERE s.id = ? AND s.deleted_at IS NULL AND s.locked = 0 LIMIT 50",
                    vec![json!(session_id)],
                ),
                MeetingTool::History { session_id } => (
                    "SELECT m.role, m.content FROM chat_messages m JOIN sessions s ON s.id = m.chat_group_id WHERE s.id = ? AND s.deleted_at IS NULL AND s.locked = 0 AND m.deleted_at IS NULL ORDER BY m.created_at DESC, m.id DESC LIMIT 20",
                    vec![json!(session_id)],
                ),
                MeetingTool::Search { query } => (
                    "SELECT id, title, created_at FROM sessions WHERE deleted_at IS NULL AND locked = 0 AND instr(lower(title), lower(?)) > 0 ORDER BY updated_at DESC LIMIT 30",
                    vec![json!(query)],
                ),
                _ => {
                    return Err(failure(
                        "This tool is not advertised by the meeting provider.",
                    ));
                }
            };
            runtime
                .read(cancellation, move |services| async move {
                    let result = services
                        .executor
                        .execute(sql.into(), params)
                        .await
                        .map_err(failure)?;
                    let result = json!(result);
                    if result.to_string().len() > MAX_TEXT {
                        return Err(failure("Tool result exceeds context limit."));
                    }
                    Ok(result)
                })?
                .receive()
                .await
        }
    }
}

async fn edit(
    runtime: &RuntimeHandle,
    approvals: &Approvals,
    note: DocumentSnapshot,
    expected: String,
    body: String,
    cancellation: CancellationToken,
) -> Result<Value> {
    let before = super::context::markdown(&note.body, &note.body_format)?;
    if expected != before && expected != note.updated_at.as_ref() {
        return Err(ServiceError::Conflict);
    }
    let encoded = if note.body_format.as_ref() == "prosemirror_json" {
        let original: Value = serde_json::from_str(&note.body).map_err(failure)?;
        let converted = anlg_tiptap::md_to_tiptap_json(&before).map_err(failure)?;
        let updated = anlg_tiptap::md_to_tiptap_json(&body).map_err(failure)?;
        let merged = anlg_tiptap::merge::merge_documents(
            &converted,
            &original,
            &updated,
            anlg_tiptap::merge::MergeSide::Local,
        )
        .ok_or_else(|| failure("Invalid document; original preserved."))?;
        if !merged.conflicts.is_empty() {
            return Err(failure(
                "This edit overlaps rich content that Markdown cannot preserve. Apply it in the editor.",
            ));
        }
        serde_json::to_string(&merged.doc).map_err(failure)?
    } else {
        body.clone()
    };
    approvals.request(before, body, &cancellation).await?;
    if cancellation.is_cancelled() {
        return Err(ServiceError::Cancelled);
    }
    let saved = runtime
        .save_document(SaveDocument {
            base: note,
            body: encoded.into(),
        })?
        .receive()
        .await?;
    Ok(json!({"applied": true, "document_id": saved.id, "updated_at": saved.updated_at}))
}
