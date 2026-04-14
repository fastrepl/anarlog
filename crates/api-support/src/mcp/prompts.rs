use rmcp::{ErrorData as McpError, model::*};

pub(crate) fn support_chat() -> Result<GetPromptResult, McpError> {
    hypr_template_support::render_support_chat()
        .map_err(|e| McpError::internal_error(e.to_string(), None))
        .map(|content| {
            GetPromptResult::new(vec![PromptMessage::new_text(
                PromptMessageRole::Assistant,
                content,
            )])
            .with_description("System prompt for the Char support chat")
        })
}
