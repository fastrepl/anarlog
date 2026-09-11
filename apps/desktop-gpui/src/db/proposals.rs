//! `session/queries/proposals.ts`: the `session_proposals` rows the chat's
//! `edit_memo` / `edit_summary` tools open for review, and applying or
//! declining them.

use super::Store;

/// `SessionProposalRecord`
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Proposal {
    pub id: String,
    pub session_id: String,
    pub kind: String,
    pub target_id: String,
    pub base_updated_at: String,
    pub current_markdown: String,
    pub proposed_markdown: String,
    pub status: String,
    pub source: String,
}

const PROPOSAL_SQL: &str = "
    SELECT id, session_id, kind, target_id, base_updated_at, current_markdown,
           proposed_markdown, status, source
    FROM session_proposals
    WHERE id = ?
    LIMIT 1
";

fn now() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
}

/// `loadTargetUpdatedAt`
async fn target_updated_at(
    pool: &sqlx::SqlitePool,
    target_id: &str,
    session_id: &str,
) -> anyhow::Result<Option<String>> {
    let target = if target_id.is_empty() {
        session_id
    } else {
        target_id
    };
    Ok(sqlx::query_scalar(
        "SELECT updated_at FROM session_documents
         WHERE id = ? AND session_id = ? AND deleted_at IS NULL
         LIMIT 1",
    )
    .bind(target)
    .bind(session_id)
    .fetch_optional(pool)
    .await?)
}

/// `loadPendingSessionProposals`: `(id, kind)` of the session's pending
/// proposals, newest first.
pub(super) async fn pending_for_session(
    pool: &sqlx::SqlitePool,
    session_id: &str,
) -> anyhow::Result<Vec<(String, String)>> {
    Ok(sqlx::query_as(
        "SELECT id, kind FROM session_proposals
         WHERE session_id = ? AND status = 'pending'
         ORDER BY created_at DESC, id DESC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?)
}

async fn load_proposal(pool: &sqlx::SqlitePool, id: &str) -> anyhow::Result<Option<Proposal>> {
    Ok(sqlx::query_as::<_, Proposal>(PROPOSAL_SQL)
        .bind(id)
        .fetch_optional(pool)
        .await?)
}

async fn set_status(pool: &sqlx::SqlitePool, id: &str, status: &str) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE session_proposals SET status = ?, updated_at = ?
         WHERE id = ? AND status = 'pending'",
    )
    .bind(status)
    .bind(now())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

impl Store {
    /// `persistChatSessionProposal`: the `pending` row with the target
    /// document's `updated_at` as its base, `source: chat`.
    pub fn persist_chat_session_proposal(
        &self,
        id: String,
        session_id: String,
        kind: String,
        target_id: String,
        current_markdown: String,
        proposed_markdown: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let base = target_updated_at(db.pool(), &target_id, &session_id)
                .await?
                .unwrap_or_default();
            sqlx::query(
                "INSERT INTO session_proposals (
                   id, session_id, kind, target_id, base_updated_at,
                   current_markdown, proposed_markdown, status, source
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', 'chat')",
            )
            .bind(&id)
            .bind(&session_id)
            .bind(&kind)
            .bind(&target_id)
            .bind(&base)
            .bind(&current_markdown)
            .bind(&proposed_markdown)
            .execute(db.pool())
            .await?;
            Ok(())
        })
    }

    /// `loadSessionProposal`
    pub fn load_session_proposal(
        &self,
        id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<Option<Proposal>>> {
        let db = self.db.clone();
        self.runtime
            .spawn(async move { load_proposal(db.pool(), &id).await })
    }

    /// `applySessionProposal`: refuse a stale or non-pending proposal, write
    /// the proposed markdown as the memo (`updateSession({ raw_md })`) or the
    /// summary (`updateEnhancedNoteContent`), and mark it applied. The error
    /// text is what the review shows.
    pub fn apply_session_proposal(
        self: &std::sync::Arc<Self>,
        id: String,
    ) -> tokio::task::JoinHandle<Result<(), String>> {
        let db = self.db.clone();
        let store = self.clone();
        self.runtime.spawn(async move {
            let pool = db.pool();
            let proposal = load_proposal(pool, &id)
                .await
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "Proposal not found".to_string())?;
            if proposal.status == "applied" {
                return Ok(());
            }
            if proposal.status != "pending" {
                return Err(format!("Proposal is {}", proposal.status));
            }
            let current = target_updated_at(pool, &proposal.target_id, &proposal.session_id)
                .await
                .map_err(|error| error.to_string())?;
            if current.is_some_and(|current| current != proposal.base_updated_at) {
                return Err(
                    "This proposal is stale. The meeting changed after it was created.".to_string(),
                );
            }
            let json = crate::document::md2json(&proposal.proposed_markdown);
            let now = now();
            if proposal.kind == "memo_replace" {
                sqlx::query(super::UPSERT_MEMO_SQL)
                    .bind(&proposal.session_id)
                    .bind("")
                    .bind(&json)
                    .bind(&now)
                    .bind(&now)
                    .bind(&proposal.session_id)
                    .execute(pool)
                    .await
                    .map_err(|error| error.to_string())?;
            } else {
                store
                    .update_enhanced_note_content(
                        proposal.target_id.clone(),
                        proposal.session_id.clone(),
                        json,
                        None,
                    )
                    .await
                    .map_err(|error| error.to_string())?
                    .map_err(|error| error.to_string())?;
            }
            set_status(pool, &proposal.id, "applied")
                .await
                .map_err(|error| error.to_string())
        })
    }

    /// `declineSessionProposal`
    pub fn decline_session_proposal(
        &self,
        id: String,
    ) -> tokio::task::JoinHandle<anyhow::Result<()>> {
        let db = self.db.clone();
        self.runtime.spawn(async move {
            let Some(proposal) = load_proposal(db.pool(), &id).await? else {
                return Ok(());
            };
            if proposal.status != "pending" {
                return Ok(());
            }
            set_status(db.pool(), &proposal.id, "declined").await
        })
    }
}
