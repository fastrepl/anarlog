use rmcp::{ErrorData as McpError, model::*};

use crate::state::AppState;

pub(crate) async fn search(
    state: &AppState,
    params: anlg_exa::SearchRequest,
) -> Result<CallToolResult, McpError> {
    let response = state
        .exa
        .search(params)
        .await
        .map_err(|e: anlg_exa::Error| McpError::internal_error(e.to_string(), None))?;

    Ok(CallToolResult::success(vec![ContentBlock::text(
        serde_json::to_string(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?,
    )]))
}
