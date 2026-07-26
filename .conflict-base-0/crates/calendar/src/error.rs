use hypr_calendar_interface::CalendarProviderType;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not authenticated")]
    NotAuthenticated,
    #[error("provider {provider:?} is not available on this platform")]
    ProviderUnavailable { provider: CalendarProviderType },
    #[error("operation '{operation}' is not supported for provider {provider:?}")]
    UnsupportedOperation {
        operation: &'static str,
        provider: CalendarProviderType,
    },
    #[error("invalid datetime for field '{field}': {value}")]
    InvalidDateTime { field: &'static str, value: String },
    #[error("invalid auth header: {0}")]
    InvalidAuthHeader(#[from] reqwest::header::InvalidHeaderValue),
    #[error("http client error: {0}")]
    HttpClient(#[from] reqwest::Error),
    #[error("api error: {0}")]
    Api(String),
    #[error("apple calendar error: {0}")]
    Apple(String),
}
