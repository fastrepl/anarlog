mod contacts;
mod error;
mod openapi;
mod providers;
mod routes;

use axum::{Router, routing::post};

pub use contacts::{CrmContact, CrmContactQuery};
pub use openapi::openapi;
pub use providers::CrmProvider;

pub fn router() -> Router {
    Router::new().route("/search-contacts", post(routes::search_contacts))
}
