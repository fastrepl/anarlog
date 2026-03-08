use askama::Template;
use rmcp::{ErrorData as McpError, model::*};

#[derive(Template, Default)]
#[template(path = "research_chat.md.jinja")]
struct ResearchChatPrompt;

pub(crate) fn research_chat() -> Result<GetPromptResult, McpError> {
    hypr_mcp::render_prompt::<ResearchChatPrompt>("System prompt for the Char research chat")
}
