#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Cactus(#[from] hypr_cactus::Error),
    #[error(transparent)]
    ModelManager(#[from] hypr_model_manager::Error<hypr_cactus::Error>),
}
