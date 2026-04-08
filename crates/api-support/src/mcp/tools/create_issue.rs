use rmcp::{
    ErrorData as McpError,
    model::*,
    schemars::{self, JsonSchema},
};
use serde::Deserialize;

use crate::github;
use crate::redact;
use crate::state::AppState;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CreateIssueParams {
    #[schemars(description = "Concise issue title, e.g. 'Transcription stops after 30 minutes'")]
    pub title: String,
    #[schemars(
        description = "Issue body in markdown. For bugs: include steps to reproduce, expected vs actual behavior, and environment info. For features: describe the use case and desired behavior."
    )]
    pub body: String,
    #[schemars(
        description = "The issue type. Use 'Bug' for bugs, 'Feature' for feature requests."
    )]
    pub issue_type: Option<String>,
    #[schemars(
        description = "Optional GitHub labels to apply. Use existing repository labels when known; the server will validate them against the live repo labels and auto-select additional matching labels."
    )]
    pub labels: Option<Vec<String>>,
}

pub(crate) async fn create_issue(
    state: &AppState,
    params: CreateIssueParams,
) -> Result<CallToolResult, McpError> {
    let labels = github::resolve_issue_labels(
        state,
        &params.title,
        &params.body,
        params.labels.as_deref().unwrap_or(&[]),
    )
    .await
    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    let title = redact::redact_pii(&params.title);
    let body = redact::redact_pii(&params.body);

    let (url, number) =
        github::create_issue(state, &title, &body, &labels, params.issue_type.as_deref())
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::json!({
            "success": true,
            "issue_url": url,
            "issue_number": number,
            "labels": labels,
        })
        .to_string(),
    )]))
}
