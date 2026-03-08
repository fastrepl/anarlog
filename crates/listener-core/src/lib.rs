pub mod actors;
mod events;
mod runtime;

pub use events::*;
pub use runtime::*;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum State {
    Active,
    Inactive,
    Finalizing,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type")]
pub enum DegradedError {
    #[serde(rename = "authentication_failed")]
    AuthenticationFailed { provider: String },
    #[serde(rename = "upstream_unavailable")]
    UpstreamUnavailable { message: String },
    #[serde(rename = "connection_timeout")]
    ConnectionTimeout,
    #[serde(rename = "stream_error")]
    StreamError { message: String },
}
