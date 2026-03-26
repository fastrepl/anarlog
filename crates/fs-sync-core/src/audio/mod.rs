mod encode;
mod file_move;

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::{AudioImportError, AudioProcessingError};
use crate::runtime::{AudioImportEvent, AudioImportRuntime};

const AUDIO_FORMATS: [&str; 3] = ["audio.mp3", "audio.wav", "audio.ogg"];

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

    if tmp_path.exists() {
        std::fs::remove_file(&tmp_path)?;
    }

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

    let result = decode_to_mp3_file(source_path, &tmp_path, None, Some(on_progress))
        .and_then(|()| file_move::atomic_move(&tmp_path, &target_path).map_err(Into::into));
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
) -> Result<PathBuf, AudioProcessingError> {
    decode_to_mp3_file(source_path, tmp_path, None, None::<fn(f64)>)?;
    file_move::atomic_move(tmp_path, target_path)?;
    Ok(target_path.to_path_buf())
}

fn decode_to_mp3_file(
    path: &Path,
    tmp_path: &Path,
    max_duration: Option<Duration>,
    on_progress: Option<impl FnMut(f64)>,
) -> Result<(), AudioProcessingError> {
    with_afconvert_fallback(path, on_progress, |path, on_progress| {
        let file = File::create(tmp_path)?;
        let writer = BufWriter::new(file);
        let bytes_written = decode_with_rodio(path, max_duration, writer, on_progress)?;
        if bytes_written == 0 {
            let _ = std::fs::remove_file(tmp_path);
            return Err(AudioProcessingError::EmptyInput);
        }
        Ok(())
    })
}

fn with_afconvert_fallback<F, T>(
    source_path: &Path,
    mut on_progress: Option<impl FnMut(f64)>,
    mut try_fn: F,
) -> Result<T, AudioProcessingError>
where
    F: FnMut(&Path, Option<&mut dyn FnMut(f64)>) -> Result<T, AudioProcessingError>,
{
    match try_fn(
        source_path,
        on_progress.as_mut().map(|p| p as &mut dyn FnMut(f64)),
    ) {
        Ok(val) => Ok(val),
        Err(_first_err) => {
            #[cfg(target_os = "macos")]
            {
                let wav_path = hypr_afconvert::to_wav(source_path)
                    .map_err(|e| AudioProcessingError::AfconvertFailed(e.to_string()))?;
                let result = try_fn(
                    &wav_path,
                    on_progress.as_mut().map(|p| p as &mut dyn FnMut(f64)),
                );
                let _ = std::fs::remove_file(&wav_path);
                result
            }
            #[cfg(not(target_os = "macos"))]
            Err(_first_err)
        }
    }
}

fn decode_with_rodio<W: Write>(
    path: &Path,
    max_duration: Option<Duration>,
    output: W,
    on_progress: Option<&mut dyn FnMut(f64)>,
) -> Result<usize, AudioProcessingError> {
    let file = File::open(path)?;
    let decoder = rodio::Decoder::try_from(file)?;
    encode::encode_source_to_mp3(decoder, max_duration, output, on_progress)
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

        let file = File::open(&target_path).unwrap();
        let decoder = rodio::Decoder::try_from(file).unwrap();
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
