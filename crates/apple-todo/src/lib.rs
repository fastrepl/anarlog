#[cfg(target_os = "macos")]
mod apple;

mod error;
pub mod types;

pub use error::{Error, Result};
pub use types::ReadPathResult;

#[cfg(target_os = "macos")]
pub use apple::{Handle, ReminderAuthStatus, setup_change_notification};
