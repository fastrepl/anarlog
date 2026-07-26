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
    fn inline(_type_map: &mut specta::TypeMap, _generics: specta::Generics) -> specta::DataType {
        specta::DataType::Primitive(specta::datatype::PrimitiveType::String)
    }
}
