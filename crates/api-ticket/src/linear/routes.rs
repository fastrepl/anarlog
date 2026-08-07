use anlg_api_auth::AuthContext;
use anlg_api_nango::{Linear, NangoConnectionState, NangoIntegrationId};
use anlg_linear::LinearClient;
use anlg_ticket_interface::{CollectionPage, CollectionRef, TicketPage, TicketSummary};
use axum::{Extension, Json};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::error::{Result, TicketError};
use crate::normalize::linear_issue_to_ticket;

#[derive(Debug, Deserialize, ToSchema)]
pub struct LinearListTeamsRequest {
    pub connection_id: String,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LinearListTicketsRequest {
    pub connection_id: String,
    pub team_id: String,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub cursor: Option<String>,
}

#[utoipa::path(
    post,
    path = "/linear/list-teams",
    operation_id = "linear_list_teams",
    request_body = LinearListTeamsRequest,
    responses(
        (status = 200, description = "Linear teams fetched", body = CollectionPage),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "ticket",
)]
pub async fn list_teams(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<LinearListTeamsRequest>,
) -> Result<Json<CollectionPage>> {
    let http = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            Linear::ID,
            &req.connection_id,
        )
        .await?;

    let client = LinearClient::new(http);

    let teams = client
        .list_teams(anlg_linear::ListTeamsRequest {
            first: req.limit,
            after: req.cursor,
        })
        .await
        .map_err(|e| TicketError::Internal(e.to_string()))?;

    let next_cursor = if teams.page_info.has_next_page {
        teams.page_info.end_cursor
    } else {
        None
    };

    let items = teams
        .nodes
        .into_iter()
        .map(|t| CollectionRef {
            id: t.id,
            name: t.name,
            key: Some(t.key),
            url: None,
        })
        .collect();

    Ok(Json(CollectionPage { items, next_cursor }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LinearCreateIssueRequest {
    pub connection_id: String,
    pub team_id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[utoipa::path(
    post,
    path = "/linear/create-issue",
    operation_id = "linear_create_issue",
    request_body = LinearCreateIssueRequest,
    responses(
        (status = 200, description = "Linear issue created", body = TicketSummary),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "ticket",
)]
pub async fn create_issue(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<LinearCreateIssueRequest>,
) -> Result<Json<TicketSummary>> {
    let http = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            Linear::ID,
            &req.connection_id,
        )
        .await?;

    let client = LinearClient::new(http);

    let issue = client
        .create_issue(anlg_linear::CreateIssueRequest {
            team_id: req.team_id,
            title: req.title,
            description: req.description,
        })
        .await
        .map_err(|e| TicketError::Internal(e.to_string()))?;

    let collection = CollectionRef {
        id: issue.team.id.clone(),
        name: issue.team.name.clone(),
        key: Some(issue.team.key.clone()),
        url: None,
    };

    Ok(Json(linear_issue_to_ticket(&issue, &collection)))
}

#[utoipa::path(
    post,
    path = "/linear/list-tickets",
    operation_id = "linear_list_tickets",
    request_body = LinearListTicketsRequest,
    responses(
        (status = 200, description = "Linear tickets fetched", body = TicketPage),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "ticket",
)]
pub async fn list_tickets(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<LinearListTicketsRequest>,
) -> Result<Json<TicketPage>> {
    let http = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            Linear::ID,
            &req.connection_id,
        )
        .await?;

    let client = LinearClient::new(http);

    let issues = client
        .list_issues(anlg_linear::ListIssuesRequest {
            team_id: Some(req.team_id.clone()),
            first: req.limit,
            after: req.cursor,
            query: req.query,
        })
        .await
        .map_err(|e| TicketError::Internal(e.to_string()))?;

    let next_cursor = if issues.page_info.has_next_page {
        issues.page_info.end_cursor
    } else {
        None
    };

    // Build collection ref from the first issue's team, or from the request team_id.
    let collection = issues
        .nodes
        .first()
        .map(|i| CollectionRef {
            id: i.team.id.clone(),
            name: i.team.name.clone(),
            key: Some(i.team.key.clone()),
            url: None,
        })
        .unwrap_or_else(|| CollectionRef {
            id: req.team_id,
            name: String::new(),
            key: None,
            url: None,
        });

    let items = issues
        .nodes
        .iter()
        .map(|issue| linear_issue_to_ticket(issue, &collection))
        .collect();

    Ok(Json(TicketPage { items, next_cursor }))
}
