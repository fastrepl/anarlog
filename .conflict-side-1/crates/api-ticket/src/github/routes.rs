use axum::{Extension, Json};
use hypr_api_auth::AuthContext;
use hypr_api_nango::{GitHub, NangoConnectionState, NangoIntegrationId};
use hypr_github_issues::{GitHubIssuesClient, IssueStateFilter};
use hypr_ticket_interface::{CollectionPage, CollectionRef, TicketPage};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::error::{Result, TicketError};
use crate::normalize::github_issue_to_ticket;

#[derive(Debug, Deserialize, ToSchema)]
pub struct GitHubListReposRequest {
    pub connection_id: String,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GitHubListTicketsRequest {
    pub connection_id: String,
    pub owner: String,
    pub repo: String,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub labels: Option<Vec<String>>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub cursor: Option<String>,
}

#[utoipa::path(
    post,
    path = "/github/list-repos",
    operation_id = "github_list_repos",
    request_body = GitHubListReposRequest,
    responses(
        (status = 200, description = "GitHub repositories fetched", body = CollectionPage),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "ticket",
)]
pub async fn list_repos(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<GitHubListReposRequest>,
) -> Result<Json<CollectionPage>> {
    let http = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            GitHub::ID,
            &req.connection_id,
        )
        .await?;

    let client = GitHubIssuesClient::new(http);

    let page: u32 = req
        .cursor
        .as_deref()
        .and_then(|c| c.parse().ok())
        .unwrap_or(1);
    let per_page = req.limit.unwrap_or(30);

    let repos = client
        .list_repos(hypr_github_issues::ListReposRequest {
            per_page: Some(per_page),
            page: Some(page),
            sort: Some("updated".to_string()),
        })
        .await
        .map_err(|e| TicketError::Internal(e.to_string()))?;

    let next_cursor = if repos.len() as u32 >= per_page {
        Some((page + 1).to_string())
    } else {
        None
    };

    let items = repos
        .into_iter()
        .map(|r| CollectionRef {
            id: r.id.to_string(),
            name: r.full_name.clone(),
            key: Some(r.full_name),
            url: Some(r.html_url),
        })
        .collect();

    Ok(Json(CollectionPage { items, next_cursor }))
}

#[utoipa::path(
    post,
    path = "/github/list-tickets",
    operation_id = "github_list_tickets",
    request_body = GitHubListTicketsRequest,
    responses(
        (status = 200, description = "GitHub tickets fetched", body = TicketPage),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "ticket",
)]
pub async fn list_tickets(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<GitHubListTicketsRequest>,
) -> Result<Json<TicketPage>> {
    let http = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            GitHub::ID,
            &req.connection_id,
        )
        .await?;

    let client = GitHubIssuesClient::new(http);

    let page: u32 = req
        .cursor
        .as_deref()
        .and_then(|c| c.parse().ok())
        .unwrap_or(1);
    let per_page = req.limit.unwrap_or(30);

    let state_filter = match req.state.as_deref() {
        Some("open") => Some(IssueStateFilter::Open),
        Some("closed") => Some(IssueStateFilter::Closed),
        Some("all") => Some(IssueStateFilter::All),
        _ => Some(IssueStateFilter::All),
    };

    let issues = client
        .list_issues(hypr_github_issues::ListIssuesRequest {
            owner: req.owner.clone(),
            repo: req.repo.clone(),
            state: state_filter,
            labels: req.labels,
            per_page: Some(per_page),
            page: Some(page),
            ..Default::default()
        })
        .await
        .map_err(|e| TicketError::Internal(e.to_string()))?;

    let next_cursor = if issues.len() as u32 >= per_page {
        Some((page + 1).to_string())
    } else {
        None
    };

    let collection = CollectionRef {
        id: format!("{}/{}", req.owner, req.repo),
        name: format!("{}/{}", req.owner, req.repo),
        key: Some(format!("{}/{}", req.owner, req.repo)),
        url: Some(format!("https://github.com/{}/{}", req.owner, req.repo)),
    };

    let items = issues
        .iter()
        .map(|issue| github_issue_to_ticket(issue, &collection))
        .collect();

    Ok(Json(TicketPage { items, next_cursor }))
}
