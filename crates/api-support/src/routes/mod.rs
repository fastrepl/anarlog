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
    let resolver = hypr_llm_proxy::StaticModelResolver::default()
        .with_models(
            hypr_llm_proxy::MODEL_KEY_DEFAULT,
            vec![
                "openai/gpt-oss-120b".into(),
                "moonshotai/kimi-k2-0905".into(),
            ],
        )
        .with_models(
            hypr_llm_proxy::MODEL_KEY_TOOL_CALLING,
            vec![
                "anthropic/claude-haiku-4.5".into(),
                "moonshotai/kimi-k2-0905:exacto".into(),
            ],
        );
    let llm_config = hypr_llm_proxy::LlmProxyConfig::new(&config.openrouter)
        .with_model_resolver(std::sync::Arc::new(resolver));
    let llm_router = hypr_llm_proxy::router(llm_config);

    let state = AppState::new(config).await;
    let mcp = mcp_service(state.clone());

    let chatwoot_routes = Router::new()
        .route("/contact", post(chatwoot::create_contact))
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
        .nest_service("/support/llm", llm_router)
        .with_state(state)
}
