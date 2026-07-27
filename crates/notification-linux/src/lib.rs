pub use hypr_notification_interface::*;

#[cfg(any(target_os = "linux", test))]
mod callbacks;

#[cfg(target_os = "linux")]
mod r#impl;

#[cfg(target_os = "linux")]
pub use callbacks::{
    setup_notification_accept_handler, setup_notification_confirm_handler,
    setup_notification_dismiss_handler, setup_notification_footer_action_handler,
    setup_notification_option_selected_handler, setup_notification_timeout_handler,
};

#[cfg(target_os = "linux")]
pub use r#impl::{dismiss_all, show};
