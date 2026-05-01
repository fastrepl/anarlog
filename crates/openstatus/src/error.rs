#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("openstatus api error: {0}")]
    ApiError(String),
    #[error("failed to list incidents: {0}")]
    ListIncidentsError(String),
    #[error("failed to get incident: {0}")]
    GetIncidentError(String),
    #[error("failed to update incident: {0}")]
    UpdateIncidentError(String),
    #[error("failed to list status reports: {0}")]
    ListStatusReportsError(String),
    #[error("failed to get status report: {0}")]
    GetStatusReportError(String),
    #[error("failed to create status report: {0}")]
    CreateStatusReportError(String),
    #[error("failed to delete status report: {0}")]
    DeleteStatusReportError(String),
    #[error("failed to create status report update: {0}")]
    CreateStatusReportUpdateError(String),
}
