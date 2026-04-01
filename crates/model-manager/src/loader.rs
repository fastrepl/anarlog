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

#[cfg(feature = "whisper-local")]
impl ModelLoader for hypr_whisper_local::LoadedWhisper {
    type Error = hypr_whisper_local::Error;

    fn load(path: &Path) -> Result<Self, Self::Error> {
        hypr_whisper_local::LoadedWhisper::builder()
            .model_path(path.to_string_lossy().into_owned())
            .build()
    }
}
