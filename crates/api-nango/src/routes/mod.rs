pub(crate) mod connect;
pub(crate) mod disconnect;
pub(crate) mod status;
pub(crate) mod webhook;

use axum::{
    Router,
    routing::{get, post},
};

use crate::config::NangoConfig;
use crate::state::AppState;

pub use connect::{CreateSessionRequest, SessionResponse};
pub use disconnect::{DeleteConnectionRequest, DeleteConnectionResponse};
pub use status::{ConnectionItem, ListConnectionsResponse};
pub use webhook::WebhookResponse;

pub fn router(config: NangoConfig) -> Router {
    let state = AppState::new(config);

    Router::new()
        .route("/session", post(connect::create_session))
        .route(
            "/connections",
            get(status::list_connections).delete(disconnect::delete_connection),
        )
        .with_state(state)
}

pub fn webhook_router(config: NangoConfig) -> Router {
    let state = AppState::new(config);

    Router::new()
        .route("/webhook", post(webhook::nango_webhook))
        .with_state(state)
}
