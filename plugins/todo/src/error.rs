#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    AppleTodo(#[from] hypr_apple_todo::Error),
    #[error("unsupported platform")]
    UnsupportedPlatform,
    #[error("auth error: {0}")]
    Auth(String),
    #[error("api error: {0}")]
    Api(String),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    InvalidHeader(#[from] reqwest::header::InvalidHeaderValue),
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
