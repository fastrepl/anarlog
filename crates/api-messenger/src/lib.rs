mod error;
mod openapi;
mod routes;

pub use error::{MessengerError, Result};
pub use openapi::openapi;
pub use routes::router;
