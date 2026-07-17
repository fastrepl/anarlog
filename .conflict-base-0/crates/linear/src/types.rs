use serde::{Deserialize, Serialize};

#[cfg(feature = "specta")]
use specta::Type;

#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

// === GraphQL response wrapper ===

#[derive(Debug, Clone, Deserialize)]
pub struct GraphQLResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GraphQLError {
    pub message: String,
}

// === Pagination ===

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection<T> {
    pub nodes: Vec<T>,
    pub page_info: PageInfo,
}

// === Request types ===

#[derive(Debug, Clone, Default)]
pub struct ListIssuesRequest {
    pub team_id: Option<String>,
    pub first: Option<u32>,
    pub after: Option<String>,
    pub query: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ListTeamsRequest {
    pub first: Option<u32>,
    pub after: Option<String>,
}

// === Response wrappers for GraphQL data field ===

#[derive(Debug, Clone, Deserialize)]
pub struct IssuesData {
    pub issues: Connection<Issue>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TeamIssuesData {
    pub team: TeamWithIssues,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TeamWithIssues {
    pub issues: Connection<Issue>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TeamsData {
    pub teams: Connection<Team>,
}

// === Core types ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: String,
    pub identifier: String,
    pub number: f64,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub url: String,
    #[serde(default)]
    pub priority: Option<f64>,
    #[serde(default)]
    pub priority_label: Option<String>,
    pub state: WorkflowState,
    #[serde(default)]
    pub assignee: Option<LinearUser>,
    #[serde(default)]
    pub creator: Option<LinearUser>,
    pub team: TeamRef,
    #[serde(default)]
    pub labels: Option<LabelConnection>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub cancelled_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct WorkflowState {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub state_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LinearUser {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct TeamRef {
    pub id: String,
    pub name: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LabelConnection {
    pub nodes: Vec<LinearLabel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LinearLabel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub id: String,
    pub name: String,
    pub key: String,
    #[serde(default)]
    pub description: Option<String>,
}
