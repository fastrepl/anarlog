pub use client::AuthClient;
pub use error::{Error, Result};

mod client;
mod error;
mod response;

#[cfg(test)]
mod tests;
