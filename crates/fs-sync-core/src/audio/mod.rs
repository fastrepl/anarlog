use std::path::{Path, PathBuf};

use crate::error::AudioImportError;
use crate::runtime::{AudioImportEvent, AudioImportRuntime};
use chrono::{DateTime, Utc};

const AUDIO_FORMATS: [&str; 3] = ["audio.mp3", "audio.wav", "audio.ogg"];

#[derive(Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AudioSourceMetadata {
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub duration_ms: Option<u64>,
}

pub fn exists(session_dir: &Path) -> std::io::Result<bool> {
    AUDIO_FORMATS
        .iter()
        .map(|format| session_dir.join(format))
        .try_fold(false, |acc, path| {
            std::fs::exists(&path).map(|exists| acc || exists)
        })
}

pub fn delete(session_dir: &Path) -> std::io::Result<()> {
    for format in AUDIO_FORMATS {
        let path = session_dir.join(format);
        if std::fs::exists(&path).unwrap_or(false) {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

pub fn path(session_dir: &Path) -> Option<PathBuf> {
    AUDIO_FORMATS
        .iter()
        .map(|format| session_dir.join(format))
        .find(|path| path.exists())
}

pub fn source_metadata(source_path: &Path) -> std::io::Result<AudioSourceMetadata> {
    use hypr_audio_utils::Source;

    let metadata = std::fs::metadata(source_path)?;
    let created_at = metadata.created().ok().map(system_time_to_iso);
    let modified_at = metadata.modified().ok().map(system_time_to_iso);
    let duration_ms = hypr_audio_utils::source_from_path(source_path)
        .ok()
        .and_then(|source| source.total_duration())
        .and_then(|duration| u64::try_from(duration.as_millis()).ok());

    Ok(AudioSourceMetadata {
        created_at,
        modified_at,
        duration_ms,
    })
}

pub fn import_to_session(
    runtime: &dyn AudioImportRuntime,
    session_id: &str,
    session_dir: &Path,
    source_path: &Path,
) -> Result<PathBuf, AudioImportError> {
    runtime.emit(AudioImportEvent::Started {
        session_id: session_id.to_string(),
    });

    std::fs::create_dir_all(session_dir)?;

    let target_path = session_dir.join("audio.mp3");
    let tmp_path = session_dir.join("audio.mp3.tmp");

    let on_progress = {
        let session_id = session_id.to_string();
        let mut last_emitted: f64 = 0.0;
        let mut last_time = std::time::Instant::now();
        move |percentage: f64| {
            let now = std::time::Instant::now();
            if (percentage - last_emitted) >= 0.01
                || now.duration_since(last_time).as_millis() >= 100
            {
                runtime.emit(AudioImportEvent::Progress {
                    session_id: session_id.clone(),
                    percentage,
                });
                last_emitted = percentage;
                last_time = now;
            }
        }
    };

    let result = hypr_audio_norm::normalize_file(
        source_path,
        &tmp_path,
        &target_path,
        None,
        Some(on_progress),
    )
    .map(|_| ());
    match result {
        Ok(()) => {
            let final_path = target_path;
            runtime.emit(AudioImportEvent::Completed {
                session_id: session_id.to_string(),
            });
            Ok(final_path.to_path_buf())
        }
        Err(error) => {
            if tmp_path.exists() {
                let _ = std::fs::remove_file(&tmp_path);
            }
            runtime.emit(AudioImportEvent::Failed {
                session_id: session_id.to_string(),
                error: error.to_string(),
            });
            Err(error.into())
        }
    }
}

pub fn import_audio(
    source_path: &Path,
    tmp_path: &Path,
    target_path: &Path,
) -> Result<PathBuf, hypr_audio_norm::Error> {
    hypr_audio_norm::normalize_file(source_path, tmp_path, target_path, None, None::<fn(f64)>)
}

fn system_time_to_iso(time: std::time::SystemTime) -> String {
    DateTime::<Utc>::from(time).to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use hypr_audio_utils::Source;

    const MIN_MP3_BYTES: u64 = 1024;

    macro_rules! test_import_audio {
        ($($name:ident: $path:expr),* $(,)?) => {
            $(
                #[test]
                fn $name() {
                    let source_path = std::path::Path::new($path);
                    let temp = TempDir::new().unwrap();
                    let tmp_path = temp.path().join("tmp.mp3");
                    let target_path = temp.path().join("target.mp3");

                    let result = import_audio(source_path, &tmp_path, &target_path);
                    assert!(result.is_ok(), "import failed: {:?}", result.err());
                    assert!(target_path.exists());

                    let size = std::fs::metadata(&target_path).unwrap().len();
                    assert!(
                        size > MIN_MP3_BYTES,
                        "Output too small ({size} bytes), likely empty audio"
                    );
                }
            )*
        };
    }

    test_import_audio! {
        test_import_wav: hypr_data::english_1::AUDIO_PATH,
        test_import_mp3: hypr_data::english_1::AUDIO_MP3_PATH,
        test_import_mp4: hypr_data::english_1::AUDIO_MP4_PATH,
        test_import_m4a: hypr_data::english_1::AUDIO_M4A_PATH,
        test_import_ogg: hypr_data::english_1::AUDIO_OGG_PATH,
        test_import_flac: hypr_data::english_1::AUDIO_FLAC_PATH,
        test_import_aac: hypr_data::english_1::AUDIO_AAC_PATH,
        test_import_aiff: hypr_data::english_1::AUDIO_AIFF_PATH,
        test_import_caf: hypr_data::english_1::AUDIO_CAF_PATH,
    }

    #[test]
    fn test_import_stereo_mp3() {
        let source_path = std::path::Path::new(hypr_data::english_10::AUDIO_MP3_PATH);
        let temp = TempDir::new().unwrap();
        let tmp_path = temp.path().join("tmp.mp3");
        let target_path = temp.path().join("target.mp3");

        let result = import_audio(source_path, &tmp_path, &target_path);
        assert!(result.is_ok(), "import failed: {:?}", result.err());
        assert!(target_path.exists());

        let size = std::fs::metadata(&target_path).unwrap().len();
        assert!(
            size > MIN_MP3_BYTES,
            "Output too small ({size} bytes), likely empty audio"
        );

        let decoder = hypr_audio_utils::source_from_path(&target_path).unwrap();
        let channels: u16 = decoder.channels().into();
        assert_eq!(channels, 2, "stereo input should produce stereo output");
    }

    #[test]
    fn test_import_problem_m4a() {
        let source = match std::env::var("PROBLEM_M4A") {
            Ok(p) => PathBuf::from(p),
            Err(_) => return,
        };
        let temp = TempDir::new().unwrap();
        let result = import_audio(
            &source,
            &temp.path().join("tmp.mp3"),
            &temp.path().join("out.mp3"),
        );
        assert!(result.is_ok(), "import failed: {:?}", result.err());
    }

    #[test]
    fn test_import_problem2_m4a() {
        let source = match std::env::var("PROBLEM2_M4A") {
            Ok(p) => PathBuf::from(p),
            Err(_) => return,
        };
        let temp = TempDir::new().unwrap();
        let result = import_audio(
            &source,
            &temp.path().join("tmp.mp3"),
            &temp.path().join("out.mp3"),
        );
        assert!(result.is_ok(), "import failed: {:?}", result.err());
    }
}
