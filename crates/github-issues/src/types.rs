use serde::{Deserialize, Serialize};

#[cfg(feature = "specta")]
use specta::Type;

#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

// === Request types ===

#[derive(Debug, Clone, Default)]
pub struct ListIssuesRequest {
    pub owner: String,
    pub repo: String,
    pub state: Option<IssueStateFilter>,
    pub labels: Option<Vec<String>>,
    pub assignee: Option<String>,
    pub sort: Option<String>,
    pub direction: Option<String>,
    pub per_page: Option<u32>,
    pub page: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct ListReposRequest {
    pub per_page: Option<u32>,
    pub page: Option<u32>,
    pub sort: Option<String>,
}

// === Enums ===

#[derive(Debug, Clone, Copy)]
pub enum IssueStateFilter {
    Open,
    Closed,
    All,
}

impl IssueStateFilter {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
            Self::All => "all",
        }
    }
}

// === Response types ===

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct Issue {
    pub id: u64,
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    pub state: String,
    #[serde(default)]
    pub state_reason: Option<String>,
    pub html_url: String,
    pub url: String,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub assignees: Option<Vec<User>>,
    #[serde(default)]
    pub labels: Option<Vec<Label>>,
    #[serde(default)]
    pub milestone: Option<Milestone>,
    #[serde(default)]
    pub pull_request: Option<IssuePullRequest>,
    #[serde(default)]
    pub locked: Option<bool>,
    #[serde(default)]
    pub comments: Option<u64>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub closed_at: Option<String>,
    #[serde(default)]
    pub closed_by: Option<User>,
    #[serde(default)]
    pub draft: Option<bool>,
}

impl Issue {
    pub fn is_pull_request(&self) -> bool {
        self.pull_request.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct IssuePullRequest {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub diff_url: Option<String>,
    #[serde(default)]
    pub patch_url: Option<String>,
    #[serde(default)]
    pub merged_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct User {
    pub id: u64,
    pub login: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    pub html_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct Label {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct Milestone {
    pub id: u64,
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct Repository {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub html_url: String,
    #[serde(default)]
    pub description: Option<String>,
    pub private: bool,
    pub owner: User,
}
