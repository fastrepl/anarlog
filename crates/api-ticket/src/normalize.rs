use hypr_ticket_interface::{
    CollectionRef, LabelRef, PersonRef, PullRequestDetail, TicketKind, TicketPriority,
    TicketProviderType, TicketState, TicketSummary,
};

pub fn github_issue_to_ticket(
    issue: &hypr_github_issues::Issue,
    collection: &CollectionRef,
) -> TicketSummary {
    let is_pr = issue.is_pull_request();
    let kind = if is_pr {
        TicketKind::PullRequest
    } else {
        TicketKind::Issue
    };

    let (state, state_detail) = github_state(issue);

    let pull_request = if is_pr {
        let pr = issue.pull_request.as_ref().unwrap();
        Some(PullRequestDetail {
            is_draft: issue.draft.unwrap_or(false),
            is_merged: pr.merged_at.is_some(),
            source_branch: None,
            target_branch: None,
            merged_at: pr.merged_at.clone(),
            merged_by: None,
        })
    } else {
        None
    };

    let author = issue.user.as_ref().map(github_user_to_person);
    let assignees = issue
        .assignees
        .as_ref()
        .map(|a| a.iter().map(github_user_to_person).collect())
        .unwrap_or_default();
    let labels = issue
        .labels
        .as_ref()
        .map(|l| l.iter().map(github_label_to_ref).collect())
        .unwrap_or_default();

    let raw = serde_json::to_string(issue).unwrap_or_default();

    TicketSummary {
        provider: TicketProviderType::GitHub,
        kind,
        id: issue.number.to_string(),
        number: Some(issue.number),
        collection: collection.clone(),
        title: issue.title.clone(),
        state,
        state_detail,
        priority: None,
        author,
        assignees,
        labels,
        url: issue.html_url.clone(),
        created_at: issue.created_at.clone(),
        updated_at: issue.updated_at.clone(),
        closed_at: issue.closed_at.clone(),
        pull_request,
        raw,
    }
}

fn github_state(issue: &hypr_github_issues::Issue) -> (TicketState, Option<String>) {
    match issue.state.as_str() {
        "open" => (TicketState::Open, Some("open".to_string())),
        "closed" => {
            if let Some(ref pr) = issue.pull_request {
                if pr.merged_at.is_some() {
                    return (TicketState::Done, Some("merged".to_string()));
                }
            }
            match issue.state_reason.as_deref() {
                Some("completed") => (TicketState::Done, Some("completed".to_string())),
                Some("not_planned") => (TicketState::Closed, Some("not_planned".to_string())),
                _ => (TicketState::Closed, Some("closed".to_string())),
            }
        }
        other => (TicketState::Open, Some(other.to_string())),
    }
}

fn github_user_to_person(user: &hypr_github_issues::User) -> PersonRef {
    PersonRef {
        id: Some(user.id.to_string()),
        name: Some(user.login.clone()),
        email: None,
        avatar_url: user.avatar_url.clone(),
    }
}

fn github_label_to_ref(label: &hypr_github_issues::Label) -> LabelRef {
    LabelRef {
        id: label.id.to_string(),
        name: label.name.clone(),
        color: label.color.clone(),
    }
}

pub fn linear_issue_to_ticket(
    issue: &hypr_linear::Issue,
    collection: &CollectionRef,
) -> TicketSummary {
    let (state, state_detail) = linear_state(issue);
    let priority = linear_priority(issue);

    let author = issue.creator.as_ref().map(linear_user_to_person);
    let assignees = issue
        .assignee
        .as_ref()
        .map(|a| vec![linear_user_to_person(a)])
        .unwrap_or_default();
    let labels = issue
        .labels
        .as_ref()
        .map(|l| l.nodes.iter().map(linear_label_to_ref).collect())
        .unwrap_or_default();

    let raw = serde_json::to_string(issue).unwrap_or_default();

    TicketSummary {
        provider: TicketProviderType::Linear,
        kind: TicketKind::Issue,
        id: issue.identifier.clone(),
        number: Some(issue.number as u64),
        collection: collection.clone(),
        title: issue.title.clone(),
        state,
        state_detail,
        priority,
        author,
        assignees,
        labels,
        url: issue.url.clone(),
        created_at: issue.created_at.clone(),
        updated_at: issue.updated_at.clone(),
        closed_at: issue
            .completed_at
            .clone()
            .or_else(|| issue.cancelled_at.clone()),
        pull_request: None,
        raw,
    }
}

fn linear_state(issue: &hypr_linear::Issue) -> (TicketState, Option<String>) {
    let state_name = issue.state.name.clone();
    let normalized = match issue.state.state_type.as_str() {
        "backlog" => TicketState::Backlog,
        "unstarted" => TicketState::Open,
        "started" => TicketState::InProgress,
        "completed" => TicketState::Done,
        "cancelled" | "canceled" => TicketState::Closed,
        _ => TicketState::Open,
    };
    (normalized, Some(state_name))
}

fn linear_priority(issue: &hypr_linear::Issue) -> Option<TicketPriority> {
    issue.priority.map(|p| match p as u32 {
        0 => TicketPriority::None,
        1 => TicketPriority::Urgent,
        2 => TicketPriority::High,
        3 => TicketPriority::Medium,
        4 => TicketPriority::Low,
        _ => TicketPriority::None,
    })
}

fn linear_user_to_person(user: &hypr_linear::LinearUser) -> PersonRef {
    PersonRef {
        id: Some(user.id.clone()),
        name: Some(user.name.clone()),
        email: user.email.clone(),
        avatar_url: user.avatar_url.clone(),
    }
}

fn linear_label_to_ref(label: &hypr_linear::LinearLabel) -> LabelRef {
    LabelRef {
        id: label.id.clone(),
        name: label.name.clone(),
        color: label.color.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_collection() -> CollectionRef {
        CollectionRef {
            id: "test".to_string(),
            name: "owner/repo".to_string(),
            key: Some("owner/repo".to_string()),
            url: Some("https://github.com/owner/repo".to_string()),
        }
    }

    #[test]
    fn github_open_issue_maps_to_open() {
        let issue = hypr_github_issues::Issue {
            id: 1,
            number: 42,
            title: "Test issue".to_string(),
            body: None,
            state: "open".to_string(),
            state_reason: None,
            html_url: "https://github.com/owner/repo/issues/42".to_string(),
            url: "https://api.github.com/repos/owner/repo/issues/42".to_string(),
            user: None,
            assignees: None,
            labels: None,
            milestone: None,
            pull_request: None,
            locked: None,
            comments: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            closed_at: None,
            closed_by: None,
            draft: None,
        };
        let ticket = github_issue_to_ticket(&issue, &make_collection());
        assert_eq!(ticket.kind, TicketKind::Issue);
        assert_eq!(ticket.state, TicketState::Open);
        assert!(ticket.pull_request.is_none());
    }

    #[test]
    fn github_merged_pr_maps_to_done() {
        let issue = hypr_github_issues::Issue {
            id: 2,
            number: 43,
            title: "Test PR".to_string(),
            body: None,
            state: "closed".to_string(),
            state_reason: None,
            html_url: "https://github.com/owner/repo/pull/43".to_string(),
            url: "https://api.github.com/repos/owner/repo/issues/43".to_string(),
            user: None,
            assignees: None,
            labels: None,
            milestone: None,
            pull_request: Some(hypr_github_issues::IssuePullRequest {
                url: None,
                html_url: None,
                diff_url: None,
                patch_url: None,
                merged_at: Some("2024-01-02T00:00:00Z".to_string()),
            }),
            locked: None,
            comments: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
            closed_at: Some("2024-01-02T00:00:00Z".to_string()),
            closed_by: None,
            draft: None,
        };
        let ticket = github_issue_to_ticket(&issue, &make_collection());
        assert_eq!(ticket.kind, TicketKind::PullRequest);
        assert_eq!(ticket.state, TicketState::Done);
        let pr = ticket.pull_request.unwrap();
        assert!(pr.is_merged);
    }

    #[test]
    fn github_closed_not_planned_maps_to_closed() {
        let issue = hypr_github_issues::Issue {
            id: 3,
            number: 44,
            title: "Won't fix".to_string(),
            body: None,
            state: "closed".to_string(),
            state_reason: Some("not_planned".to_string()),
            html_url: "https://github.com/owner/repo/issues/44".to_string(),
            url: "https://api.github.com/repos/owner/repo/issues/44".to_string(),
            user: None,
            assignees: None,
            labels: None,
            milestone: None,
            pull_request: None,
            locked: None,
            comments: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            closed_at: Some("2024-01-01T00:00:00Z".to_string()),
            closed_by: None,
            draft: None,
        };
        let ticket = github_issue_to_ticket(&issue, &make_collection());
        assert_eq!(ticket.state, TicketState::Closed);
    }

    #[test]
    fn linear_started_maps_to_in_progress() {
        let issue = hypr_linear::Issue {
            id: "abc".to_string(),
            identifier: "ENG-123".to_string(),
            number: 123.0,
            title: "Linear issue".to_string(),
            description: None,
            url: "https://linear.app/team/issue/ENG-123".to_string(),
            priority: Some(2.0),
            priority_label: Some("High".to_string()),
            state: hypr_linear::WorkflowState {
                id: "state-1".to_string(),
                name: "In Progress".to_string(),
                state_type: "started".to_string(),
            },
            assignee: None,
            creator: None,
            team: hypr_linear::TeamRef {
                id: "team-1".to_string(),
                name: "Engineering".to_string(),
                key: "ENG".to_string(),
            },
            labels: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            completed_at: None,
            cancelled_at: None,
        };
        let collection = CollectionRef {
            id: "team-1".to_string(),
            name: "Engineering".to_string(),
            key: Some("ENG".to_string()),
            url: None,
        };
        let ticket = linear_issue_to_ticket(&issue, &collection);
        assert_eq!(ticket.kind, TicketKind::Issue);
        assert_eq!(ticket.state, TicketState::InProgress);
        assert_eq!(ticket.priority, Some(TicketPriority::High));
        assert!(ticket.pull_request.is_none());
    }
}
