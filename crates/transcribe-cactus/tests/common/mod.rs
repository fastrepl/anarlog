use std::path::PathBuf;

pub fn model_path() -> PathBuf {
    let path = std::env::var("CACTUS_STT_MODEL").unwrap_or_else(|_| {
        dirs::data_dir()
            .expect("could not find data dir")
            .join("com.hyprnote.dev/models/cactus/whisper-small-int8-apple")
            .to_string_lossy()
            .into_owned()
    });
    let path = PathBuf::from(path);
    assert!(path.exists(), "model not found: {}", path.display());
    path
}
