mod prompts;
mod server;
mod tools;

use crate::state::AppState;

use server::ResearchMcpServer;

pub(crate) fn mcp_service(
    state: AppState,
) -> rmcp::transport::streamable_http_server::StreamableHttpService<ResearchMcpServer> {
    hypr_mcp::create_service(move || Ok(ResearchMcpServer::new(state.clone())))
}
