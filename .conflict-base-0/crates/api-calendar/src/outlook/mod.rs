pub(crate) mod routes;

use axum::{Router, routing::post};

pub fn router() -> Router {
    Router::new()
        .route("/list-calendars", post(routes::list_calendars))
        .route("/list-events", post(routes::list_events))
}
