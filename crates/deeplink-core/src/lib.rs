//! Deep-link parsing and the browser callback page shared by the Tauri
//! `deeplink2` plugin and the GPUI shell.

mod callback;
mod error;
mod types;

pub use callback::{
    parse_callback, redact_url, render_html, render_html_from_callback, subscription_auth_deeplink,
};
pub use error::{Error, Result};
pub use types::{
    AuthCallbackSearch, BillingRefreshSearch, DeepLink, IncomingDeepLink,
    IntegrationCallbackSearch, OnboardingDemoCompleteSearch, ShareOpenRequest,
};
