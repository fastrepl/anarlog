use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct AppPaths {
    #[allow(dead_code)]
    pub base: PathBuf,
    pub models_base: PathBuf,
}

pub fn resolve_paths(base_override: Option<&Path>) -> AppPaths {
    let data_dir = dirs::data_dir().unwrap_or_else(std::env::temp_dir);
    let base = base_override
        .map(Path::to_path_buf)
        .unwrap_or_else(|| data_dir.join("char"));
    let models_base = base.join("models");

    AppPaths { base, models_base }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_paths_uses_override() {
        let base = Path::new("/tmp/char-cli-tests");
        let paths = resolve_paths(Some(base));

        assert_eq!(paths.base, base);
        assert_eq!(paths.models_base, base.join("models"));
    }

    #[test]
    fn resolve_paths_defaults_models_under_base() {
        let paths = resolve_paths(None);

        assert!(paths.base.ends_with("char"));
        assert_eq!(paths.models_base, paths.base.join("models"));
    }
}
