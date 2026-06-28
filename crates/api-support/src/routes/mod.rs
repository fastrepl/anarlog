pub(crate) mod chatwoot;
pub(crate) mod feedback;

use axum::{
    Router,
    routing::{get, post},
};

use crate::config::SupportConfig;
use crate::mcp::mcp_service;
use crate::state::AppState;

pub use feedback::{FeedbackRequest, FeedbackResponse};

pub async fn router(config: SupportConfig) -> Router {
    let state = AppState::new(config);
    let mcp = mcp_service(state.clone());

    let chatwoot_routes = Router::new()
        .route("/contact", post(chatwoot::create_contact))
        .route("/webhook", post(chatwoot::webhook))
        .route("/callback", post(chatwoot::callback))
        .route(
            "/conversations",
            post(chatwoot::create_conversation).get(chatwoot::list_conversations),
        )
        .route(
            "/conversations/{conversation_id}/messages",
            post(chatwoot::send_message).get(chatwoot::get_messages),
        )
        .route(
            "/conversations/{conversation_id}/events",
            get(chatwoot::conversation_events),
        );

    Router::new()
        .nest(
            "/feedback",
            Router::new().route("/submit", post(feedback::submit)),
        )
        .nest("/support", Router::new().nest_service("/mcp", mcp))
        .nest("/support/chatwoot", chatwoot_routes)
        .with_state(state)
}
