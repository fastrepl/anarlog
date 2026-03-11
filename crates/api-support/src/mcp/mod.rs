mod prompts;
mod server;
mod tools;

use crate::state::AppState;

use server::SupportMcpServer;

pub(crate) fn mcp_service(
    state: AppState,
) -> rmcp::transport::streamable_http_server::StreamableHttpService<SupportMcpServer> {
    hypr_mcp::create_service(move || Ok(SupportMcpServer::new(state.clone())))
}
