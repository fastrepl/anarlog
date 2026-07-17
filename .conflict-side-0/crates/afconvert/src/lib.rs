use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("afconvert failed: {0}")]
    Failed(String),
}

pub fn to_wav(source_path: &Path) -> Result<PathBuf, Error> {
    let file_stem = source_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let wav_path = std::env::temp_dir().join(format!(
        "{}_afconvert_{}.wav",
        file_stem,
        std::process::id()
    ));

    let output = Command::new("afconvert")
        .arg("-f")
        .arg("WAVE")
        .arg("-d")
        .arg("LEI16")
        .arg(source_path)
        .arg(&wav_path)
        .output()?;

    if !output.status.success() {
        let _ = std::fs::remove_file(&wav_path);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Failed(stderr.into_owned()));
    }

    Ok(wav_path)
}
