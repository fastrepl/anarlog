pub(crate) mod notion;

use axum::{Router, routing::post};

pub fn router() -> Router {
    Router::new()
        .route("/search-pages", post(notion::search_pages))
        .route("/append-update", post(notion::append_update))
}
