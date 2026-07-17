#[derive(Debug, thiserror::Error)]

pub enum Error {
    #[error(transparent)]
    ConfigError(#[from] config::ConfigError),
}
