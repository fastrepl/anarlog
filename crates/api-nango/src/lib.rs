mod config;
mod error;
pub mod extractor;
pub mod integrations;
mod openapi;
mod routes;
mod state;
mod supabase;

pub use config::NangoConfig;
pub use extractor::{NangoConnection, NangoConnectionError, NangoConnectionState};
pub use integrations::{
    Discord, GitHub, GoogleCalendar, GoogleDrive, GoogleMail, Linear, NangoIntegrationId, Notion,
    Outlook, Slack,
};
pub use openapi::openapi;
pub use routes::{
    ForwardHandler, ForwardHandlerRegistry, forward_handler, management_router, router,
    session_router, webhook_router,
};
