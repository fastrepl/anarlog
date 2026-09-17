use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::time::Instant;

use anlg_audio_utils::{
    decode_vorbis_to_mono_wav_file, decode_vorbis_to_wav_file, ogg_has_identical_channels,
};
use ractor::ActorProcessingErr;

use super::into_actor_err;
#[cfg(test)]
use anlg_audio_utils::mix_audio_f32;

const FINAL_AUDIO_FILE: &str = "audio.mp3";
const WAV_FILE: &str = "audio.wav";
const OGG_FILE: &str = "audio.ogg";
#[cfg(test)]
const FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1000);

pub(super) struct DiskSink {
    writer: Option<hound::WavWriter<BufWriter<File>>>,
    writer_mic: Option<hound::WavWriter<BufWriter<File>>>,
    writer_spk: Option<hound::WavWriter<BufWriter<File>>>,
    wav_path: PathBuf,
    #[cfg(test)]
    last_flush: Instant,
    #[cfg(test)]
    is_stereo: bool,
    pub(super) recovered_audio: bool,
}

pub(super) fn create_disk_sink(session_dir: &Path) -> Result<DiskSink, ActorProcessingErr> {
    let wav_path = session_dir.join(WAV_FILE);
    let ogg_path = session_dir.join(OGG_FILE);
    let encoded_path = session_dir.join(FINAL_AUDIO_FILE);
    let recovered_audio = if !encoded_path.exists() && !ogg_path.exists() {
        preserve_invalid_wav(&wav_path)?
    } else {
        false
    };
    let has_existing_audio = encoded_path.exists() || ogg_path.exists() || wav_path.exists();
    let is_stereo =
        prepare_existing_audio_state(&encoded_path, &ogg_path, &wav_path)? || !has_existing_audio;

    let spec = hound::WavSpec {
        channels: if is_stereo { 2 } else { 1 },
        sample_rate: super::super::SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let writer = if wav_path.exists() {
        hound::WavWriter::append(&wav_path)?
    } else {
        hound::WavWriter::create(&wav_path, spec)?
    };

    let mono_spec = hound::WavSpec {
        channels: 1,
        ..spec
    };

    let (writer_mic, writer_spk) = if is_debug_mode() {
        let mic_path = session_dir.join("audio_mic.wav");
        let spk_path = session_dir.join("audio_spk.wav");

        preserve_invalid_wav(&mic_path)?;
        preserve_invalid_wav(&spk_path)?;

        let mic_writer = if mic_path.exists() {
            hound::WavWriter::append(&mic_path)?
        } else {
            hound::WavWriter::create(&mic_path, mono_spec)?
        };

        let spk_writer = if spk_path.exists() {
            hound::WavWriter::append(&spk_path)?
        } else {
            hound::WavWriter::create(&spk_path, mono_spec)?
        };

        (Some(mic_writer), Some(spk_writer))
    } else {
        (None, None)
    };

    let mut sink = DiskSink {
        writer: Some(writer),
        writer_mic,
        writer_spk,
        wav_path,
        #[cfg(test)]
        last_flush: Instant::now(),
        #[cfg(test)]
        is_stereo,
        recovered_audio,
    };
    // Capture can stall before its first samples; leave a readable header on disk.
    flush_all(&mut sink)?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&sink.wav_path)?
        .sync_all()?;
    sync_dir(&sink.wav_path);
    Ok(sink)
}

#[cfg(test)]
pub(super) fn write_single(sink: &mut DiskSink, samples: &[f32]) -> Result<(), ActorProcessingErr> {
    if let Some(writer) = sink.writer.as_mut() {
        if sink.is_stereo {
            write_mono_as_stereo(writer, samples)?;
        } else {
            write_mono_samples(writer, samples)?;
        }
    }

    flush_if_due(sink)?;
    Ok(())
}

#[cfg(test)]
pub(super) fn write_dual(
    sink: &mut DiskSink,
    mic: &[f32],
    spk: &[f32],
) -> Result<(), ActorProcessingErr> {
    if let Some(writer) = sink.writer.as_mut() {
        if sink.is_stereo {
            write_interleaved_stereo(writer, mic, spk)?;
        } else {
            let mixed = mix_audio_f32(mic, spk);
            write_mono_samples(writer, &mixed)?;
        }
    }

    if let Some(writer_mic) = sink.writer_mic.as_mut() {
        write_mono_samples(writer_mic, mic)?;
    }

    if let Some(writer_spk) = sink.writer_spk.as_mut() {
        write_mono_samples(writer_spk, spk)?;
    }

    flush_if_due(sink)?;
    Ok(())
}

pub(super) fn finalize_disk_sink(sink: &mut DiskSink) -> Result<(), ActorProcessingErr> {
    finalize_writer(&mut sink.writer, Some(&sink.wav_path))?;
    finalize_writer(&mut sink.writer_mic, None)?;
    finalize_writer(&mut sink.writer_spk, None)?;

    if sink.wav_path.exists() {
        let encoded_path = sink.wav_path.with_extension("mp3");
        match anlg_mp3::encode_wav(&sink.wav_path, &encoded_path) {
            Ok(()) => {
                sync_file(&encoded_path);
                sync_dir(&encoded_path);
                std::fs::remove_file(&sink.wav_path)?;
                sync_dir(&sink.wav_path);
            }
            Err(error) => {
                tracing::error!("Encoding to mp3 failed, keeping WAV: {}", error);
                sync_file(&sink.wav_path);
                sync_dir(&sink.wav_path);
            }
        }
    }

    Ok(())
}

fn prepare_existing_audio_state(
    encoded_path: &Path,
    ogg_path: &Path,
    wav_path: &Path,
) -> Result<bool, ActorProcessingErr> {
    if encoded_path.exists() {
        decode_mp3_to_wav(encoded_path, wav_path)?;
        std::fs::remove_file(encoded_path)?;
        return Ok(wav_is_stereo(wav_path)?);
    }

    if ogg_path.exists() {
        let has_identical = ogg_has_identical_channels(ogg_path).map_err(into_actor_err)?;
        if has_identical {
            decode_vorbis_to_mono_wav_file(ogg_path, wav_path).map_err(into_actor_err)?;
        } else {
            decode_vorbis_to_wav_file(ogg_path, wav_path).map_err(into_actor_err)?;
        }
        std::fs::remove_file(ogg_path)?;
        return Ok(!has_identical);
    }

    if wav_path.exists() {
        return Ok(wav_is_stereo(wav_path)?);
    }

    Ok(false)
}

fn decode_mp3_to_wav(encoded_path: &Path, wav_path: &Path) -> Result<(), ActorProcessingErr> {
    let tmp_path = wav_path.with_extension("wav.tmp");
    if tmp_path.exists() {
        std::fs::remove_file(&tmp_path)?;
    }

    anlg_mp3::decode_to_wav(encoded_path, &tmp_path).map_err(into_actor_err)?;

    if wav_path.exists() {
        std::fs::remove_file(wav_path)?;
    }
    std::fs::rename(tmp_path, wav_path)?;
    Ok(())
}

fn wav_is_stereo(wav_path: &Path) -> Result<bool, hound::Error> {
    let reader = hound::WavReader::open(wav_path)?;
    Ok(reader.spec().channels == 2)
}

fn preserve_invalid_wav(path: &Path) -> Result<bool, ActorProcessingErr> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let invalid = match hound::WavReader::new(file) {
        Ok(_) => false,
        Err(hound::Error::FormatError(_)) => true,
        Err(hound::Error::IoError(error))
            if error.kind() == std::io::ErrorKind::UnexpectedEof
                || (error.kind() == std::io::ErrorKind::Other
                    && error.to_string() == "Failed to read enough bytes.") =>
        {
            true
        }
        Err(error) => return Err(error.into()),
    };
    if !invalid {
        return Ok(false);
    }

    let recovery_path = path.with_extension(format!("recovery-{}.wav", uuid::Uuid::new_v4()));
    // Preserve the original until a durable backup exists, including on filesystems without links.
    if let Err(link_error) = std::fs::hard_link(path, &recovery_path) {
        tracing::debug!(?link_error, "recording_backup_copy_fallback");
        copy_recovery_backup(path, &recovery_path)?;
    }
    std::fs::OpenOptions::new()
        .write(true)
        .open(&recovery_path)?
        .sync_all()?;
    sync_dir(&recovery_path);
    std::fs::remove_file(path)?;
    sync_dir(path);
    tracing::warn!("invalid_recording_preserved_for_recovery");
    Ok(true)
}

fn copy_recovery_backup(path: &Path, recovery_path: &Path) -> std::io::Result<()> {
    let mut source = File::open(path)?;
    let mut backup = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(recovery_path)?;
    std::io::copy(&mut source, &mut backup)?;
    Ok(())
}

fn is_debug_mode() -> bool {
    cfg!(debug_assertions)
        || std::env::var("LISTENER_DEBUG")
            .map(|value| !value.is_empty() && value != "0" && value != "false")
            .unwrap_or(false)
}

#[cfg(test)]
fn flush_if_due(sink: &mut DiskSink) -> Result<(), hound::Error> {
    if sink.last_flush.elapsed() < FLUSH_INTERVAL {
        return Ok(());
    }

    flush_all(sink)
}

fn flush_all(sink: &mut DiskSink) -> Result<(), hound::Error> {
    if let Some(writer) = sink.writer.as_mut() {
        writer.flush()?;
    }
    if let Some(writer_mic) = sink.writer_mic.as_mut() {
        writer_mic.flush()?;
    }
    if let Some(writer_spk) = sink.writer_spk.as_mut() {
        writer_spk.flush()?;
    }
    #[cfg(test)]
    {
        sink.last_flush = Instant::now();
    }
    Ok(())
}

#[cfg(test)]
fn write_mono_samples(
    writer: &mut hound::WavWriter<BufWriter<File>>,
    samples: &[f32],
) -> Result<(), hound::Error> {
    for sample in samples {
        writer.write_sample(*sample)?;
    }
    Ok(())
}

#[cfg(test)]
fn write_mono_as_stereo(
    writer: &mut hound::WavWriter<BufWriter<File>>,
    samples: &[f32],
) -> Result<(), hound::Error> {
    for sample in samples {
        writer.write_sample(*sample)?;
        writer.write_sample(*sample)?;
    }
    Ok(())
}

#[cfg(test)]
fn write_interleaved_stereo(
    writer: &mut hound::WavWriter<BufWriter<File>>,
    mic: &[f32],
    spk: &[f32],
) -> Result<(), hound::Error> {
    let frames = mic.len().max(spk.len());
    for i in 0..frames {
        writer.write_sample(mic.get(i).copied().unwrap_or(0.0))?;
        writer.write_sample(spk.get(i).copied().unwrap_or(0.0))?;
    }
    Ok(())
}

fn finalize_writer(
    writer: &mut Option<hound::WavWriter<BufWriter<File>>>,
    path: Option<&Path>,
) -> Result<(), hound::Error> {
    if let Some(mut writer) = writer.take() {
        writer.flush()?;
        writer.finalize()?;

        if let Some(path) = path {
            sync_file(path);
        }
    }
    Ok(())
}

fn sync_file(path: &Path) {
    if let Ok(file) = File::open(path) {
        let _ = file.sync_all();
    }
}

fn sync_dir(path: &Path) {
    if let Some(parent) = path.parent()
        && let Ok(dir) = File::open(parent)
    {
        let _ = dir.sync_all();
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::actors::SAMPLE_RATE;

    use super::*;

    #[test]
    fn copy_fallback_preserves_bytes_without_overwriting_an_existing_backup() {
        let dir = tempdir().unwrap();
        let original = dir.path().join("audio.wav");
        let backup = dir.path().join("audio.recovery.wav");
        std::fs::write(&original, b"damaged recording bytes").unwrap();
        copy_recovery_backup(&original, &backup).unwrap();
        assert_eq!(std::fs::read(&backup).unwrap(), b"damaged recording bytes");
        std::fs::write(&original, b"new damaged recording").unwrap();
        assert_eq!(
            copy_recovery_backup(&original, &backup).unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read(&backup).unwrap(), b"damaged recording bytes");
        assert_eq!(std::fs::read(&original).unwrap(), b"new damaged recording");
    }

    #[test]
    fn header_is_readable_before_any_audio_arrives() {
        let dir = tempdir().unwrap();
        let _sink = create_disk_sink(dir.path()).unwrap();
        let reader = hound::WavReader::open(dir.path().join(WAV_FILE)).unwrap();
        assert_eq!(reader.spec().channels, 2);
        assert_eq!(reader.len(), 0);
        if is_debug_mode() {
            for name in ["audio_mic.wav", "audio_spk.wav"] {
                assert_eq!(
                    hound::WavReader::open(dir.path().join(name)).unwrap().len(),
                    0
                );
            }
        }
    }

    #[test]
    fn resume_preserves_invalid_wavs_and_records_new_audio() {
        for bytes in [b"".as_slice(), b"RIFF\x24\x00", b"not a wave file"] {
            let dir = tempdir().unwrap();
            let path = dir.path().join(WAV_FILE);
            std::fs::write(&path, bytes).unwrap();
            let mut sink = create_disk_sink(dir.path()).unwrap();
            assert!(sink.recovered_audio);
            assert!(sink.is_stereo);
            let backup = std::fs::read_dir(dir.path())
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| path.to_string_lossy().contains(".recovery-"))
                .unwrap();
            assert_eq!(std::fs::read(&backup).unwrap(), bytes);
            write_dual(&mut sink, &[0.25], &[0.5]).unwrap();
            flush_all(&mut sink).unwrap();
            let samples = hound::WavReader::open(&path)
                .unwrap()
                .samples::<f32>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert_eq!(samples, vec![0.25, 0.5]);
            assert_eq!(std::fs::read(backup).unwrap(), bytes);
        }
    }

    #[test]
    fn readable_header_keeps_partial_audio_available_for_append() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(WAV_FILE);
        write_test_wav(&path, 128);
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(file.metadata().unwrap().len() - 4).unwrap();
        let original = std::fs::read(&path).unwrap();
        assert!(!preserve_invalid_wav(&path).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn resume_after_startup_without_samples_keeps_valid_audio() {
        let dir = tempdir().unwrap();
        let mut sink = create_disk_sink(dir.path()).unwrap();
        // Copy only the bytes already visible on disk, without dropping/finalizing the writer.
        let interrupted = tempdir().unwrap();
        std::fs::copy(dir.path().join(WAV_FILE), interrupted.path().join(WAV_FILE)).unwrap();
        let mut resumed = create_disk_sink(interrupted.path()).unwrap();
        assert!(!resumed.recovered_audio);
        write_single(&mut resumed, &[0.25]).unwrap();
        flush_all(&mut resumed).unwrap();
        assert_eq!(
            hound::WavReader::open(interrupted.path().join(WAV_FILE))
                .unwrap()
                .len(),
            2
        );
        write_single(&mut sink, &[0.5]).unwrap();
        flush_all(&mut sink).unwrap();
        drop(sink);
        let mut appended = create_disk_sink(dir.path()).unwrap();
        assert!(!appended.recovered_audio);
        write_single(&mut appended, &[0.25]).unwrap();
        flush_all(&mut appended).unwrap();
        assert_eq!(
            hound::WavReader::open(dir.path().join(WAV_FILE))
                .unwrap()
                .samples::<f32>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            vec![0.5, 0.5, 0.25, 0.25]
        );
    }

    #[test]
    fn repeated_recovery_keeps_every_backup() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(WAV_FILE);
        for bytes in [b"first interrupted header", b"later interrupted header"] {
            std::fs::write(&path, bytes).unwrap();
            assert!(preserve_invalid_wav(&path).unwrap());
        }
        let mut backups = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| std::fs::read(entry.unwrap().path()).unwrap())
            .collect::<Vec<_>>();
        backups.sort();
        assert_eq!(
            backups,
            vec![
                b"first interrupted header".to_vec(),
                b"later interrupted header".to_vec()
            ]
        );
    }

    #[test]
    fn damaged_debug_audio_does_not_block_a_valid_recording() {
        if !is_debug_mode() {
            return;
        }
        let dir = tempdir().unwrap();
        write_test_wav(&dir.path().join(WAV_FILE), 128);
        for name in ["audio_mic.wav", "audio_spk.wav"] {
            std::fs::write(dir.path().join(name), b"RIFF").unwrap();
        }
        let sink = create_disk_sink(dir.path()).unwrap();
        assert!(!sink.recovered_audio);
        assert_eq!(
            hound::WavReader::open(dir.path().join(WAV_FILE))
                .unwrap()
                .len(),
            128
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 5);
    }

    #[test]
    fn recovery_does_not_treat_io_errors_as_corruption() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(WAV_FILE);
        std::fs::create_dir(&path).unwrap();
        assert!(preserve_invalid_wav(&path).is_err());
        assert!(path.is_dir());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn create_disk_sink_decodes_existing_mp3_to_wav() {
        let dir = tempdir().unwrap();
        let session_dir = dir.path().join("session");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::copy(
            anlg_data::english_1::AUDIO_MP3_PATH,
            session_dir.join(FINAL_AUDIO_FILE),
        )
        .unwrap();

        let _sink = create_disk_sink(&session_dir).unwrap();

        assert!(session_dir.join(WAV_FILE).exists());
        assert!(!session_dir.join(FINAL_AUDIO_FILE).exists());
    }

    #[test]
    fn create_disk_sink_preserves_new_recording_channels() {
        let dir = tempdir().unwrap();
        let session_dir = dir.path().join("session");
        std::fs::create_dir_all(&session_dir).unwrap();

        let mut sink = create_disk_sink(&session_dir).unwrap();
        write_dual(&mut sink, &[0.25, -0.25], &[0.5, 0.5]).unwrap();
        finalize_writer(&mut sink.writer, Some(&sink.wav_path)).unwrap();

        let mut reader = hound::WavReader::open(session_dir.join(WAV_FILE)).unwrap();
        assert_eq!(reader.spec().channels, 2);
        assert_eq!(
            reader
                .samples::<f32>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            vec![0.25, 0.5, -0.25, 0.5]
        );
    }

    #[test]
    fn create_disk_sink_duplicates_new_single_channel_recordings() {
        let dir = tempdir().unwrap();
        let session_dir = dir.path().join("session");
        std::fs::create_dir_all(&session_dir).unwrap();

        let mut sink = create_disk_sink(&session_dir).unwrap();
        write_single(&mut sink, &[0.25, -0.25]).unwrap();
        finalize_writer(&mut sink.writer, Some(&sink.wav_path)).unwrap();

        let mut reader = hound::WavReader::open(session_dir.join(WAV_FILE)).unwrap();
        assert_eq!(reader.spec().channels, 2);
        assert_eq!(
            reader
                .samples::<f32>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            vec![0.25, 0.25, -0.25, -0.25]
        );
    }

    #[test]
    fn create_disk_sink_prefers_existing_mp3_over_stale_wav() {
        let dir = tempdir().unwrap();
        let session_dir = dir.path().join("session");
        std::fs::create_dir_all(&session_dir).unwrap();
        let encoded_path = session_dir.join(FINAL_AUDIO_FILE);
        let wav_path = session_dir.join(WAV_FILE);
        std::fs::copy(anlg_data::english_1::AUDIO_MP3_PATH, &encoded_path).unwrap();
        write_test_wav(&wav_path, 128);
        let original_frames = decoded_frame_count(&encoded_path);

        let mut sink = create_disk_sink(&session_dir).unwrap();
        write_single(&mut sink, &vec![0.0; SAMPLE_RATE as usize]).unwrap();
        finalize_disk_sink(&mut sink).unwrap();

        assert!(!wav_path.exists());
        assert!(encoded_path.exists());
        assert!(decoded_frame_count(&encoded_path) > original_frames);
    }

    #[test]
    fn create_disk_sink_keeps_legacy_wav_for_append() {
        let dir = tempdir().unwrap();
        let session_dir = dir.path().join("session");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::copy(anlg_data::english_1::AUDIO_PATH, session_dir.join(WAV_FILE)).unwrap();

        let _sink = create_disk_sink(&session_dir).unwrap();

        assert!(session_dir.join(WAV_FILE).exists());
        assert!(!session_dir.join(FINAL_AUDIO_FILE).exists());
    }

    fn decoded_frame_count(path: &Path) -> usize {
        use anlg_audio_utils::Source;

        let source = anlg_audio_utils::source_from_path(path).unwrap();
        let channels = u16::from(source.channels()).max(1) as usize;
        source.count() / channels
    }

    fn write_test_wav(path: &Path, frames: usize) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for _ in 0..frames {
            writer.write_sample(0.0f32).unwrap();
        }
        writer.finalize().unwrap();
    }
}
