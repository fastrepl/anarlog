#![forbid(unsafe_code)]

mod bundle;
mod error;
mod init;
mod network;

use std::path::PathBuf;

use sqlx::sqlite::SqliteConnectOptions;

pub use bundle::bundled_extension_path;
pub use error::Error;
pub use init::{init, version};
pub use network::{network_init, network_set_apikey, network_set_token, network_sync};

pub const CLOUDSYNC_VERSION: &str = "0.8.68";

pub fn apply(options: SqliteConnectOptions) -> Result<(SqliteConnectOptions, PathBuf), Error> {
    let extension_path = bundled_extension_path()?;
    let options = options.extension(extension_path.to_string_lossy().into_owned());

    Ok((options, extension_path))
}

#[cfg(any(
    all(test, target_os = "macos", target_arch = "aarch64"),
    all(test, target_os = "macos", target_arch = "x86_64"),
    all(test, target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(test, target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(
        test,
        target_os = "linux",
        target_env = "musl",
        target_arch = "aarch64"
    ),
    all(test, target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(test, target_os = "windows", target_arch = "x86_64"),
))]
mod tests {
    use super::*;
    use std::str::FromStr;

    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn loads_bundled_cloudsync() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let (options, _) = apply(options).unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();

        let version = version(&pool).await.unwrap();

        assert!(!version.is_empty());
    }
}
