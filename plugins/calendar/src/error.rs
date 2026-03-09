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
    #[error("auth plugin error: {0}")]
    Auth(String),
    #[error("api error: {0}")]
    Api(String),
    #[error("apple calendar error: {0}")]
    Apple(String),
}

impl serde::Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl specta::Type for Error {
    fn inline(_type_map: &mut specta::TypeMap, _generics: specta::Generics) -> specta::DataType {
        specta::DataType::Primitive(specta::datatype::PrimitiveType::String)
    }
}
