use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use hypr_audio_utils::{
    decode_vorbis_to_mono_wav_file, decode_vorbis_to_wav_file, mix_audio_f32,
    ogg_has_identical_channels,
};
use ractor::ActorProcessingErr;

use super::into_actor_err;

const FINAL_AUDIO_FILE: &str = "audio.mp3";
const WAV_FILE: &str = "audio.wav";
const OGG_FILE: &str = "audio.ogg";
const FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1000);

pub(super) struct DiskSink {
    writer: Option<hound::WavWriter<BufWriter<File>>>,
    writer_mic: Option<hound::WavWriter<BufWriter<File>>>,
    writer_spk: Option<hound::WavWriter<BufWriter<File>>>,
    wav_path: PathBuf,
    last_flush: Instant,
    is_stereo: bool,
}

pub(super) fn has_existing_audio(session_dir: &Path) -> bool {
    let wav_path = session_dir.join(WAV_FILE);
    let ogg_path = session_dir.join(OGG_FILE);
    let encoded_path = session_dir.join(FINAL_AUDIO_FILE);

    encoded_path.exists() || wav_path.exists() || ogg_path.exists()
}

pub(super) fn infer_existing_audio_channels(
    session_dir: &Path,
) -> Result<Option<u16>, ActorProcessingErr> {
    use hypr_audio_utils::Source;

    let wav_path = session_dir.join(WAV_FILE);
    let ogg_path = session_dir.join(OGG_FILE);
    let encoded_path = session_dir.join(FINAL_AUDIO_FILE);

    if ogg_path.exists() {
        let has_identical = ogg_has_identical_channels(&ogg_path).map_err(into_actor_err)?;
        return Ok(Some(if has_identical { 1 } else { 2 }));
    }

    if wav_path.exists() {
        let reader = hound::WavReader::open(&wav_path)?;
        return Ok(Some(reader.spec().channels));
    }

    if encoded_path.exists() {
        let source = hypr_audio_utils::source_from_path(&encoded_path).map_err(into_actor_err)?;
        return Ok(Some(source.channels()));
    }

    Ok(None)
}

pub(super) fn create_disk_sink(session_dir: &Path) -> Result<DiskSink, ActorProcessingErr> {
    let wav_path = session_dir.join(WAV_FILE);
    let ogg_path = session_dir.join(OGG_FILE);
    let encoded_path = session_dir.join(FINAL_AUDIO_FILE);
    let is_stereo = prepare_existing_audio_state(&encoded_path, &ogg_path, &wav_path)?;

    let stereo_spec = hound::WavSpec {
        channels: 2,
        sample_rate: super::super::SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mono_spec = hound::WavSpec {
        channels: 1,
        sample_rate: super::super::SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let writer = if wav_path.exists() {
        hound::WavWriter::append(&wav_path)?
    } else if is_stereo {
        hound::WavWriter::create(&wav_path, stereo_spec)?
    } else {
        hound::WavWriter::create(&wav_path, mono_spec)?
    };

    let (writer_mic, writer_spk) = if is_debug_mode() {
        let mic_path = session_dir.join("audio_mic.wav");
        let spk_path = session_dir.join("audio_spk.wav");

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

    Ok(DiskSink {
        writer: Some(writer),
        writer_mic,
        writer_spk,
        wav_path,
        last_flush: Instant::now(),
        is_stereo,
    })
}

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
        match hypr_mp3::encode_wav(&sink.wav_path, &encoded_path) {
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

pub(super) fn persist_encoded_audio_chunks<'a, I>(
    session_dir: &Path,
    encoded_audio_chunks: I,
) -> Result<(), ActorProcessingErr>
where
    I: IntoIterator<Item = &'a [u8]>,
{
    let temp_mp3_path = session_dir.join("audio.in-memory.mp3");
    let temp_wav_path = session_dir.join("audio.in-memory.wav");

    let mut wrote_any = false;
    {
        let mut file = File::create(&temp_mp3_path)?;
        for chunk in encoded_audio_chunks {
            if chunk.is_empty() {
                continue;
            }

            file.write_all(chunk)?;
            wrote_any = true;
        }
        file.flush()?;
    }

    if !wrote_any {
        let _ = std::fs::remove_file(&temp_mp3_path);
        return Ok(());
    }

    let result = (|| -> Result<(), ActorProcessingErr> {
        let mut sink = create_disk_sink(session_dir)?;
        append_encoded_audio_to_sink(&mut sink, &temp_mp3_path, &temp_wav_path)?;
        finalize_disk_sink(&mut sink)
    })();

    let _ = std::fs::remove_file(&temp_mp3_path);
    let _ = std::fs::remove_file(&temp_wav_path);

    result
}

fn prepare_existing_audio_state(
    encoded_path: &Path,
    ogg_path: &Path,
    wav_path: &Path,
) -> Result<bool, ActorProcessingErr> {
    if encoded_path.exists() && !wav_path.exists() {
        hypr_mp3::decode_to_wav(encoded_path, wav_path).map_err(into_actor_err)?;
        std::fs::remove_file(encoded_path)?;
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
        let reader = hound::WavReader::open(wav_path)?;
        return Ok(reader.spec().channels == 2);
    }

    Ok(true)
}

fn append_encoded_audio_to_sink(
    sink: &mut DiskSink,
    mp3_path: &Path,
    wav_path: &Path,
) -> Result<(), ActorProcessingErr> {
    hypr_mp3::decode_to_wav(mp3_path, wav_path).map_err(into_actor_err)?;

    let mut reader = hound::WavReader::open(wav_path)?;
    let channels = reader.spec().channels;

    match channels {
        1 => {
            let mut samples = Vec::new();
            for sample in reader.samples::<f32>() {
                samples.push(sample?);
            }
            write_single(sink, &samples)?;
        }
        2 => {
            let mut left = Vec::new();
            let mut right = Vec::new();

            for (index, sample) in reader.samples::<f32>().enumerate() {
                let sample = sample?;
                if index % 2 == 0 {
                    left.push(sample);
                } else {
                    right.push(sample);
                }
            }

            write_dual(sink, &left, &right)?;
        }
        count => {
            return Err(into_actor_err(hypr_mp3::Error::UnsupportedChannelCount(
                count,
            )));
        }
    }

    Ok(())
}

fn is_debug_mode() -> bool {
    cfg!(debug_assertions)
        || std::env::var("LISTENER_DEBUG")
            .map(|value| !value.is_empty() && value != "0" && value != "false")
            .unwrap_or(false)
}

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
    sink.last_flush = Instant::now();
    Ok(())
}

fn write_mono_samples(
    writer: &mut hound::WavWriter<BufWriter<File>>,
    samples: &[f32],
) -> Result<(), hound::Error> {
    for sample in samples {
        writer.write_sample(*sample)?;
    }
    Ok(())
}

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

    use super::*;

    #[test]
    fn create_disk_sink_decodes_existing_mp3_to_wav() {
        let dir = tempdir().unwrap();
        let session_dir = dir.path().join("session");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::copy(
            hypr_data::english_1::AUDIO_MP3_PATH,
            session_dir.join(FINAL_AUDIO_FILE),
        )
        .unwrap();

        let _sink = create_disk_sink(&session_dir).unwrap();

        assert!(session_dir.join(WAV_FILE).exists());
        assert!(!session_dir.join(FINAL_AUDIO_FILE).exists());
    }

    #[test]
    fn create_disk_sink_keeps_legacy_wav_for_append() {
        let dir = tempdir().unwrap();
        let session_dir = dir.path().join("session");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::copy(hypr_data::english_1::AUDIO_PATH, session_dir.join(WAV_FILE)).unwrap();

        let _sink = create_disk_sink(&session_dir).unwrap();

        assert!(session_dir.join(WAV_FILE).exists());
        assert!(!session_dir.join(FINAL_AUDIO_FILE).exists());
    }
}
