mod client;
mod error;
mod types;

#[cfg(feature = "utoipa")]
pub mod openapi;

pub use client::GoogleMailClient;
pub use error::Error;
pub use types::*;
