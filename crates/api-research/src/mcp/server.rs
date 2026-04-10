use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, handler::server::tool::ToolRouter,
    handler::server::wrapper::Parameters, model::*, service::RequestContext, tool, tool_handler,
    tool_router,
};

use crate::state::AppState;

use super::prompts;
use super::tools;

#[derive(Clone)]
pub struct ResearchMcpServer {
    state: AppState,
    tool_router: ToolRouter<Self>,
}

impl ResearchMcpServer {
    pub(super) fn new(state: AppState) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl ResearchMcpServer {
    #[tool(
        description = "Search the web using Exa. Returns relevant results for a given query.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = true
        )
    )]
    async fn search(
        &self,
        Parameters(params): Parameters<hypr_exa::SearchRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::search(&self.state, params).await
    }

    #[tool(
        description = "Get the contents of web pages by URL. Returns the text content of the given URLs.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = true
        )
    )]
    async fn get_contents(
        &self,
        Parameters(params): Parameters<hypr_exa::GetContentsRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::get_contents(&self.state, params).await
    }

    #[tool(
        description = "Read a URL and convert it to clean, LLM-friendly markdown text. Powered by Jina Reader.",
        annotations(
            read_only_hint = true,
            destructive_hint = false,
            open_world_hint = true
        )
    )]
    async fn read_url(
        &self,
        Parameters(params): Parameters<hypr_jina::ReadUrlRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::read_url(&self.state, params).await
    }
}

#[tool_handler]
impl ServerHandler for ResearchMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .build(),
            server_info: Implementation {
                name: "char-research".to_string(),
                title: None,
                version: env!("CARGO_PKG_VERSION").to_string(),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "Char research server. Provides tools for web search and content retrieval powered by Exa."
                    .to_string(),
            ),
        }
    }

    async fn list_prompts(
        &self,
        _params: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult {
            prompts: vec![Prompt::new(
                "research_chat",
                Some("System prompt for the Char research chat"),
                None::<Vec<PromptArgument>>,
            )],
            next_cursor: None,
            meta: None,
        })
    }

    async fn get_prompt(
        &self,
        params: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        match params.name.as_str() {
            "research_chat" => prompts::research_chat(),
            _ => Err(McpError::invalid_params(
                format!("Unknown prompt: {}", params.name),
                None,
            )),
        }
    }
}
