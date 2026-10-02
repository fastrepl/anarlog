use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use crate::{CustomSttModelFormat, CustomSttModelInfo, Error};

const GGUF_MAGIC: &[u8; 4] = b"GGUF";

pub fn inspect_custom_model_path(path: &str) -> Result<(PathBuf, CustomSttModelInfo), Error> {
    let path = Path::new(path);
    if !path.is_absolute() {
        return Err(Error::InvalidModelPath(
            "Model path must be absolute".to_string(),
        ));
    }

    let path = std::fs::canonicalize(path)
        .map_err(|error| Error::InvalidModelPath(format!("Model file is unavailable: {error}")))?;
    let metadata = path
        .metadata()
        .map_err(|error| Error::InvalidModelPath(format!("Model file is unavailable: {error}")))?;
    if !metadata.is_file() {
        return Err(Error::InvalidModelPath(
            "Model path must point to a file".to_string(),
        ));
    }

    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(extension.as_deref(), Some("bin" | "gguf")) {
        return Err(Error::InvalidModelPath(
            "Select a whisper.cpp .bin or transcribe.cpp .gguf model".to_string(),
        ));
    }

    let mut magic = [0_u8; 4];
    let magic_is_gguf = File::open(&path)
        .and_then(|mut file| file.read_exact(&mut magic))
        .is_ok()
        && &magic == GGUF_MAGIC;

    if extension.as_deref() == Some("gguf") || magic_is_gguf {
        if extension.as_deref() == Some("gguf") && !magic_is_gguf {
            return Err(Error::InvalidModelPath(
                "The selected file is not a valid GGUF model".to_string(),
            ));
        }

        return Err(Error::GgufModelUnsupported);
    }

    let path_string = path
        .to_str()
        .ok_or_else(|| Error::InvalidModelPath("Model path is not valid UTF-8".to_string()))?
        .to_string();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::InvalidModelPath("Model filename is not valid UTF-8".to_string()))?
        .to_string();

    Ok((
        path,
        CustomSttModelInfo {
            path: path_string,
            name,
            size_bytes: metadata.len(),
            format: CustomSttModelFormat::Ggml,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_an_absolute_bin_path() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ggml-small.bin");
        std::fs::write(&path, b"ggml").unwrap();

        let (_, info) = inspect_custom_model_path(path.to_str().unwrap()).unwrap();

        assert_eq!(info.name, "ggml-small.bin");
        assert_eq!(info.size_bytes, 4);
        assert_eq!(info.format, CustomSttModelFormat::Ggml);
    }

    #[test]
    fn rejects_unusable_model_paths() {
        for (filename, contents, is_relative, expected_substrings) in [
            ("ggml-small.bin", None, true, &["absolute"][..]),
            ("missing.bin", None, false, &["unavailable"][..]),
            ("model.pt", Some(&b"model"[..]), false, &[".bin"][..]),
            (
                "whisper.gguf",
                Some(&b"GGUF"[..]),
                false,
                &["transcribe.cpp", ".bin"][..],
            ),
            (
                "whisper.gguf",
                Some(&b"nope"[..]),
                false,
                &["not a valid GGUF"][..],
            ),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join(filename);
            if let Some(contents) = contents {
                std::fs::write(&path, contents).unwrap();
            }
            let input = if is_relative {
                filename
            } else {
                path.to_str().unwrap()
            };
            let error = inspect_custom_model_path(input).unwrap_err().to_string();
            for substring in expected_substrings {
                assert!(
                    error.contains(substring),
                    "{filename:?}: expected error to contain {substring:?}, got {error:?}"
                );
            }
        }
    }
}
