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
    #[error("failed to initialize the cloudsync extension: {0}")]
    ExtensionInitialization(String),
    #[error("CloudSync requires WAL journal mode for commit-safe change notifications")]
    WalRequired,
    #[error("cloudsync pending payload limits must be greater than zero")]
    InvalidPendingPayloadLimits,
    #[error(
        "cloudsync outbound payload exceeds the safe send limit: observed {chunks} chunks, {rows} rows, and {bytes} bytes (limits: {max_chunks} chunks, {max_rows} rows, and {max_bytes} bytes)"
    )]
    OutboundPayloadTooLarge {
        chunks: u32,
        rows: u64,
        bytes: u64,
        max_chunks: u32,
        max_rows: u64,
        max_bytes: u64,
    },
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
    // SQLite Cloud wraps HTTP JSON in SQLITE_ERROR (code 1), sometimes with a prefix.
    if let Some(response) = message
        .find('{')
        .and_then(|start| serde_json::from_str::<serde_json::Value>(&message[start..]).ok())
        && let Some(errors) = response.get("errors").and_then(serde_json::Value::as_array)
        && !errors.is_empty()
    {
        return errors
            .iter()
            .all(|error| {
                matches!(
                    (error["status"].as_str(), error["code"].as_str()),
                    (
                        Some("409"),
                        Some("already_exists" | "apply_batch_superseded")
                    ) | (Some("404"), Some("not_found"))
                )
            })
            .then_some(ErrorKind::Transient);
    }

    let message = message.to_ascii_lowercase();
    [
        "connection reset by peer",
        "database is locked",
        "database table is locked",
        "error sending request for url",
        "timed out",
        "timeout was reached",
        "unable to upload payload chunk",
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
    fn superseded_cloud_batch_is_transient() {
        let message =
            r#"{"errors":[{"status":"409","code":"apply_batch_superseded","title":"Conflict"}]}"#;

        assert_eq!(
            classify_database_error(Some("1"), message),
            ErrorKind::Transient
        );
        assert_eq!(
            classify_io_error(&std::io::Error::other(format!("sqlx error: {message}"))),
            ErrorKind::Transient
        );
    }

    #[test]
    fn unrelated_cloud_conflicts_remain_fatal() {
        for message in [
            r#"{"errors":[{"status":"409","code":"schema_conflict"}]}"#,
            r#"{"errors":[{"status":"403","code":"apply_batch_superseded"}]}"#,
            r#"{"errors":[{"status":"409","code":"schema_conflict"},{"status":"403","code":"apply_batch_superseded"}]}"#,
            r#"{"errors":[{"status":"409","code":"apply_batch_superseded"},{"status":"409","code":"schema_conflict"}]}"#,
            r#"{"errors":[{"status":"409","code":"apply_batch_superseded"}]"#,
        ] {
            assert_eq!(
                classify_database_error(Some("1"), message),
                ErrorKind::Fatal
            );
        }
    }

    #[test]
    fn cloud_error_classification_ignores_json_formatting() {
        let message =
            r#"sqlx error: {"errors": [{"code": "apply_batch_superseded", "status": "409"}]}"#;

        assert_eq!(
            classify_database_error(Some("1"), message),
            ErrorKind::Transient
        );
    }

    #[test]
    fn unrecognized_cloud_response_preserves_native_error_kind() {
        assert_eq!(
            classify_database_error(
                Some("10004"),
                r#"{"errors":[{"status":"401","code":"unauthorized"}]}"#,
            ),
            ErrorKind::Auth
        );
    }

    #[test]
    fn managed_database_not_found_is_transient() {
        let message = r#"error returned from database: (code: 1) {"errors": [{"status":"404","code":"not_found","title":"Not Found","detail":"managed database not found"}]}"#;

        assert_eq!(
            classify_database_error(Some("1"), message),
            ErrorKind::Transient
        );
        assert_eq!(
            classify_io_error(&std::io::Error::other(format!("sqlx error: {message}"))),
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
            classify_database_error(
                Some("1"),
                "Operation timed out after 250 milliseconds with 0 bytes received"
            ),
            ErrorKind::Transient
        );
        assert_eq!(
            classify_database_error(Some("1"), "Timeout was reached"),
            ErrorKind::Transient
        );
        assert_eq!(
            classify_database_error(
                Some("1"),
                "cloudsync_network_send_changes unable to upload payload chunk to remote host"
            ),
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
