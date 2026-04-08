mod encode;
mod error;
mod file_move;

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub use error::Error;

pub type Result<T> = std::result::Result<T, Error>;

pub use encode::TARGET_SAMPLE_RATE_HZ;

pub fn normalize_file(
    source_path: &Path,
    tmp_path: &Path,
    target_path: &Path,
    max_duration: Option<Duration>,
    on_progress: Option<impl FnMut(f64)>,
) -> Result<PathBuf> {
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if tmp_path.exists() {
        std::fs::remove_file(tmp_path)?;
    }

    let result = decode_to_mp3_file(source_path, tmp_path, max_duration, on_progress)
        .and_then(|()| file_move::atomic_move(tmp_path, target_path).map_err(Into::into));

    match result {
        Ok(()) => Ok(target_path.to_path_buf()),
        Err(error) => {
            if tmp_path.exists() {
                let _ = std::fs::remove_file(tmp_path);
            }
            Err(error)
        }
    }
}

fn decode_to_mp3_file(
    path: &Path,
    tmp_path: &Path,
    max_duration: Option<Duration>,
    on_progress: Option<impl FnMut(f64)>,
) -> Result<()> {
    with_afconvert_fallback(path, on_progress, |path, on_progress| {
        let file = File::create(tmp_path)?;
        let writer = BufWriter::new(file);
        let bytes_written = decode_with_rodio(path, max_duration, writer, on_progress)?;
        if bytes_written == 0 {
            let _ = std::fs::remove_file(tmp_path);
            return Err(Error::EmptyInput);
        }
        Ok(())
    })
}

fn with_afconvert_fallback<F, T>(
    source_path: &Path,
    mut on_progress: Option<impl FnMut(f64)>,
    mut try_fn: F,
) -> Result<T>
where
    F: FnMut(&Path, Option<&mut dyn FnMut(f64)>) -> Result<T>,
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
                    .map_err(|e| Error::AfconvertFailed(e.to_string()))?;
                let result = try_fn(
                    &wav_path,
                    on_progress.as_mut().map(|p| p as &mut dyn FnMut(f64)),
                );
                let _ = std::fs::remove_file(&wav_path);
                result
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err(_first_err)
            }
        }
    }
}

fn decode_with_rodio<W: Write>(
    path: &Path,
    max_duration: Option<Duration>,
    output: W,
    on_progress: Option<&mut dyn FnMut(f64)>,
) -> Result<usize> {
    let file = File::open(path)?;
    let decoder = rodio::Decoder::try_from(file)?;
    encode::encode_source_to_mp3(decoder, max_duration, output, on_progress)
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use hypr_audio_utils::Source;

    use super::*;

    const MIN_MP3_BYTES: u64 = 1024;

    macro_rules! test_normalize_audio {
        ($($name:ident: $path:expr),* $(,)?) => {
            $(
                #[test]
                fn $name() {
                    let source_path = std::path::Path::new($path);
                    let temp = TempDir::new().unwrap();
                    let tmp_path = temp.path().join("tmp.mp3");
                    let target_path = temp.path().join("target.mp3");

                    let result = normalize_file(source_path, &tmp_path, &target_path, None, None::<fn(f64)>);
                    assert!(result.is_ok(), "normalize failed: {:?}", result.err());
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

    test_normalize_audio! {
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

        let result = normalize_file(source_path, &tmp_path, &target_path, None, None::<fn(f64)>);
        assert!(result.is_ok(), "normalize failed: {:?}", result.err());
        assert!(target_path.exists());

        let size = std::fs::metadata(&target_path).unwrap().len();
        assert!(
            size > MIN_MP3_BYTES,
            "Output too small ({size} bytes), likely empty audio"
        );

        let file = std::fs::File::open(&target_path).unwrap();
        let decoder = rodio::Decoder::try_from(file).unwrap();
        let channels: u16 = decoder.channels().into();
        let rate: u32 = decoder.sample_rate().into();

        assert_eq!(channels, 2, "stereo channels were not preserved");
        assert_eq!(
            rate, TARGET_SAMPLE_RATE_HZ,
            "output sample rate should be 16kHz"
        );
    }
}
