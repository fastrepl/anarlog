pub use anlg_notification_interface::*;

mod callbacks;
mod icon;
mod layout;

#[cfg(target_os = "windows")]
mod r#impl;

pub use callbacks::{
    setup_notification_accept_handler, setup_notification_confirm_handler,
    setup_notification_dismiss_handler, setup_notification_footer_action_handler,
    setup_notification_option_selected_handler, setup_notification_timeout_handler,
};

#[cfg(target_os = "windows")]
pub use r#impl::{dismiss_all, show};
