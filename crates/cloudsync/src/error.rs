const SUPPORTED_CLOUDSYNC_TARGETS: &str = concat!(
    "macos/{aarch64,x86_64}, ",
    "linux/{gnu,musl}/{aarch64,x86_64}, ",
    "windows/x86_64"
);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("no cache directory is available for the bundled cloudsync extension")]
    MissingCacheDir,
    #[error(
        "the bundled cloudsync extension is not available for this target; supported targets: {SUPPORTED_CLOUDSYNC_TARGETS}"
    )]
    UnsupportedBundledCloudsync,
}
