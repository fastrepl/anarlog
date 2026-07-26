use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusReportStatus {
    Investigating,
    Identified,
    Monitoring,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusReport {
    pub id: i64,
    pub title: String,
    pub status: StatusReportStatus,
    #[serde(default)]
    pub status_report_update_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub monitor_ids: Vec<i64>,
    pub page_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusReportUpdate {
    #[serde(default)]
    pub id: Option<String>,
    pub status: StatusReportStatus,
    #[serde(default)]
    pub date: Option<String>,
    pub message: String,
    pub status_report_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Incident {
    pub id: i64,
    pub started_at: Option<String>,
    pub monitor_id: Option<i64>,
    pub acknowledged_at: Option<String>,
    pub acknowledged_by: Option<i64>,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStatusReportRequest {
    pub title: String,
    pub status: StatusReportStatus,
    pub page_id: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monitor_ids: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStatusReportUpdateRequest {
    pub status: StatusReportStatus,
    pub message: String,
    pub status_report_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateIncidentRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acknowledged_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<String>,
}
