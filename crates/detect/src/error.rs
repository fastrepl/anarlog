#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to get audio processes: {0}")]
    AudioProcessQuery(String),
    #[error("failed to inspect audio process state: {0}")]
    AudioProcessState(String),
    #[cfg(target_os = "linux")]
    #[error("failed to create pulseaudio mainloop")]
    PulseMainloop,
    #[cfg(target_os = "linux")]
    #[error("failed to create pulseaudio context")]
    PulseContext,
    #[cfg(target_os = "linux")]
    #[error("failed to connect to pulseaudio")]
    PulseConnect,
}
