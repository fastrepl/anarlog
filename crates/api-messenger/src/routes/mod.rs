pub(crate) mod messenger;

use axum::{
    Router,
    routing::{get, post},
};

pub fn router() -> Router {
    Router::new()
        .route("/slack/channels", get(messenger::list_slack_channels))
        .route("/slack/messages", post(messenger::send_slack_message))
}
