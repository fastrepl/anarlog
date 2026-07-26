mod error;
mod google;
mod openapi;

use axum::Router;

pub use openapi::openapi;

pub fn router() -> Router {
    Router::new().nest("/google", google::router())
}
