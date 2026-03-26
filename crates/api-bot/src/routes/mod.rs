pub(crate) mod bot;
pub(crate) mod webhook;

use std::sync::Arc;

use axum::{
    Router,
    routing::{delete, get, post},
};

use hypr_recall::RecallClient;

use crate::config::BotConfig;
use crate::error::BotError;

pub fn router(config: BotConfig) -> Result<Router, BotError> {
    let client =
        RecallClient::new(&config.recall_api_key).map_err(|e| BotError::Internal(e.to_string()))?;

    let router = Router::new()
        .route("/bot", post(bot::send_bot))
        .route("/bot/:bot_id", delete(bot::remove_bot))
        .route("/onboarding/demo", post(bot::start_demo))
        .route("/onboarding/demo/:bot_id", get(bot::demo_status))
        .route("/onboarding/player", get(bot::player))
        .route("/webhook", post(webhook::status_change))
        .route("/webhook/transcript", post(webhook::transcript))
        .layer(axum::Extension(Arc::new(client)))
        .layer(axum::Extension(Arc::new(config)));

    Ok(router)
}
