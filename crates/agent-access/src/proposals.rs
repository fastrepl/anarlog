use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use crate::{DEFAULT_LIST_LIMIT, Error, MAX_LIST_LIMIT, Pagination, Result, pagination};

pub const DEFAULT_PROPOSAL_LIST_LIMIT: u32 = DEFAULT_LIST_LIMIT;
pub const MAX_PROPOSAL_LIST_LIMIT: u32 = MAX_LIST_LIMIT;

const KIND_MEMO: &str = "memo_replace";
const KIND_SUMMARY: &str = "summary_replace";
const STATUS_PENDING: &str = "pending";
const STATUS_DECLINED: &str = "declined";

#[derive(
    Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Type, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub struct CreateProposalInput {
    #[schemars(description = "Anarlog meeting id")]
    pub meeting_id: String,
    #[schemars(description = "summary_replace or memo_replace")]
    pub kind: String,
    #[schemars(description = "Summary document id. Required when multiple summaries exist.")]
    pub target_id: Option<String>,
    #[schemars(description = "Complete replacement markdown")]
    pub content: String,
    #[schemars(description = "cli, mcp, or chat")]
    pub source: Option<String>,
}

#[derive(
    Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Type, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub struct ListProposalsInput {
    #[schemars(description = "Limit results to one meeting")]
    pub meeting_id: Option<String>,
    #[schemars(description = "pending, applied, or declined. Defaults to pending.")]
    pub status: Option<String>,
    #[schemars(description = "Maximum results; defaults to 20 and is capped at 200")]
    #[schemars(range(min = 1, max = 200))]
    pub limit: Option<u32>,
    #[schemars(description = "Number of results to skip; defaults to 0")]
    pub offset: Option<u32>,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Type, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub struct GetProposalInput {
    #[schemars(description = "Proposal id")]
    pub proposal_id: String,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Type, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub struct DeclineProposalInput {
    #[schemars(description = "Proposal id")]
    pub proposal_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct Proposal {
    pub id: String,
    pub meeting_id: String,
    pub kind: String,
    pub target_id: String,
    pub base_updated_at: String,
    pub current_markdown: String,
    pub proposed_markdown: String,
    pub status: String,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
    pub diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub struct ProposalPage {
    pub proposals: Vec<Proposal>,
    pub pagination: Pagination,
}

pub async fn create_proposal(pool: &SqlitePool, input: CreateProposalInput) -> Result<Proposal> {
    let kind = normalize_kind(&input.kind)?;
    let content = input.content.trim().to_string();
    if content.is_empty() {
        return Err(Error::Invalid("proposal content is empty".to_string()));
    }
    let source = normalize_source(input.source.as_deref());
    let session = anlg_db_app::get_session(pool, &input.meeting_id)
        .await
        .map_err(|source| Error::Database {
            action: "load meeting",
            source,
        })?
        .ok_or_else(|| Error::NotFound(format!("meeting '{}'", input.meeting_id)))?;

    let target = resolve_target(pool, &session.id, kind, input.target_id.as_deref()).await?;
    let id = uuid::Uuid::new_v4().to_string();
    let row = anlg_db_app::insert_session_proposal(
        pool,
        anlg_db_app::InsertSessionProposal {
            id: &id,
            workspace_id: &session.workspace_id,
            session_id: &session.id,
            kind,
            target_id: &target.id,
            base_updated_at: &target.updated_at,
            current_markdown: &target.markdown,
            proposed_markdown: &content,
            source: &source,
        },
    )
    .await
    .map_err(|source| Error::Database {
        action: "create proposal",
        source,
    })?;

    Ok(Proposal::from(row))
}

pub async fn list_proposals(pool: &SqlitePool, input: ListProposalsInput) -> Result<ProposalPage> {
    if let Some(meeting_id) = input.meeting_id.as_deref() {
        let exists = anlg_db_app::get_session(pool, meeting_id)
            .await
            .map_err(|source| Error::Database {
                action: "load meeting",
                source,
            })?
            .is_some();
        if !exists {
            return Err(Error::NotFound(format!("meeting '{meeting_id}'")));
        }
    }

    let status = match input
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        None => Some(STATUS_PENDING.to_string()),
        Some("all") => None,
        Some(status) => Some(normalize_status(status)?.to_string()),
    };
    let limit = input
        .limit
        .unwrap_or(DEFAULT_PROPOSAL_LIST_LIMIT)
        .clamp(1, MAX_PROPOSAL_LIST_LIMIT);
    let offset = input.offset.unwrap_or(0);
    let mut rows = anlg_db_app::list_session_proposals(
        pool,
        input.meeting_id.as_deref(),
        status.as_deref(),
        limit + 1,
        offset,
    )
    .await
    .map_err(|source| Error::Database {
        action: "list proposals",
        source,
    })?;
    let has_more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let proposals = rows.into_iter().map(Proposal::from).collect::<Vec<_>>();
    Ok(ProposalPage {
        pagination: pagination(offset, limit, proposals.len(), None, has_more),
        proposals,
    })
}

pub async fn get_proposal(pool: &SqlitePool, input: GetProposalInput) -> Result<Proposal> {
    load_proposal(pool, &input.proposal_id).await
}

pub async fn decline_proposal(pool: &SqlitePool, input: DeclineProposalInput) -> Result<Proposal> {
    let current = load_proposal(pool, &input.proposal_id).await?;
    if current.status != STATUS_PENDING {
        return Err(Error::Conflict(format!(
            "proposal '{}' is {}",
            current.id, current.status
        )));
    }

    let row = anlg_db_app::update_session_proposal_status(
        pool,
        &input.proposal_id,
        STATUS_PENDING,
        STATUS_DECLINED,
    )
    .await
    .map_err(|source| Error::Database {
        action: "decline proposal",
        source,
    })?
    .ok_or_else(|| Error::NotFound(format!("proposal '{}'", input.proposal_id)))?;
    if row.status != STATUS_DECLINED {
        return Err(Error::Conflict(format!(
            "proposal '{}' is {}",
            row.id, row.status
        )));
    }
    Ok(Proposal::from(row))
}

pub fn proposal_unified_diff(current: &str, proposed: &str) -> String {
    if current == proposed {
        return "No changes.\n".to_string();
    }

    let mut diff = String::from("--- current\n+++ proposed\n");
    let current_lines = current.lines().collect::<Vec<_>>();
    let proposed_lines = proposed.lines().collect::<Vec<_>>();
    let max = current_lines.len().max(proposed_lines.len());
    for index in 0..max {
        match (current_lines.get(index), proposed_lines.get(index)) {
            (Some(left), Some(right)) if left == right => {
                diff.push(' ');
                diff.push_str(left);
                diff.push('\n');
            }
            (Some(left), Some(right)) => {
                diff.push('-');
                diff.push_str(left);
                diff.push('\n');
                diff.push('+');
                diff.push_str(right);
                diff.push('\n');
            }
            (Some(left), None) => {
                diff.push('-');
                diff.push_str(left);
                diff.push('\n');
            }
            (None, Some(right)) => {
                diff.push('+');
                diff.push_str(right);
                diff.push('\n');
            }
            (None, None) => {}
        }
    }
    diff
}

struct TargetSnapshot {
    id: String,
    markdown: String,
    updated_at: String,
}

async fn resolve_target(
    pool: &SqlitePool,
    meeting_id: &str,
    kind: &str,
    target_id: Option<&str>,
) -> Result<TargetSnapshot> {
    let meeting = crate::get_meeting(
        pool,
        crate::GetMeetingInput {
            meeting_id: meeting_id.to_string(),
        },
    )
    .await?;

    if kind == KIND_MEMO {
        let note = meeting
            .note
            .ok_or_else(|| Error::NotFound(format!("note for meeting '{meeting_id}'")))?;
        return Ok(TargetSnapshot {
            id: note.id,
            markdown: note.markdown,
            updated_at: note.updated_at,
        });
    }

    let summaries = meeting.summaries;
    if summaries.is_empty() {
        return Err(Error::NotFound(format!(
            "summary for meeting '{meeting_id}'"
        )));
    }

    if let Some(target_id) = target_id.map(str::trim).filter(|value| !value.is_empty()) {
        let summary = summaries
            .iter()
            .find(|document| document.id == target_id)
            .ok_or_else(|| {
                Error::NotFound(format!("summary '{target_id}' for meeting '{meeting_id}'"))
            })?;
        return Ok(TargetSnapshot {
            id: summary.id.clone(),
            markdown: summary.markdown.clone(),
            updated_at: summary.updated_at.clone(),
        });
    }

    if summaries.len() > 1 {
        return Err(Error::Invalid(
            "multiple summaries exist; specify target_id".to_string(),
        ));
    }

    let summary = &summaries[0];
    Ok(TargetSnapshot {
        id: summary.id.clone(),
        markdown: summary.markdown.clone(),
        updated_at: summary.updated_at.clone(),
    })
}

async fn load_proposal(pool: &SqlitePool, proposal_id: &str) -> Result<Proposal> {
    anlg_db_app::get_session_proposal(pool, proposal_id)
        .await
        .map_err(|source| Error::Database {
            action: "load proposal",
            source,
        })?
        .map(Proposal::from)
        .ok_or_else(|| Error::NotFound(format!("proposal '{proposal_id}'")))
}

fn normalize_kind(kind: &str) -> Result<&'static str> {
    match kind.trim() {
        "summary" | KIND_SUMMARY => Ok(KIND_SUMMARY),
        "memo" | "note" | KIND_MEMO => Ok(KIND_MEMO),
        other => Err(Error::Invalid(format!(
            "unsupported proposal kind '{other}'"
        ))),
    }
}

fn normalize_status(status: &str) -> Result<&'static str> {
    match status {
        STATUS_PENDING | "applied" | STATUS_DECLINED => Ok(match status {
            STATUS_PENDING => STATUS_PENDING,
            "applied" => "applied",
            _ => STATUS_DECLINED,
        }),
        other => Err(Error::Invalid(format!(
            "unsupported proposal status '{other}'"
        ))),
    }
}

fn normalize_source(source: Option<&str>) -> String {
    match source.map(str::trim).filter(|value| !value.is_empty()) {
        Some("mcp") => "mcp".to_string(),
        Some("chat") => "chat".to_string(),
        _ => "cli".to_string(),
    }
}

impl From<anlg_db_app::SessionProposalRow> for Proposal {
    fn from(value: anlg_db_app::SessionProposalRow) -> Self {
        let diff = proposal_unified_diff(&value.current_markdown, &value.proposed_markdown);
        Self {
            id: value.id,
            meeting_id: value.session_id,
            kind: value.kind,
            target_id: value.target_id,
            base_updated_at: value.base_updated_at,
            current_markdown: value.current_markdown,
            proposed_markdown: value.proposed_markdown,
            status: value.status,
            source: value.source,
            created_at: value.created_at,
            updated_at: value.updated_at,
            diff,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> anlg_db_core::Db {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        db
    }

    async fn seed_meeting(db: &anlg_db_core::Db) {
        sqlx::query(
            "INSERT INTO sessions (id, title, started_at) VALUES ('meeting-1', 'Planning', '2026-07-13')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_documents
             (id, session_id, kind, body_format, body, title, updated_at)
             VALUES
             ('meeting-1', 'meeting-1', 'note', 'markdown', 'Launch decision', 'Notes', '2026-07-13T00:00:00Z'),
             ('summary-1', 'meeting-1', 'summary', 'markdown', 'Ship Tuesday', 'Summary', '2026-07-13T00:00:00Z')",
        )
        .execute(db.pool())
        .await
        .unwrap();
    }

    #[test]
    fn unified_diff_marks_changed_lines() {
        assert_eq!(
            proposal_unified_diff("alpha\nbeta", "alpha\ngamma"),
            "--- current\n+++ proposed\n alpha\n-beta\n+gamma\n"
        );
    }

    #[tokio::test]
    async fn create_list_and_decline_summary_proposal() {
        let db = test_db().await;
        seed_meeting(&db).await;

        let created = create_proposal(
            db.pool(),
            CreateProposalInput {
                meeting_id: "meeting-1".to_string(),
                kind: "summary".to_string(),
                target_id: None,
                content: "Ship Wednesday".to_string(),
                source: Some("mcp".to_string()),
            },
        )
        .await
        .unwrap();

        assert_eq!(created.kind, KIND_SUMMARY);
        assert_eq!(created.target_id, "summary-1");
        assert_eq!(created.status, STATUS_PENDING);
        assert_eq!(created.source, "mcp");
        assert!(created.diff.contains("-Ship Tuesday"));
        assert!(created.diff.contains("+Ship Wednesday"));

        let page = list_proposals(
            db.pool(),
            ListProposalsInput {
                meeting_id: Some("meeting-1".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(page.proposals.len(), 1);
        assert_eq!(page.proposals[0].id, created.id);

        let declined = decline_proposal(
            db.pool(),
            DeclineProposalInput {
                proposal_id: created.id.clone(),
            },
        )
        .await
        .unwrap();
        assert_eq!(declined.status, STATUS_DECLINED);

        let conflict = decline_proposal(
            db.pool(),
            DeclineProposalInput {
                proposal_id: created.id,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(conflict, Error::Conflict(_)));
    }

    #[tokio::test]
    async fn create_rejects_empty_content_and_unknown_meeting() {
        let db = test_db().await;
        seed_meeting(&db).await;

        let empty = create_proposal(
            db.pool(),
            CreateProposalInput {
                meeting_id: "meeting-1".to_string(),
                kind: "memo".to_string(),
                target_id: None,
                content: "   ".to_string(),
                source: None,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(empty, Error::Invalid(_)));

        let missing = create_proposal(
            db.pool(),
            CreateProposalInput {
                meeting_id: "missing".to_string(),
                kind: "memo".to_string(),
                target_id: None,
                content: "Agenda".to_string(),
                source: None,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(missing, Error::NotFound(_)));
    }
}
