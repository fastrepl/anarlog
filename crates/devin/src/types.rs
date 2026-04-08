use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    New,
    Creating,
    Claimed,
    Running,
    Exit,
    Error,
    Suspended,
    Resuming,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatusDetail {
    Working,
    WaitingForUser,
    WaitingForApproval,
    Finished,
    Inactivity,
    UserRequest,
    UsageLimitExceeded,
    OutOfCredits,
    OutOfQuota,
    NoQuotaAllocation,
    PaymentDeclined,
    OrgUsageLimitExceeded,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionOrigin {
    Webapp,
    Slack,
    Teams,
    Api,
    Linear,
    Jira,
    Scheduled,
    Cli,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPullRequest {
    pub pr_state: String,
    pub pr_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub acus_consumed: f64,
    pub created_at: i64,
    pub org_id: String,
    pub pull_requests: Vec<SessionPullRequest>,
    pub session_id: String,
    pub status: SessionStatus,
    pub tags: Vec<String>,
    pub updated_at: i64,
    pub url: String,
    #[serde(default)]
    pub child_session_ids: Option<Vec<String>>,
    #[serde(default)]
    pub is_advanced: bool,
    #[serde(default)]
    pub is_archived: bool,
    #[serde(default)]
    pub parent_session_id: Option<String>,
    #[serde(default)]
    pub playbook_id: Option<String>,
    #[serde(default)]
    pub service_user_id: Option<String>,
    #[serde(default)]
    pub status_detail: Option<SessionStatusDetail>,
    #[serde(default)]
    pub structured_output: Option<serde_json::Value>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListSessionsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_after: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_before: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_after: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_before: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playbook_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origins: Option<Vec<SessionOrigin>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_user_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListSessionMessagesRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub created_at: i64,
    pub event_id: String,
    pub message: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorPage<T> {
    pub items: Vec<T>,
    #[serde(default)]
    pub end_cursor: Option<String>,
    #[serde(default)]
    pub has_next_page: bool,
    #[serde(default)]
    pub total: Option<u64>,
}
