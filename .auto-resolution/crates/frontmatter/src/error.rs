use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("missing opening delimiter")]
    MissingOpeningDelimiter,
    #[error("missing closing delimiter")]
    MissingClosingDelimiter,
    #[error("failed to parse YAML frontmatter: {0}")]
    YamlParse(#[from] serde_yaml::Error),
}
