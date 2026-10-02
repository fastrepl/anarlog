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
    fn skips_generated_and_internal_paths() {
        for (relative, absolute) in [
            ("some/path/.DS_Store", "/some/path/.DS_Store"),
            (".tmp6s1cca", "/vault/.tmp6s1cca"),
            (".tmpvdaLsp", "/vault/.tmpvdaLsp"),
            ("subdir/.tmpABC123", "/vault/subdir/.tmpABC123"),
            ("store.json", "/vault/store.json"),
            ("argmax/some/file.txt", "/vault/argmax/some/file.txt"),
            ("argmax_data.json", "/vault/argmax_data.json"),
            ("audio/recording.wav", "/vault/audio/recording.wav"),
            ("audio/recording.ogg", "/vault/audio/recording.ogg"),
            ("temp/file.tmp", "/vault/temp/file.tmp"),
            (
                "models/local/encoder.layer_6_self_attn_output.bias",
                "/vault/models/local/encoder.layer_6_self_attn_output.bias",
            ),
            (
                "search_index/abc123.fieldnorm",
                "/vault/search_index/abc123.fieldnorm",
            ),
            (
                "search_index/abc123.fast",
                "/vault/search_index/abc123.fast",
            ),
            (
                "search_index/abc123.term",
                "/vault/search_index/abc123.term",
            ),
        ] {
            assert!(
                should_skip_path(relative, &PathBuf::from(absolute)),
                "expected {relative:?} to be skipped"
            );
        }
    }

    #[test]
    fn keeps_user_files() {
        for (relative, absolute) in [
            ("notes/note.md", "/vault/notes/note.md"),
            ("data.json", "/vault/data.json"),
            ("sessions/session.txt", "/vault/sessions/session.txt"),
            ("subdir/store.json", "/vault/subdir/store.json"),
        ] {
            assert!(
                !should_skip_path(relative, &PathBuf::from(absolute)),
                "unexpectedly skipped {relative:?}"
            );
        }
    }

    #[test]
    fn to_relative_path_strips_only_matching_base() {
        for (path, base, expected) in [
            ("/vault/base/notes/file.md", "/vault/base", "notes/file.md"),
            (
                "/vault/notes/file.md",
                "/different/base",
                "/vault/notes/file.md",
            ),
            ("/vault/base", "/vault/base", ""),
        ] {
            assert_eq!(
                to_relative_path(&PathBuf::from(path), &PathBuf::from(base)),
                expected,
                "unexpected relative path for {path:?} under {base:?}"
            );
        }
    }
}
