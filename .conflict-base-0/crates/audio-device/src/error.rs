use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Failed to enumerate devices: {0}")]
    EnumerationFailed(String),

    #[error("Failed to get default device: {0}")]
    GetDefaultFailed(String),

    #[error("Failed to set default device: {0}")]
    SetDefaultFailed(String),

    #[error("Platform not supported: {0}")]
    PlatformNotSupported(String),

    #[error("Audio system error: {0}")]
    AudioSystemError(String),
}
