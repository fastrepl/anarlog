pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Typst compile error: {0}")]
    TypstCompile(String),
    #[error("Typst PDF error: {0}")]
    TypstPdf(String),
}
