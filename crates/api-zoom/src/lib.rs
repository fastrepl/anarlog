mod error;
mod import;
mod openapi;
mod routes;

use axum::{Router, routing::post};

pub use openapi::openapi;

pub fn router() -> Router {
    Router::new().route("/import-meetings", post(routes::import_meetings))
}
