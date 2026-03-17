pub(crate) mod messenger;

use axum::{Router, routing::post};

pub fn router() -> Router {
    Router::new().route("/send", post(messenger::send_message))
}
