use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use crate::error::AudioImportError;
use crate::path::is_uuid;
use crate::runtime::{AudioImportEvent, AudioImportRuntime};
use chrono::{DateTime, Utc};

const AUDIO_FORMATS: [&str; 3] = ["audio.mp3", "audio.wav", "audio.ogg"];
const AUDIO_ARTIFACTS: [&str; 7] = [
    "audio.mp3",
    "audio.wav",
    "audio.ogg",
    "audio.mp3.tmp",
    "audio.wav.tmp",
    "audio_mic.wav",
    "audio_spk.wav",
];

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
    for artifact in AUDIO_ARTIFACTS {
        let path = session_dir.join(artifact);
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

pub fn delete_orphaned_expired(
    sessions_dir: &Path,
    known_session_ids: &[String],
    retention_ms: u64,
    now_ms: u64,
) -> std::io::Result<Vec<String>> {
    if !sessions_dir.exists() {
        return Ok(Vec::new());
    }

    let known_session_ids: HashSet<&str> = known_session_ids.iter().map(String::as_str).collect();
    let expires_before_ms = now_ms.saturating_sub(retention_ms);
    let mut deleted = Vec::new();

    delete_orphaned_expired_in_dir(
        sessions_dir,
        &known_session_ids,
        expires_before_ms,
        &mut deleted,
    )?;

    Ok(deleted)
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

fn delete_orphaned_expired_in_dir(
    dir: &Path,
    known_session_ids: &HashSet<&str>,
    expires_before_ms: u64,
    deleted: &mut Vec<String>,
) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        if is_uuid(name) {
            if known_session_ids.contains(name) || path.join("_meta.json").exists() {
                continue;
            }

            if orphan_audio_expired(&path, expires_before_ms)? {
                delete(&path)?;
                deleted.push(name.to_string());
            }
            continue;
        }

        delete_orphaned_expired_in_dir(&path, known_session_ids, expires_before_ms, deleted)?;
    }

    Ok(())
}

fn orphan_audio_expired(session_dir: &Path, expires_before_ms: u64) -> std::io::Result<bool> {
    let mut latest_modified_ms: Option<u64> = None;

    for artifact in AUDIO_ARTIFACTS {
        let path = session_dir.join(artifact);
        let metadata = match std::fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };

        let modified_ms = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX);

        latest_modified_ms =
            Some(latest_modified_ms.map_or(modified_ms, |latest| latest.max(modified_ms)));
    }

    Ok(latest_modified_ms.is_some_and(|modified_ms| modified_ms <= expires_before_ms))
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
    use std::time::SystemTime;

    const MIN_MP3_BYTES: u64 = 1024;
    const KNOWN_SESSION_ID: &str = "11111111-1111-4111-8111-111111111111";
    const ORPHAN_SESSION_ID: &str = "22222222-2222-4222-8222-222222222222";
    const META_SESSION_ID: &str = "33333333-3333-4333-8333-333333333333";
    const FRESH_ORPHAN_SESSION_ID: &str = "44444444-4444-4444-8444-444444444444";

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .try_into()
            .unwrap()
    }

    fn write_audio(path: &Path) {
        std::fs::write(path, b"audio").unwrap();
    }

    #[test]
    fn test_delete_removes_audio_artifacts() {
        let temp = TempDir::new().unwrap();
        let session_dir = temp.path();
        for artifact in AUDIO_ARTIFACTS {
            write_audio(&session_dir.join(artifact));
        }
        let note_path = session_dir.join("note.md");
        std::fs::write(&note_path, b"keep").unwrap();

        delete(session_dir).unwrap();

        for artifact in AUDIO_ARTIFACTS {
            assert!(!session_dir.join(artifact).exists());
        }
        assert!(note_path.exists());
    }

    #[test]
    fn test_delete_orphaned_expired_removes_nested_orphan_audio() {
        let temp = TempDir::new().unwrap();
        let sessions_dir = temp.path();
        let orphan_dir = sessions_dir.join("folder").join(ORPHAN_SESSION_ID);
        let known_dir = sessions_dir.join(KNOWN_SESSION_ID);
        let meta_dir = sessions_dir.join(META_SESSION_ID);
        std::fs::create_dir_all(&orphan_dir).unwrap();
        std::fs::create_dir_all(&known_dir).unwrap();
        std::fs::create_dir_all(&meta_dir).unwrap();
        write_audio(&orphan_dir.join("audio.wav"));
        write_audio(&orphan_dir.join("audio_mic.wav"));
        write_audio(&known_dir.join("audio.wav"));
        write_audio(&meta_dir.join("audio.wav"));
        std::fs::write(meta_dir.join("_meta.json"), b"{}").unwrap();

        let deleted =
            delete_orphaned_expired(sessions_dir, &[KNOWN_SESSION_ID.to_string()], 0, now_ms())
                .unwrap();

        assert_eq!(deleted, vec![ORPHAN_SESSION_ID.to_string()]);
        assert!(!orphan_dir.join("audio.wav").exists());
        assert!(!orphan_dir.join("audio_mic.wav").exists());
        assert!(known_dir.join("audio.wav").exists());
        assert!(meta_dir.join("audio.wav").exists());
    }

    #[test]
    fn test_delete_orphaned_expired_keeps_fresh_orphan_audio() {
        let temp = TempDir::new().unwrap();
        let sessions_dir = temp.path();
        let orphan_dir = sessions_dir.join(FRESH_ORPHAN_SESSION_ID);
        std::fs::create_dir_all(&orphan_dir).unwrap();
        write_audio(&orphan_dir.join("audio.wav"));

        let deleted = delete_orphaned_expired(sessions_dir, &[], u64::MAX, now_ms()).unwrap();

        assert!(deleted.is_empty());
        assert!(orphan_dir.join("audio.wav").exists());
    }

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
