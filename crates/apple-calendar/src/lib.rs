#[cfg(target_os = "macos")]
mod apple;

#[cfg(feature = "fixture")]
pub mod fixture;

mod error;
pub mod types;

pub use error::{Error, Result};

#[cfg(target_os = "macos")]
pub use apple::{ContactFetcher, Handle, setup_change_notification};
