pub(crate) mod routes;

use axum::{Router, routing::post};

pub fn router() -> Router {
    Router::new()
        .route("/list-labels", post(routes::list_labels))
        .route("/list-messages", post(routes::list_messages))
        .route("/get-message", post(routes::get_message))
        .route("/get-attachment", post(routes::get_attachment))
        .route("/get-profile", post(routes::get_profile))
        .route("/list-threads", post(routes::list_threads))
        .route("/get-thread", post(routes::get_thread))
        .route("/list-history", post(routes::list_history))
}
