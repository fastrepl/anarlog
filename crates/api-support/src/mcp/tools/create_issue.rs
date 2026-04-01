use rmcp::{
    ErrorData as McpError,
    model::*,
    schemars::{self, JsonSchema},
};
use serde::Deserialize;

use crate::github;
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
}

pub(crate) async fn create_issue(
    state: &AppState,
    params: CreateIssueParams,
) -> Result<CallToolResult, McpError> {
    let labels = vec!["Engineering".to_string()];

    let (url, number) = github::create_issue(
        state,
        &params.title,
        &params.body,
        &labels,
        params.issue_type.as_deref(),
    )
    .await
    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::json!({
            "success": true,
            "issue_url": url,
            "issue_number": number,
        })
        .to_string(),
    )]))
}
