use axum::Router;

use crate::config::ResearchConfig;
use crate::mcp::mcp_service;
use crate::state::AppState;

pub fn router(config: ResearchConfig) -> Router {
    let state = AppState::new(config);
    let mcp = mcp_service(state);

    Router::new().nest("/research", Router::new().nest_service("/mcp", mcp))
}
