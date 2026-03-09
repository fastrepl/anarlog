use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Gix(Box<gix::open::Error>),
    #[error(transparent)]
    GixInit(Box<gix::init::Error>),
    #[error(transparent)]
    GixDiscover(Box<gix::discover::Error>),
    #[error("{0}")]
    Custom(String),
}

impl From<gix::open::Error> for Error {
    fn from(e: gix::open::Error) -> Self {
        Self::Gix(Box::new(e))
    }
}

impl From<gix::init::Error> for Error {
    fn from(e: gix::init::Error) -> Self {
        Self::GixInit(Box::new(e))
    }
}

impl From<gix::discover::Error> for Error {
    fn from(e: gix::discover::Error) -> Self {
        Self::GixDiscover(Box::new(e))
    }
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
