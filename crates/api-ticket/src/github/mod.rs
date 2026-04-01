pub(crate) mod routes;

use axum::{Router, routing::post};

pub fn router() -> Router {
    Router::new()
        .route("/list-repos", post(routes::list_repos))
        .route("/list-tickets", post(routes::list_tickets))
}
