use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    EmptyMemoryId,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyMemoryId => f.write_str("memory id cannot be empty"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T, E = Error> = std::result::Result<T, E>;
