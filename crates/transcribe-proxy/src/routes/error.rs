use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub(crate) enum RouteError {
    MissingConfig(&'static str),
    Unauthorized(&'static str),
    BadRequest(String),
    NotFound(&'static str),
    BadGateway(String),
    Internal(String),
}

impl IntoResponse for RouteError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            Self::MissingConfig(m) => {
                tracing::error!(detail = m, "route_error_missing_config");
                (StatusCode::INTERNAL_SERVER_ERROR, m.into())
            }
            Self::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m.into()),
            Self::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            Self::NotFound(m) => (StatusCode::NOT_FOUND, m.into()),
            Self::BadGateway(m) => (StatusCode::BAD_GATEWAY, m),
            Self::Internal(m) => {
                tracing::error!(detail = %m, "route_error_internal");
                (StatusCode::INTERNAL_SERVER_ERROR, m)
            }
        };
        (status, msg).into_response()
    }
}

pub(crate) fn parse_async_provider(s: &str) -> Result<owhisper_client::Provider, RouteError> {
    match s {
        "soniox" => Ok(owhisper_client::Provider::Soniox),
        "deepgram" => Ok(owhisper_client::Provider::Deepgram),
        other => Err(RouteError::BadRequest(format!(
            "unsupported async provider: {other}"
        ))),
    }
}
