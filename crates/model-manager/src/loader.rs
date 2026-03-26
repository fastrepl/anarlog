use std::path::Path;

pub trait ModelLoader: Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    fn load(path: &Path) -> Result<Self, Self::Error>
    where
        Self: Sized;
}

#[cfg(feature = "cactus")]
impl ModelLoader for hypr_cactus::Model {
    type Error = hypr_cactus::Error;

    fn load(path: &Path) -> Result<Self, Self::Error> {
        hypr_cactus::Model::new(path)
    }
}
