use askama::Template;
use rmcp::{ErrorData as McpError, model::*};

pub fn render_prompt<T: Template + Default>(
    description: &str,
) -> Result<GetPromptResult, McpError> {
    let content = T::default()
        .render()
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    Ok(GetPromptResult {
        description: Some(description.to_string()),
        messages: vec![PromptMessage::new_text(
            PromptMessageRole::Assistant,
            content,
        )],
    })
}
