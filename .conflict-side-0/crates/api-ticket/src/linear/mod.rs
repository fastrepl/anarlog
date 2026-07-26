pub(crate) mod routes;

use axum::{Router, routing::post};

pub fn router() -> Router {
    Router::new()
        .route("/list-teams", post(routes::list_teams))
        .route("/list-tickets", post(routes::list_tickets))
}
