pub use hypr_notification_interface::*;

#[cfg(target_os = "linux")]
mod r#impl;

#[cfg(target_os = "linux")]
pub use r#impl::{
    dismiss_all, setup_notification_accept_handler, setup_notification_confirm_handler,
    setup_notification_dismiss_handler, setup_notification_timeout_handler, show,
};
