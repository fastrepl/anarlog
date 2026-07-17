mod error;
mod github;
mod linear;
mod normalize;
mod openapi;

use axum::Router;

pub use openapi::openapi;

pub fn router() -> Router {
    Router::new()
        .nest("/github", github::router())
        .nest("/linear", linear::router())
}
