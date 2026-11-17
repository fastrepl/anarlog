//! Shipping's `session_proposals` inbox (`session/queries/proposals.ts`,
//! `session/proposal-review.ts`): memo and summary replacements proposed by chat, the CLI or
//! MCP, applied or declined from the review tab, the chat card or the note banner.

use std::sync::Arc;

use desktop_runtime::{
    CancellationToken, DocumentSnapshot, Result, RuntimeHandle, SaveDocument, SessionId,
};
use serde_json::{Value, json};

use crate::meeting::model::failure;

pub const MEMO: &str = "memo_replace";
pub const SUMMARY: &str = "summary_replace";

#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    pub id: Arc<str>,
    pub session_id: Arc<str>,
    pub kind: Arc<str>,
    pub target_id: Arc<str>,
    pub base_updated_at: Arc<str>,
    pub current: Arc<str>,
    pub proposed: Arc<str>,
    pub status: Arc<str>,
    pub source: Arc<str>,
}

impl Proposal {
    /// Shipping's `proposalKindLabel`: anything but a memo replacement reviews as a summary.
    pub fn memo(&self) -> bool {
        self.kind.as_ref() == MEMO
    }

    fn decode(row: &Value) -> Self {
        let text = |key: &str| Arc::<str>::from(row[key].as_str().unwrap_or_default());
        Self {
            id: text("id"),
            session_id: text("session_id"),
            kind: text("kind"),
            target_id: text("target_id"),
            base_updated_at: text("base_updated_at"),
            current: text("current_markdown"),
            proposed: text("proposed_markdown"),
            status: text("status"),
            source: text("source"),
        }
    }
}

/// A chat edit's proposal, stored before the review opens.
pub struct NewProposal {
    pub id: Arc<str>,
    pub session_id: Arc<str>,
    pub kind: &'static str,
    pub target_id: Arc<str>,
    pub current: String,
    pub proposed: String,
}

const COLUMNS: &str = "SELECT id, session_id, kind, target_id, base_updated_at, current_markdown, proposed_markdown, status, source FROM session_proposals";

async fn rows(runtime: &RuntimeHandle, sql: String, params: Vec<Value>) -> Result<Vec<Value>> {
    runtime
        .read(CancellationToken::new(), move |services| async move {
            services
                .executor
                .execute(sql, params)
                .await
                .map_err(failure)
        })?
        .receive()
        .await
}

pub async fn load(runtime: &RuntimeHandle, id: Arc<str>) -> Result<Option<Proposal>> {
    Ok(rows(
        runtime,
        format!("{COLUMNS} WHERE id = ? LIMIT 1"),
        vec![json!(id)],
    )
    .await?
    .first()
    .map(Proposal::decode))
}

/// Pending proposals of one note, newest first.
pub async fn pending(runtime: &RuntimeHandle, session: Arc<str>) -> Result<Vec<Proposal>> {
    Ok(rows(
        runtime,
        format!(
            "{COLUMNS} WHERE session_id = ? AND status = 'pending' ORDER BY created_at DESC, id DESC"
        ),
        vec![json!(session)],
    )
    .await?
    .iter()
    .map(Proposal::decode)
    .collect())
}

async fn target_updated_at(
    runtime: &RuntimeHandle,
    target: &str,
    session: &str,
) -> Result<Option<String>> {
    let target = if target.is_empty() { session } else { target };
    Ok(rows(
        runtime,
        "SELECT updated_at FROM session_documents WHERE id = ? AND session_id = ? AND deleted_at IS NULL LIMIT 1".into(),
        vec![json!(target), json!(session)],
    )
    .await?
    .first()
    .and_then(|row| row["updated_at"].as_str().map(str::to_owned)))
}

pub async fn persist_chat(runtime: &RuntimeHandle, proposal: NewProposal) -> Result<()> {
    let base = target_updated_at(runtime, &proposal.target_id, &proposal.session_id)
        .await?
        .unwrap_or_default();
    runtime
        .submit(move |services| async move {
            services
                .executor
                .execute(
                    "INSERT INTO session_proposals (id, session_id, kind, target_id, base_updated_at, current_markdown, proposed_markdown, status, source) VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', 'chat')".into(),
                    vec![
                        json!(proposal.id),
                        json!(proposal.session_id),
                        json!(proposal.kind),
                        json!(proposal.target_id),
                        json!(base),
                        json!(proposal.current),
                        json!(proposal.proposed),
                    ],
                )
                .await
                .map_err(failure)?;
            Ok(())
        })?
        .receive()
        .await
}

async fn set_status(runtime: &RuntimeHandle, id: Arc<str>, status: &'static str) -> Result<()> {
    runtime
        .submit(move |services| async move {
            services
                .executor
                .execute(
                    "UPDATE session_proposals SET status = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? AND status = 'pending'".into(),
                    vec![json!(status), json!(id)],
                )
                .await
                .map_err(failure)?;
            Ok(())
        })?
        .receive()
        .await
}

/// Shipping's `applySessionProposal`: replaces the memo or summary with the proposed
/// Markdown unless the target changed after the proposal was made.
pub async fn apply(runtime: &RuntimeHandle, id: Arc<str>) -> Result<()> {
    let proposal = load(runtime, id.clone())
        .await?
        .ok_or_else(|| failure("Proposal not found"))?;
    match proposal.status.as_ref() {
        "applied" => return Ok(()),
        "pending" => {}
        status => return Err(failure(format!("Proposal is {status}"))),
    }
    let current = target_updated_at(runtime, &proposal.target_id, &proposal.session_id).await?;
    if current
        .as_deref()
        .is_some_and(|current| current != proposal.base_updated_at.as_ref())
    {
        return Err(failure(
            "This proposal is stale. The meeting changed after it was created.",
        ));
    }
    let body: Arc<str> = anlg_tiptap::md_to_tiptap_json(&proposal.proposed)
        .map_err(failure)?
        .to_string()
        .into();
    let session = SessionId(proposal.session_id.clone());
    let base = if proposal.memo() {
        runtime
            .open_session(session.clone(), CancellationToken::new())?
            .receive()
            .await?
            .note
            .unwrap_or_else(|| DocumentSnapshot::unsaved_note(session))
    } else {
        let row = rows(
            runtime,
            "SELECT id, session_id, body_format, body, updated_at FROM session_documents WHERE id = ? AND session_id = ? AND deleted_at IS NULL".into(),
            vec![json!(proposal.target_id), json!(proposal.session_id)],
        )
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| failure("Summary not found"))?;
        serde_json::from_value(row).map_err(failure)?
    };
    runtime
        .save_document(SaveDocument { base, body })?
        .receive()
        .await?;
    set_status(runtime, id, "applied").await
}

/// Shipping's `declineSessionProposal`: only a pending proposal changes.
pub async fn decline(runtime: &RuntimeHandle, id: Arc<str>) -> Result<()> {
    set_status(runtime, id, "declined").await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meeting::store::statement;

    async fn sql(runtime: &RuntimeHandle, sql: &'static str) {
        runtime
            .submit(move |services| async move {
                services
                    .executor
                    .execute_transaction(vec![statement(sql, vec![], None)])
                    .await
                    .map_err(failure)
            })
            .unwrap()
            .receive()
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn apply_replaces_the_target_once_and_refuses_stale_or_declined_proposals() {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
            database: directory.path().join("proposals.sqlite"),
        })
        .unwrap();
        ready.receive().await.unwrap();
        sql(
            &runtime,
            "INSERT INTO sessions (id, title) VALUES ('s', 'Sync')",
        )
        .await;
        sql(&runtime, r#"INSERT INTO session_documents (id, session_id, kind, title, body, updated_at) VALUES ('d', 's', 'summary', 'Summary', '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Old"}]}]}', '2026-11-01T00:00:00.000Z')"#).await;
        let propose = |id: &'static str| NewProposal {
            id: id.into(),
            session_id: "s".into(),
            kind: SUMMARY,
            target_id: "d".into(),
            current: "Old".into(),
            proposed: format!("New from {id}"),
        };
        persist_chat(&runtime, propose("p1")).await.unwrap();
        persist_chat(&runtime, propose("p2")).await.unwrap();
        persist_chat(&runtime, propose("p3")).await.unwrap();
        assert_eq!(pending(&runtime, "s".into()).await.unwrap().len(), 3);

        decline(&runtime, "p3".into()).await.unwrap();
        apply(&runtime, "p1".into()).await.unwrap();
        // Applying twice is a no-op, like a chat card and its tool both applying.
        apply(&runtime, "p1".into()).await.unwrap();
        let stale = apply(&runtime, "p2".into()).await.unwrap_err().to_string();
        assert!(stale.contains("stale"), "{stale}");
        let declined = apply(&runtime, "p3".into()).await.unwrap_err().to_string();
        assert_eq!(declined, "Proposal is declined");

        let body = rows(
            &runtime,
            "SELECT body FROM session_documents WHERE id = 'd'".into(),
            vec![],
        )
        .await
        .unwrap()[0]["body"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(body.contains("New from p1"), "{body}");
        let left = pending(&runtime, "s".into()).await.unwrap();
        assert_eq!(
            left.iter().map(|p| p.id.as_ref()).collect::<Vec<_>>(),
            ["p2"]
        );
        runtime.shutdown().await.unwrap();
    }
}
