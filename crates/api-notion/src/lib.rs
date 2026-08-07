mod blocks;
mod error;
mod openapi;
mod routes;

pub use error::{NotionError, Result};
pub use openapi::openapi;
pub use routes::router;
