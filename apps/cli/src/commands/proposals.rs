use std::path::PathBuf;

use crate::cli::ProposalCommand;
use crate::{Result, output};
use anlg_agent_access::{
    CreateProposalInput, DeclineProposalInput, GetProposalInput, ListProposalsInput, Proposal,
    create_proposal, decline_proposal, get_proposal, list_proposals,
};

pub async fn run(db: &anlg_db_core::Db, command: ProposalCommand, json: bool) -> Result<()> {
    match command {
        ProposalCommand::Create {
            meeting_id,
            kind,
            target_id,
            content,
            content_file,
        } => {
            let proposal = create_proposal(
                db.pool(),
                CreateProposalInput {
                    meeting_id,
                    kind: kind.as_str().to_string(),
                    target_id,
                    content: read_content(content, content_file)?,
                    source: Some("cli".to_string()),
                },
            )
            .await?;
            emit_proposal("proposals.create", &proposal, json)
        }
        ProposalCommand::List {
            meeting_id,
            status,
            limit,
            offset,
        } => {
            let page = list_proposals(
                db.pool(),
                ListProposalsInput {
                    meeting_id,
                    status,
                    limit: Some(limit),
                    offset: Some(offset),
                },
            )
            .await?;
            if json {
                output::emit(&output::json(
                    "proposals.list",
                    &page.proposals,
                    Some(&page.pagination),
                )?);
            } else if page.proposals.is_empty() {
                output::emit("No proposals found.");
            } else {
                output::emit(&render_list(&page.proposals));
            }
            Ok(())
        }
        ProposalCommand::Show { id } => {
            let proposal = get_proposal(db.pool(), GetProposalInput { proposal_id: id }).await?;
            emit_proposal("proposals.show", &proposal, json)
        }
        ProposalCommand::Decline { id } => {
            let proposal =
                decline_proposal(db.pool(), DeclineProposalInput { proposal_id: id }).await?;
            emit_proposal("proposals.decline", &proposal, json)
        }
    }
}

fn read_content(content: Option<String>, content_file: Option<PathBuf>) -> Result<String> {
    match (content, content_file) {
        (Some(content), None) => Ok(content),
        (None, Some(path)) => std::fs::read_to_string(&path)
            .map_err(|error| crate::Error::operation("read proposal content", error.to_string())),
        (Some(_), Some(_)) => Err(crate::Error::operation(
            "read proposal content",
            "pass either --content or --content-file, not both",
        )),
        (None, None) => Err(crate::Error::operation(
            "read proposal content",
            "pass --content or --content-file",
        )),
    }
}

fn emit_proposal(command: &'static str, proposal: &Proposal, json: bool) -> Result<()> {
    if json {
        output::emit(&output::json(command, proposal, None)?);
    } else {
        output::emit(&render_proposal(proposal));
    }
    Ok(())
}

fn render_list(proposals: &[Proposal]) -> String {
    let mut lines =
        vec!["STATUS     KIND              MEETING                         ID".to_string()];
    for proposal in proposals {
        lines.push(format!(
            "{:<10} {:<16} {:<30} {}",
            truncate(&proposal.status, 10),
            truncate(&proposal.kind, 16),
            truncate(&proposal.meeting_id, 30),
            proposal.id
        ));
    }
    lines.join("\n")
}

fn render_proposal(proposal: &Proposal) -> String {
    format!(
        "ID: {}\nMeeting: {}\nKind: {}\nTarget: {}\nStatus: {}\nSource: {}\nCreated: {}\n\n{}",
        proposal.id,
        proposal.meeting_id,
        proposal.kind,
        proposal.target_id,
        proposal.status,
        proposal.source,
        proposal.created_at,
        proposal.diff.trim_end()
    )
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    let mut text = value
        .chars()
        .take(width.saturating_sub(1))
        .collect::<String>();
    text.push('…');
    text
}
