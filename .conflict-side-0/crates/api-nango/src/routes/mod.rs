pub(crate) mod connect;
pub(crate) mod disconnect;
pub(crate) mod identity;
pub(crate) mod status;
pub(crate) mod webhook;
pub(crate) mod whoami;

use axum::{
    Router,
    routing::{get, post},
};

use crate::config::NangoConfig;
use crate::state::AppState;

pub use connect::{CreateSessionRequest, SessionMode, SessionResponse};
pub use disconnect::{DeleteConnectionRequest, DeleteConnectionResponse};
pub use status::{ConnectionItem, ListConnectionsResponse};
pub use webhook::{ForwardHandler, ForwardHandlerRegistry, WebhookResponse, forward_handler};
pub use whoami::{WhoAmIItem, WhoAmIResponse};

pub fn router(config: NangoConfig) -> Router {
    session_router(config.clone()).merge(management_router(config))
}

pub fn session_router(config: NangoConfig) -> Router {
    let state = AppState::new(config);

    Router::new()
        .route("/session", post(connect::create_session))
        .with_state(state)
}

pub fn management_router(config: NangoConfig) -> Router {
    let state = AppState::new(config);

    Router::new()
        .route(
            "/connections",
            get(status::list_connections).delete(disconnect::delete_connection),
        )
        .route("/whoami", get(whoami::whoami))
        .with_state(state)
}

pub fn webhook_router(config: NangoConfig, forward_handlers: ForwardHandlerRegistry) -> Router {
    let state = AppState::with_forward_handlers(config, forward_handlers);

    Router::new()
        .route("/webhook", post(webhook::nango_webhook))
        .with_state(state)
}
