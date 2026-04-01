use axum::{Extension, Json};
use hypr_api_auth::AuthContext;
use hypr_api_nango::{Linear, NangoConnectionState, NangoIntegrationId};
use hypr_linear::LinearClient;
use hypr_ticket_interface::{CollectionPage, CollectionRef, TicketPage};
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
        .list_teams(hypr_linear::ListTeamsRequest {
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
        .list_issues(hypr_linear::ListIssuesRequest {
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
