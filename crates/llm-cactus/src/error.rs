#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Cactus(#[from] hypr_cactus::Error),
    #[error(transparent)]
    Manager(#[from] hypr_model_manager::Error),
}
