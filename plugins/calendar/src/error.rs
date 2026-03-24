#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Calendar(#[from] hypr_calendar::Error),
    #[error("auth error: {0}")]
    Auth(String),
}

impl serde::Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl specta::Type for Error {
    fn definition(_: &mut specta::TypeCollection) -> specta::datatype::DataType {
        specta::datatype::DataType::Primitive(specta::datatype::Primitive::String)
    }
}
