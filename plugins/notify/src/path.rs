use std::path::Path;

pub fn should_skip_path(relative_path: &str, path: &Path) -> bool {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if name == ".DS_Store" {
            return true;
        }

        // https://docs.rs/tempfile/latest/tempfile/struct.Builder.html#method.prefix
        if name.starts_with(".tmp") {
            return true;
        }
    }

    if relative_path == "store.json" {
        return true;
    }

    if relative_path.starts_with("argmax") {
        return true;
    }

    if relative_path.starts_with("search_index") {
        return true;
    }

    if relative_path.starts_with("models/") {
        return true;
    }

    if path
        .extension()
        .is_some_and(|ext| ext == "wav" || ext == "ogg" || ext == "tmp")
    {
        return true;
    }

    false
}

pub fn to_relative_path(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_str()
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_skip_ds_store() {
        let path = PathBuf::from("/some/path/.DS_Store");
        assert!(should_skip_path("some/path/.DS_Store", &path));
    }

    #[test]
    fn test_skip_tmp_prefix() {
        let path = PathBuf::from("/vault/.tmp6s1cca");
        assert!(should_skip_path(".tmp6s1cca", &path));

        let path = PathBuf::from("/vault/.tmpvdaLsp");
        assert!(should_skip_path(".tmpvdaLsp", &path));

        let path = PathBuf::from("/vault/subdir/.tmpABC123");
        assert!(should_skip_path("subdir/.tmpABC123", &path));
    }

    #[test]
    fn test_skip_store_json() {
        let path = PathBuf::from("/vault/store.json");
        assert!(should_skip_path("store.json", &path));
    }

    #[test]
    fn test_skip_argmax_prefix() {
        let path = PathBuf::from("/vault/argmax/some/file.txt");
        assert!(should_skip_path("argmax/some/file.txt", &path));

        let path = PathBuf::from("/vault/argmax_data.json");
        assert!(should_skip_path("argmax_data.json", &path));
    }

    #[test]
    fn test_skip_wav_extension() {
        let path = PathBuf::from("/vault/audio/recording.wav");
        assert!(should_skip_path("audio/recording.wav", &path));
    }

    #[test]
    fn test_skip_ogg_extension() {
        let path = PathBuf::from("/vault/audio/recording.ogg");
        assert!(should_skip_path("audio/recording.ogg", &path));
    }

    #[test]
    fn test_skip_tmp_extension() {
        let path = PathBuf::from("/vault/temp/file.tmp");
        assert!(should_skip_path("temp/file.tmp", &path));
    }

    #[test]
    fn test_skip_models() {
        let path = PathBuf::from("/vault/models/cactus/encoder.layer_6_self_attn_output.bias");
        assert!(should_skip_path(
            "models/cactus/encoder.layer_6_self_attn_output.bias",
            &path
        ));
    }

    #[test]
    fn test_skip_search_index() {
        let path = PathBuf::from("/vault/search_index/abc123.fieldnorm");
        assert!(should_skip_path("search_index/abc123.fieldnorm", &path));

        let path = PathBuf::from("/vault/search_index/abc123.fast");
        assert!(should_skip_path("search_index/abc123.fast", &path));

        let path = PathBuf::from("/vault/search_index/abc123.term");
        assert!(should_skip_path("search_index/abc123.term", &path));
    }

    #[test]
    fn test_allow_regular_files() {
        let path = PathBuf::from("/vault/notes/note.md");
        assert!(!should_skip_path("notes/note.md", &path));

        let path = PathBuf::from("/vault/data.json");
        assert!(!should_skip_path("data.json", &path));

        let path = PathBuf::from("/vault/sessions/session.txt");
        assert!(!should_skip_path("sessions/session.txt", &path));
    }

    #[test]
    fn test_allow_nested_store_json() {
        let path = PathBuf::from("/vault/subdir/store.json");
        assert!(!should_skip_path("subdir/store.json", &path));
    }

    #[test]
    fn test_to_relative_path_strips_base() {
        let base = PathBuf::from("/vault/base");
        let path = PathBuf::from("/vault/base/notes/file.md");
        assert_eq!(to_relative_path(&path, &base), "notes/file.md");
    }

    #[test]
    fn test_to_relative_path_returns_original_if_not_prefixed() {
        let base = PathBuf::from("/different/base");
        let path = PathBuf::from("/vault/notes/file.md");
        assert_eq!(to_relative_path(&path, &base), "/vault/notes/file.md");
    }

    #[test]
    fn test_to_relative_path_handles_exact_match() {
        let base = PathBuf::from("/vault/base");
        let path = PathBuf::from("/vault/base");
        assert_eq!(to_relative_path(&path, &base), "");
    }
}
