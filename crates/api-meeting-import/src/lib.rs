mod error;
mod import;
mod openapi;
mod routes;

use axum::{Router, routing::post};

pub use openapi::openapi;

pub fn router() -> Router {
    Router::new()
        .route(
            "/fathom/import-meetings",
            post(routes::fathom_import_meetings),
        )
        .route(
            "/webex/import-meetings",
            post(routes::webex_import_meetings),
        )
        .route(
            "/google-meet/import-meetings",
            post(routes::google_meet_import_meetings),
        )
        .route(
            "/microsoft-teams/import-meetings",
            post(routes::teams_import_meetings),
        )
}
