mod config;
mod error;
pub mod extractor;
pub mod integrations;
mod openapi;
mod routes;
mod state;
mod supabase;

pub use config::NangoConfig;
pub use extractor::{
    NangoConnection, NangoConnectionError, NangoConnectionState, is_provider_auth_failure,
};
pub use integrations::{
    Attio, Close, Discord, Fathom, GitHub, GoogleCalendar, GoogleDrive, GoogleMail, GoogleMeet,
    HubSpot, Linear, MicrosoftTeams, NangoIntegrationId, Notion, Outlook, Pipedrive, Salesforce,
    Slack, Webex, Zoom,
};
pub use openapi::openapi;
pub use routes::{
    ForwardHandler, ForwardHandlerRegistry, forward_handler, management_router, router,
    session_router, webhook_router,
};
