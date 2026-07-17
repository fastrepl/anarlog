#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type,
)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum TicketProviderType {
    GitHub,
    Linear,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type,
)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum TicketKind {
    Issue,
    PullRequest,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type,
)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum TicketState {
    Backlog,
    Open,
    InProgress,
    Done,
    Closed,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type,
)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum TicketPriority {
    Urgent,
    High,
    Medium,
    Low,
    None,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct PersonRef {
    pub id: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct LabelRef {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct CollectionRef {
    pub id: String,
    /// Display name. GitHub: "owner/repo", Linear: team name.
    pub name: String,
    /// Short identifier. GitHub: "owner/repo", Linear: team key (e.g., "ENG").
    pub key: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct PullRequestDetail {
    pub is_draft: bool,
    pub is_merged: bool,
    pub source_branch: Option<String>,
    pub target_branch: Option<String>,
    pub merged_at: Option<String>,
    pub merged_by: Option<PersonRef>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct TicketSummary {
    pub provider: TicketProviderType,
    pub kind: TicketKind,

    /// Provider-specific unique ID.
    pub id: String,
    /// Numeric identifier (GitHub: number, Linear: number).
    pub number: Option<u64>,

    pub collection: CollectionRef,

    pub title: String,
    pub state: TicketState,
    /// Provider's original state string for display (e.g. "In Review", "merged").
    pub state_detail: Option<String>,
    pub priority: Option<TicketPriority>,

    pub author: Option<PersonRef>,
    pub assignees: Vec<PersonRef>,
    pub labels: Vec<LabelRef>,

    pub url: String,

    /// ISO 8601.
    pub created_at: String,
    /// ISO 8601.
    pub updated_at: String,
    /// ISO 8601.
    pub closed_at: Option<String>,

    /// Present only when kind == PullRequest.
    pub pull_request: Option<PullRequestDetail>,

    /// Raw provider JSON for forward compatibility.
    pub raw: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, specta::Type)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct TicketFilter {
    #[serde(default)]
    pub states: Option<Vec<TicketState>>,
    #[serde(default)]
    pub kinds: Option<Vec<TicketKind>>,
    #[serde(default)]
    pub assignee_id: Option<String>,
    #[serde(default)]
    pub labels: Option<Vec<String>>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct TicketPage {
    pub items: Vec<TicketSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct CollectionPage {
    pub items: Vec<CollectionRef>,
    pub next_cursor: Option<String>,
}
