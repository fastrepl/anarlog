use serde::{Serialize, ser::Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("bad request (400): {0}")]
    BadRequest(String),
    #[error("unauthorized (401): {0}")]
    Unauthorized(String),
    #[error("forbidden (403): {0}")]
    Forbidden(String),
    #[error("not found (404): {0}")]
    NotFound(String),
    #[error("method not allowed (405): {0}")]
    MethodNotAllowed(String),
    #[error("request too large (413): {0}")]
    RequestTooLarge(String),
    #[error("command failed (422): {0}")]
    CommandFailed(String),
    #[error("rate limited (429): {0}")]
    RateLimited(String),
    #[error("internal server error (500): {0}")]
    Internal(String),
    #[error("timeout (504): {0}")]
    Timeout(String),
    #[error("API error (status {0}): {1}")]
    Api(u16, String),

    #[error(transparent)]
    Request(#[from] reqwest::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("invalid signing key: {0}")]
    InvalidSigningKey(String),
    #[error("signing failed: {0}")]
    Signing(String),
    #[error("invalid permissions: {0}")]
    InvalidPermissions(&'static str),
    #[error("invalid vm name: {0}")]
    InvalidVmName(String),
    #[error("invalid api base url")]
    InvalidApiBase,
    #[error("invalid vm url")]
    InvalidVmUrl,
    #[error("missing token or signing key")]
    MissingToken,
}

impl Error {
    pub(crate) fn from_status(status: u16, body: String) -> Self {
        let message = extract_error_message(&body).unwrap_or(body);
        match status {
            400 => Error::BadRequest(message),
            401 => Error::Unauthorized(message),
            403 => Error::Forbidden(message),
            404 => Error::NotFound(message),
            405 => Error::MethodNotAllowed(message),
            413 => Error::RequestTooLarge(message),
            422 => Error::CommandFailed(message),
            429 => Error::RateLimited(message),
            500 => Error::Internal(message),
            504 => Error::Timeout(message),
            other => Error::Api(other, message),
        }
    }
}

fn extract_error_message(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("error")
        .and_then(|e| e.as_str())
        .map(|s| s.to_string())
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
