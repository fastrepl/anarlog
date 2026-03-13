mod error;
mod google;
mod openapi;
mod outlook;

use axum::Router;

pub use openapi::openapi;

pub fn router() -> Router {
    Router::new()
        .nest("/google", google::router())
        .nest("/outlook", outlook::router())
}
