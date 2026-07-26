use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to read supabase file: {0}")]
    SupabaseFileRead(#[from] std::io::Error),

    #[error("failed to parse supabase JSON: {0}")]
    SupabaseJsonParse(#[source] serde_json::Error),

    #[error("access token not found in supabase.json")]
    AccessTokenNotFound,

    #[error("failed to parse token JSON: {0}")]
    TokenJsonParse(#[source] serde_json::Error),

    #[error("API request failed: {0}")]
    ApiRequest(#[from] reqwest::Error),

    #[error("API returned error status {status}: {body}")]
    ApiStatus { status: u16, body: String },

    #[error("failed to parse API response: {0}")]
    ApiResponseParse(#[source] serde_json::Error),

    #[error("failed to read cache file: {0}")]
    CacheFileRead(std::io::Error),

    #[error("failed to parse cache JSON: {0}")]
    CacheJsonParse(#[source] serde_json::Error),

    #[error("failed to create output directory: {0}")]
    CreateDirectory(std::io::Error),

    #[error("failed to write file {path}: {source}")]
    WriteFile {
        path: String,
        source: std::io::Error,
    },

    #[error("failed to serialize YAML frontmatter: {0}")]
    YamlSerialize(#[from] serde_yaml::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
