use std::path::{Path, PathBuf};

pub struct AudioDropRequest {
    pub file_path: String,
}

pub fn looks_like_audio_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref(),
        Some("wav" | "mp3" | "m4a" | "flac" | "ogg")
    )
}

pub fn normalize_pasted_path(pasted: &str) -> Option<PathBuf> {
    let trimmed = pasted.trim();

    if trimmed.is_empty() {
        return None;
    }

    let unquoted = trimmed
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| {
            trimmed
                .strip_prefix('\'')
                .and_then(|v| v.strip_suffix('\''))
        })
        .unwrap_or(trimmed);

    if let Ok(url) = url::Url::parse(unquoted)
        && url.scheme() == "file"
    {
        return url.to_file_path().ok();
    }

    let unescaped = unescape_shell_backslashes(unquoted);
    Some(PathBuf::from(unescaped))
}

fn unescape_shell_backslashes(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(ch);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_pasted_path_supports_file_url() {
        let path = normalize_pasted_path("file:///tmp/sample.wav").expect("path");
        assert_eq!(path, PathBuf::from("/tmp/sample.wav"));
    }

    #[test]
    fn normalize_pasted_path_unescapes_shell_spaces() {
        let path = normalize_pasted_path("/tmp/my\\ file.flac").expect("path");
        assert_eq!(path, PathBuf::from("/tmp/my file.flac"));
    }

    #[test]
    fn looks_like_audio_file_filters_extensions() {
        assert!(looks_like_audio_file(&PathBuf::from("/tmp/a.wav")));
        assert!(looks_like_audio_file(&PathBuf::from("/tmp/a.MP3")));
        assert!(!looks_like_audio_file(&PathBuf::from("/tmp/a.txt")));
    }
}
