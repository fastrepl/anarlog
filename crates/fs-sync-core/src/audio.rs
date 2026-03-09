use std::fs::{File, copy, remove_file, rename, write};
use std::io::ErrorKind;
use std::num::{NonZeroU8, NonZeroU32};
use std::path::{Path, PathBuf};

use hypr_audio_utils::{
    Source, VorbisEncodeSettings, encode_vorbis_mono, mix_down_to_mono, resample_audio,
};

use crate::error::{AudioImportError, AudioProcessingError};

const TARGET_SAMPLE_RATE_HZ: u32 = 16_000;
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
    session_dir: &Path,
    source_path: &Path,
) -> Result<PathBuf, AudioImportError> {
    std::fs::create_dir_all(session_dir)?;

    let target_path = session_dir.join("audio.ogg");
    let tmp_path = session_dir.join("audio.ogg.tmp");

    if tmp_path.exists() {
        std::fs::remove_file(&tmp_path)?;
    }

    match import_audio(source_path, &tmp_path, &target_path) {
        Ok(final_path) => Ok(final_path),
        Err(error) => {
            if tmp_path.exists() {
                let _ = std::fs::remove_file(&tmp_path);
            }
            Err(error.into())
        }
    }
}

pub fn import_audio(
    source_path: &Path,
    tmp_path: &Path,
    target_path: &Path,
) -> Result<PathBuf, AudioProcessingError> {
    let file = File::open(source_path)?;
    match rodio::Decoder::try_from(file) {
        Ok(decoder) => import_audio_from_decoder(decoder, tmp_path, target_path),
        Err(_original_err) => {
            #[cfg(target_os = "macos")]
            {
                let wav_path = hypr_afconvert::to_wav(source_path)
                    .map_err(|e| AudioProcessingError::AfconvertFailed(e.to_string()))?;
                let result = (|| {
                    let file = File::open(&wav_path)?;
                    let decoder = rodio::Decoder::try_from(file)?;
                    import_audio_from_decoder(decoder, tmp_path, target_path)
                })();
                let _ = std::fs::remove_file(&wav_path);
                result
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err(_original_err.into())
            }
        }
    }
}

fn import_audio_from_decoder<R: std::io::Read + std::io::Seek>(
    decoder: rodio::Decoder<R>,
    tmp_path: &Path,
    target_path: &Path,
) -> Result<PathBuf, AudioProcessingError> {
    let channel_count_raw = decoder.channels().max(1);
    let channel_count_u8 = u8::try_from(channel_count_raw).map_err(|_| {
        AudioProcessingError::UnsupportedChannelCount {
            count: channel_count_raw,
        }
    })?;
    let channel_count =
        NonZeroU8::new(channel_count_u8).ok_or(AudioProcessingError::InvalidChannelCount)?;

    let samples = resample_audio(decoder, TARGET_SAMPLE_RATE_HZ)?;
    let mono_samples = if channel_count.get() > 1 {
        mix_down_to_mono(&samples, channel_count)
    } else {
        samples
    };

    if mono_samples.is_empty() {
        return Err(AudioProcessingError::EmptyInput);
    }

    let target_sample_rate = NonZeroU32::new(TARGET_SAMPLE_RATE_HZ)
        .ok_or(AudioProcessingError::InvalidTargetSampleRate)?;

    let ogg_buffer = encode_vorbis_mono(
        &mono_samples,
        target_sample_rate,
        VorbisEncodeSettings::default(),
    )?;

    write(tmp_path, &ogg_buffer)?;

    match rename(tmp_path, target_path) {
        Ok(()) => {}
        Err(err) => {
            #[cfg(unix)]
            let is_cross_device = err.raw_os_error() == Some(18);
            #[cfg(not(unix))]
            let is_cross_device = false;

            if is_cross_device {
                copy(tmp_path, target_path)?;
                remove_file(tmp_path)?;
            } else if err.kind() == ErrorKind::AlreadyExists {
                remove_file(target_path)?;
                match rename(tmp_path, target_path) {
                    Ok(()) => {}
                    Err(rename_err) => {
                        #[cfg(unix)]
                        let is_cross_device_retry = rename_err.raw_os_error() == Some(18);
                        #[cfg(not(unix))]
                        let is_cross_device_retry = false;

                        if is_cross_device_retry {
                            copy(tmp_path, target_path)?;
                            remove_file(tmp_path)?;
                        } else {
                            return Err(rename_err.into());
                        }
                    }
                }
            } else {
                return Err(err.into());
            }
        }
    }

    Ok(target_path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;

    macro_rules! test_import_audio {
        ($($name:ident: $path:expr),* $(,)?) => {
            $(
                #[test]
                fn $name() {
                    let source_path = std::path::Path::new($path);
                    let temp = TempDir::new().unwrap();
                    let tmp_path = temp.path().join("tmp.ogg");
                    let target_path = temp.path().join("target.ogg");

                    let result = import_audio(source_path, &tmp_path, &target_path);
                    assert!(result.is_ok(), "import_audio failed: {:?}", result.err());
                    assert!(target_path.exists());

                    let metadata = std::fs::metadata(&target_path).unwrap();
                    assert!(metadata.len() > 0, "Output file is empty");
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
}
