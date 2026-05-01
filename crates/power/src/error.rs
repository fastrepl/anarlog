#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("power information is unavailable: {0}")]
    Unavailable(&'static str),
    #[error("power information is only supported on macOS and Windows")]
    UnsupportedPlatform,
}
