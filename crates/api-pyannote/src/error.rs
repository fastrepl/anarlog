use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub type Result<T> = std::result::Result<T, PyannoteError>;

#[derive(Debug)]
pub struct PyannoteError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl PyannoteError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.into(),
        }
    }

    pub fn bad_gateway(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            code: "upstream_error",
            message: message.into(),
        }
    }

    pub fn upstream(status: StatusCode, message: impl Into<String>) -> Self {
        let code = match status {
            StatusCode::BAD_REQUEST => "bad_request",
            StatusCode::PAYMENT_REQUIRED => "subscription_required",
            StatusCode::TOO_MANY_REQUESTS => "rate_limited",
            StatusCode::NOT_FOUND => "not_found",
            _ if status.is_server_error() => "upstream_error",
            _ => "request_failed",
        };

        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

impl IntoResponse for PyannoteError {
    fn into_response(self) -> Response {
        hypr_api_error::error_response(self.status, self.code, &self.message)
    }
}
