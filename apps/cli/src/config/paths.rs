use std::path::PathBuf;
use std::sync::OnceLock;

static BASE_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

pub fn set_base(path: PathBuf) {
    let _ = BASE_OVERRIDE.set(path);
}

#[derive(Clone, Debug)]
pub struct AppPaths {
    pub base: PathBuf,
    pub models_base: PathBuf,
}

pub fn resolve_paths() -> AppPaths {
    let data_dir = dirs::data_dir().unwrap_or_else(std::env::temp_dir);
    let base = BASE_OVERRIDE
        .get()
        .cloned()
        .unwrap_or_else(|| data_dir.join("char"));
    let models_base = base.join("models");

    AppPaths { base, models_base }
}
