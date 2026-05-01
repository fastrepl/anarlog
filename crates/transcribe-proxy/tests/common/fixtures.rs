use std::path::PathBuf;

use super::recording::WsRecording;

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

pub fn load_fixture(name: &str) -> WsRecording {
    let path = fixtures_dir().join(name);
    WsRecording::from_jsonl_file(&path).unwrap()
}
