use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct CreateIssueParams {
    pub title: String,
    pub body: String,
    pub labels: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct CreateIssueOutput {
    pub success: bool,
    pub issue_url: String,
    pub issue_number: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct AddCommentParams {
    pub issue_number: u64,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct AddCommentOutput {
    pub success: bool,
    pub comment_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SearchIssuesParams {
    pub query: String,
    pub state: Option<String>,
    pub limit: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SearchIssueItem {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub url: String,
    pub created_at: String,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SearchIssuesOutput {
    pub total_results: usize,
    pub issues: Vec<SearchIssueItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ListSubscriptionsParams {
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct SubscriptionItem {
    pub id: String,
    pub status: String,
    pub start_date: Option<i64>,
    pub cancel_at_period_end: bool,
    pub cancel_at: Option<i64>,
    pub canceled_at: Option<i64>,
    pub trial_start: Option<i64>,
    pub trial_end: Option<i64>,
}

pub type ListSubscriptionsOutput = Vec<SubscriptionItem>;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct CreateBillingPortalSessionParams {
    pub return_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct CreateBillingPortalSessionOutput {
    pub url: String,
}
