use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use hypr_audio_utils::{
    decode_vorbis_to_mono_wav_file, decode_vorbis_to_wav_file, ogg_has_identical_channels,
};
use ractor::ActorProcessingErr;

use super::{RecorderEncoder, into_actor_err};

const FINAL_AUDIO_FILE: &str = "audio.mp3";
const LEGACY_WAV_FILE: &str = "audio.wav";
const LEGACY_OGG_FILE: &str = "audio.ogg";
const FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1000);

pub(super) struct DiskSink {
    pub(super) final_path: PathBuf,
    pub(super) writer: BufWriter<File>,
    pub(super) encoder: RecorderEncoder,
    pub(super) encoded: Vec<u8>,
    pub(super) last_flush: Instant,
}

struct PreparedDiskState {
    is_stereo: bool,
    final_path: PathBuf,
}

pub(super) fn create_disk_sink(session_dir: &Path) -> Result<DiskSink, ActorProcessingErr> {
    let prepared = prepare_disk_state(session_dir)?;
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&prepared.final_path)?;
    let writer = BufWriter::new(file);
    let encoder = if prepared.is_stereo {
        RecorderEncoder::Stereo(hypr_mp3::StereoStreamEncoder::new(
            super::super::SAMPLE_RATE,
        )?)
    } else {
        RecorderEncoder::Mono(hypr_mp3::MonoStreamEncoder::new(super::super::SAMPLE_RATE)?)
    };

    Ok(DiskSink {
        final_path: prepared.final_path,
        writer,
        encoder,
        encoded: Vec::new(),
        last_flush: Instant::now(),
    })
}

pub(super) fn write_pending_disk_bytes(sink: &mut DiskSink) -> Result<(), ActorProcessingErr> {
    if sink.encoded.is_empty() {
        return Ok(());
    }

    sink.writer.write_all(&sink.encoded)?;
    sink.encoded.clear();
    Ok(())
}

pub(super) fn flush_disk_if_due(sink: &mut DiskSink) -> Result<(), ActorProcessingErr> {
    if sink.last_flush.elapsed() < FLUSH_INTERVAL {
        return Ok(());
    }

    flush_disk_sink(sink, true)
}

pub(super) fn flush_disk_sink(sink: &mut DiskSink, sync: bool) -> Result<(), ActorProcessingErr> {
    sink.writer.flush()?;
    if sync {
        sync_file(&sink.final_path);
    }
    sink.last_flush = Instant::now();
    Ok(())
}

fn prepare_disk_state(session_dir: &Path) -> Result<PreparedDiskState, ActorProcessingErr> {
    let final_path = session_dir.join(FINAL_AUDIO_FILE);
    let legacy_wav_path = session_dir.join(LEGACY_WAV_FILE);
    let legacy_ogg_path = session_dir.join(LEGACY_OGG_FILE);

    let mut is_stereo = true;

    if final_path.exists() {
        is_stereo = infer_audio_channels(&final_path)? == 2;
    } else if legacy_ogg_path.exists() {
        is_stereo = !ogg_has_identical_channels(&legacy_ogg_path).map_err(into_actor_err)?;
        migrate_legacy_ogg_to_mp3(&legacy_ogg_path, &final_path, is_stereo)?;
        std::fs::remove_file(&legacy_ogg_path)?;
    } else if legacy_wav_path.exists() {
        is_stereo = infer_wav_channels(&legacy_wav_path)? == 2;
        hypr_mp3::encode_wav(&legacy_wav_path, &final_path).map_err(into_actor_err)?;
        std::fs::remove_file(&legacy_wav_path)?;
    }

    Ok(PreparedDiskState {
        is_stereo,
        final_path,
    })
}

fn migrate_legacy_ogg_to_mp3(
    ogg_path: &Path,
    final_path: &Path,
    is_stereo: bool,
) -> Result<(), ActorProcessingErr> {
    let temp_wav_path = ogg_path.with_extension("migration.wav");
    if temp_wav_path.exists() {
        std::fs::remove_file(&temp_wav_path)?;
    }

    if is_stereo {
        decode_vorbis_to_wav_file(ogg_path, &temp_wav_path).map_err(into_actor_err)?;
    } else {
        decode_vorbis_to_mono_wav_file(ogg_path, &temp_wav_path).map_err(into_actor_err)?;
    }

    let result = hypr_mp3::encode_wav(&temp_wav_path, final_path).map_err(into_actor_err);
    let _ = std::fs::remove_file(&temp_wav_path);
    result
}

fn infer_audio_channels(path: &Path) -> Result<u16, ActorProcessingErr> {
    use hypr_audio_utils::Source;

    let source = hypr_audio_utils::source_from_path(path).map_err(into_actor_err)?;
    Ok(source.channels())
}

fn infer_wav_channels(path: &Path) -> Result<u16, ActorProcessingErr> {
    let reader = hound::WavReader::open(path)?;
    Ok(reader.spec().channels)
}

fn sync_file(path: &Path) {
    if let Ok(file) = File::open(path) {
        let _ = file.sync_all();
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn prepare_disk_state_keeps_existing_mp3() {
        let dir = tempdir().unwrap();
        let session_dir = dir.path().join("session");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::copy(
            hypr_data::english_1::AUDIO_MP3_PATH,
            session_dir.join(FINAL_AUDIO_FILE),
        )
        .unwrap();

        let prepared = prepare_disk_state(&session_dir).unwrap();

        assert!(prepared.final_path.exists());
        assert!(!prepared.is_stereo);
    }

    #[test]
    fn prepare_disk_state_converts_legacy_wav() {
        let dir = tempdir().unwrap();
        let session_dir = dir.path().join("session");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::copy(
            hypr_data::english_1::AUDIO_PATH,
            session_dir.join(LEGACY_WAV_FILE),
        )
        .unwrap();

        let prepared = prepare_disk_state(&session_dir).unwrap();

        assert!(prepared.final_path.exists());
        assert!(!session_dir.join(LEGACY_WAV_FILE).exists());
    }

    #[test]
    fn write_pending_disk_bytes_appends_encoded_audio() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("audio.mp3");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap();

        let mut sink = DiskSink {
            final_path: path.clone(),
            writer: BufWriter::new(file),
            encoder: RecorderEncoder::Stereo(
                hypr_mp3::StereoStreamEncoder::new(super::super::super::SAMPLE_RATE).unwrap(),
            ),
            encoded: vec![1, 2, 3, 4],
            last_flush: Instant::now(),
        };

        write_pending_disk_bytes(&mut sink).unwrap();
        flush_disk_sink(&mut sink, true).unwrap();

        assert_eq!(std::fs::read(path).unwrap(), vec![1, 2, 3, 4]);
    }
}
