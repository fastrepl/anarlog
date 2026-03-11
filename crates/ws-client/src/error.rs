#[derive(thiserror::Error)]
pub enum Error {
    #[error("unknown error")]
    Unknown,
    #[error("invalid request: {message}")]
    InvalidRequest { message: String },
    #[error("connect timeout on attempt {attempt}/{max_attempts}")]
    ConnectTimeout { attempt: usize, max_attempts: usize },
    #[error("connect failed on attempt {attempt}/{max_attempts}: {message}")]
    ConnectFailed {
        attempt: usize,
        max_attempts: usize,
        message: String,
        is_auth: bool,
        status_code: Option<u16>,
        retryable: bool,
    },
    #[error("connect retries exhausted after {attempts} attempts: {last_error}")]
    ConnectRetriesExhausted { attempts: usize, last_error: String },
    #[error("remote closed websocket{code_suffix}: {reason}")]
    RemoteClosed {
        code: Option<u16>,
        code_suffix: String,
        reason: String,
    },
    #[error("{}", format_connection_error(.0))]
    Connection(tokio_tungstenite::tungstenite::Error),
    #[error("message parse error: {message}")]
    ParseError { message: String },
    #[error("timeout error")]
    Timeout(tokio::time::error::Elapsed),
    #[error("send error")]
    SendError(String),
}

fn format_connection_error(e: &tokio_tungstenite::tungstenite::Error) -> String {
    if let tokio_tungstenite::tungstenite::Error::Http(response) = e {
        let status = response.status();
        let body_str = response
            .body()
            .as_ref()
            .and_then(|b| std::str::from_utf8(b).ok())
            .unwrap_or("");
        return format!("HTTP {} - {}", status, body_str);
    }
    format!("{:?}", e)
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unknown => write!(f, "Unknown"),
            Error::InvalidRequest { message } => write!(f, "InvalidRequest({message})"),
            Error::ConnectTimeout {
                attempt,
                max_attempts,
            } => write!(f, "ConnectTimeout({attempt}/{max_attempts})"),
            Error::ConnectFailed {
                attempt,
                max_attempts,
                message,
                is_auth,
                status_code,
                retryable,
            } => {
                write!(
                    f,
                    "ConnectFailed({attempt}/{max_attempts}, auth={is_auth}, status={status_code:?}, retryable={retryable}, {message})"
                )
            }
            Error::ConnectRetriesExhausted {
                attempts,
                last_error,
            } => write!(f, "ConnectRetriesExhausted({attempts}, {last_error})"),
            Error::RemoteClosed { code, reason, .. } => {
                write!(f, "RemoteClosed(code={code:?}, reason={reason})")
            }
            Error::Connection(e) => write!(f, "Connection({})", format_connection_error(e)),
            Error::ParseError { message } => write!(f, "ParseError({message})"),
            Error::Timeout(e) => write!(f, "Timeout({:?})", e),
            Error::SendError(message) => write!(f, "SendError({message})"),
        }
    }
}

impl Error {
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::InvalidRequest {
            message: message.into(),
        }
    }

    pub fn is_auth_error(&self) -> bool {
        if let Error::ConnectFailed { is_auth, .. } = self {
            return *is_auth;
        }

        if let Error::Connection(tungstenite_error) = self
            && let tokio_tungstenite::tungstenite::Error::Http(response) = tungstenite_error
        {
            let status = response.status().as_u16();
            return status == 401 || status == 403;
        }
        false
    }

    pub fn is_retryable_connect_error(&self) -> bool {
        match self {
            Error::InvalidRequest { .. } => false,
            Error::ConnectTimeout { .. } => true,
            Error::ConnectFailed { retryable, .. } => *retryable,
            Error::Connection(tungstenite_error) => {
                if let tokio_tungstenite::tungstenite::Error::Http(response) = tungstenite_error {
                    return is_retryable_http_status(response.status().as_u16());
                }

                true
            }
            Error::ConnectRetriesExhausted { .. }
            | Error::Unknown
            | Error::RemoteClosed { .. }
            | Error::ParseError { .. }
            | Error::Timeout(_)
            | Error::SendError(_) => false,
        }
    }

    pub fn connect_retries_exhausted(attempts: usize, last_error: impl Into<String>) -> Self {
        Self::ConnectRetriesExhausted {
            attempts,
            last_error: last_error.into(),
        }
    }

    pub fn connect_timeout(attempt: usize, max_attempts: usize) -> Self {
        Self::ConnectTimeout {
            attempt,
            max_attempts,
        }
    }

    pub fn connect_failed(
        attempt: usize,
        max_attempts: usize,
        error: &tokio_tungstenite::tungstenite::Error,
    ) -> Self {
        Self::ConnectFailed {
            attempt,
            max_attempts,
            message: format_connection_error(error),
            is_auth: is_http_auth_error(error),
            status_code: http_status(error),
            retryable: is_retryable_handshake_error(error),
        }
    }

    pub fn remote_closed(code: Option<u16>, reason: impl Into<String>) -> Self {
        let code_suffix = code
            .map(|code| format!(" with code {code}"))
            .unwrap_or_default();

        Self::RemoteClosed {
            code,
            code_suffix,
            reason: reason.into(),
        }
    }

    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::ParseError {
            message: message.into(),
        }
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for Error {
    fn from(value: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::Connection(value)
    }
}

impl From<tokio::time::error::Elapsed> for Error {
    fn from(value: tokio::time::error::Elapsed) -> Self {
        Self::Timeout(value)
    }
}

impl From<tokio::sync::mpsc::error::SendError<()>> for Error {
    fn from(value: tokio::sync::mpsc::error::SendError<()>) -> Self {
        Self::SendError(value.to_string())
    }
}

fn is_http_auth_error(error: &tokio_tungstenite::tungstenite::Error) -> bool {
    if let tokio_tungstenite::tungstenite::Error::Http(response) = error {
        let status = response.status().as_u16();
        return status == 401 || status == 403;
    }

    false
}

fn http_status(error: &tokio_tungstenite::tungstenite::Error) -> Option<u16> {
    if let tokio_tungstenite::tungstenite::Error::Http(response) = error {
        return Some(response.status().as_u16());
    }

    None
}

fn is_retryable_http_status(status: u16) -> bool {
    matches!(status, 408 | 425 | 429) || (500..=599).contains(&status)
}

fn is_retryable_handshake_error(error: &tokio_tungstenite::tungstenite::Error) -> bool {
    use tokio_tungstenite::tungstenite::Error as TungsteniteError;
    use tokio_tungstenite::tungstenite::error::ProtocolError;

    match error {
        TungsteniteError::Io(_) => true,
        TungsteniteError::Http(response) => is_retryable_http_status(response.status().as_u16()),
        TungsteniteError::Protocol(ProtocolError::HandshakeIncomplete) => true,
        TungsteniteError::Tls(_)
        | TungsteniteError::Capacity(_)
        | TungsteniteError::Protocol(_)
        | TungsteniteError::WriteBufferFull(_)
        | TungsteniteError::Utf8(_)
        | TungsteniteError::AttackAttempt
        | TungsteniteError::Url(_)
        | TungsteniteError::HttpFormat(_)
        | TungsteniteError::ConnectionClosed
        | TungsteniteError::AlreadyClosed => false,
    }
}

#[cfg(test)]
mod tests {
    use super::Error;
    use tokio_tungstenite::tungstenite::{Error as TungsteniteError, error::ProtocolError};

    #[test]
    fn handshake_incomplete_is_retryable() {
        let error = Error::connect_failed(
            1,
            3,
            &TungsteniteError::Protocol(ProtocolError::HandshakeIncomplete),
        );

        assert!(error.is_retryable_connect_error());
    }

    #[test]
    fn other_protocol_handshake_errors_are_not_retryable() {
        let error = Error::connect_failed(
            1,
            3,
            &TungsteniteError::Protocol(ProtocolError::WrongHttpMethod),
        );

        assert!(!error.is_retryable_connect_error());
    }
}
