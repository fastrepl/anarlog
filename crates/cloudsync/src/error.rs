const SUPPORTED_CLOUDSYNC_TARGETS: &str = concat!(
    "macos/{aarch64,x86_64}, ",
    "ios (via bundled CloudSync.xcframework), ",
    "android/{arm64-v8a,armeabi-v7a,x86_64}, ",
    "linux/{gnu,musl}/{aarch64,x86_64}, ",
    "windows/x86_64"
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    /// Network timeout, connection drop, server pressure — retry with backoff.
    Transient,
    /// Credentials expired or invalid — stop syncing, surface to UI.
    Auth,
    /// TLS, bad URL, protocol mismatch, schema error — needs intervention.
    Fatal,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid cloudsync network response: {0}")]
    InvalidNetworkResponse(#[from] serde_json::Error),
    #[error("no cache directory is available for the bundled cloudsync extension")]
    MissingCacheDir,
    #[error("failed to register the cloudsync close hook: sqlite error {0}")]
    CloseHookRegistration(i32),
    #[error("failed to register the cloudsync transaction observer: sqlite error {0}")]
    TransactionObserverRegistration(i32),
    #[error("CloudSync requires WAL journal mode for commit-safe change notifications")]
    WalRequired,
    #[error(
        "the bundled cloudsync extension is not available for this target; supported targets: {SUPPORTED_CLOUDSYNC_TARGETS}"
    )]
    UnsupportedBundledCloudsync,
}

impl Error {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Sqlx(sqlx_err) => {
                if let Some(db_err) = sqlx_err.as_database_error() {
                    return classify_database_error(db_err.code().as_deref(), db_err.message());
                }
                return classify_error_message(&sqlx_err.to_string()).unwrap_or(ErrorKind::Fatal);
            }
            Self::Io(error) => return classify_io_error(error),
            _ => {}
        }
        ErrorKind::Fatal
    }
}

fn classify_database_error(code: Option<&str>, message: &str) -> ErrorKind {
    if let Some(kind) = classify_error_message(message) {
        return kind;
    }

    code.and_then(|code| code.parse().ok())
        .map_or(ErrorKind::Fatal, classify_error_code)
}

fn classify_io_error(error: &std::io::Error) -> ErrorKind {
    match error.kind() {
        std::io::ErrorKind::BrokenPipe
        | std::io::ErrorKind::ConnectionAborted
        | std::io::ErrorKind::ConnectionReset
        | std::io::ErrorKind::Interrupted
        | std::io::ErrorKind::NotConnected
        | std::io::ErrorKind::TimedOut
        | std::io::ErrorKind::WouldBlock => ErrorKind::Transient,
        _ => classify_error_message(&error.to_string()).unwrap_or(ErrorKind::Fatal),
    }
}

fn classify_error_message(message: &str) -> Option<ErrorKind> {
    if message.contains("\"status\":\"409\"") && message.contains("\"code\":\"already_exists\"") {
        return Some(ErrorKind::Transient);
    }

    let message = message.to_ascii_lowercase();
    [
        "connection reset by peer",
        "database is locked",
        "database table is locked",
        "error sending request for url",
        "status 429 too many requests",
    ]
    .iter()
    .any(|fragment| message.contains(fragment))
    .then_some(ErrorKind::Transient)
}

// SQLite Cloud error code ranges:
//   < 10_000       SQLite native errors
//   10_000–99_999  SQLite Cloud server errors
//   >= 100_000     SDK/client internal errors
fn classify_error_code(code: i64) -> ErrorKind {
    match code {
        // SQLite Cloud server errors
        10004 => ErrorKind::Auth,                      // CLOUD_ERRCODE_AUTH
        10000 | 10003 | 10006 => ErrorKind::Transient, // MEM, INTERNAL, RAFT
        10001 | 10002 | 10005 => ErrorKind::Fatal,     // NOTFOUND, COMMAND, GENERIC

        // SDK internal errors
        100005 | 100008 => ErrorKind::Transient, // NETWORK, SOCKCLOSED
        100002 | 100003 | 100006 => ErrorKind::Fatal, // TLS, URL, FORMAT
        100000 | 100001 | 100004 | 100007 => ErrorKind::Transient, // GENERIC, PUBSUB, MEMORY, INDEX

        // SQLite contention
        5 | 6 => ErrorKind::Transient, // SQLITE_BUSY, SQLITE_LOCKED

        // Other SQLite native errors (< 10_000) are schema/constraint issues
        _ if code < 10_000 => ErrorKind::Fatal,

        // Unknown codes in cloud/sdk ranges — assume transient
        _ => ErrorKind::Transient,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn already_existing_cloud_resource_is_transient() {
        let message = r#"{"errors":[{"status":"409","code":"already_exists","title":"Conflict","detail":"resource already exists"}]}"#;

        assert_eq!(
            classify_database_error(Some("1"), message),
            ErrorKind::Transient
        );
    }

    #[test]
    fn other_sqlite_errors_remain_fatal() {
        assert_eq!(
            classify_database_error(Some("1"), "no such table: sessions"),
            ErrorKind::Fatal
        );
    }

    #[test]
    fn sqlite_contention_and_transport_failures_are_transient() {
        assert_eq!(classify_error_code(5), ErrorKind::Transient);
        assert_eq!(
            classify_database_error(Some("1"), "Recv failure: Connection reset by peer"),
            ErrorKind::Transient
        );
        assert_eq!(
            classify_io_error(&std::io::Error::other(
                "E2EE pre-sync witness apply failed: database is locked"
            )),
            ErrorKind::Transient
        );
        assert_eq!(
            Error::Sqlx(sqlx::Error::Io(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "connection reset by peer",
            )))
            .kind(),
            ErrorKind::Transient
        );
    }
}
