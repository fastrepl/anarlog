mod retained_audio;

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ractor::ActorProcessingErr;

use super::{RecorderEncoder, disk};
use retained_audio::{MemoryRetentionPolicy, RetainedAudio};

const DEFAULT_MEMORY_RETENTION: MemoryRetentionPolicy =
    MemoryRetentionPolicy::KeepLast(Duration::from_secs(60 * 60));

pub(super) struct MemorySink {
    pub(super) final_path: PathBuf,
    pub(super) encoder: RecorderEncoder,
    retained: RetainedAudio,
}

pub(super) fn create_memory_sink(session_dir: &Path) -> Result<MemorySink, ActorProcessingErr> {
    create_memory_sink_with_retention(session_dir, DEFAULT_MEMORY_RETENTION)
}

fn create_memory_sink_with_retention(
    session_dir: &Path,
    retention: MemoryRetentionPolicy,
) -> Result<MemorySink, ActorProcessingErr> {
    let final_path = session_dir.join("audio.mp3");
    let channels = disk::infer_existing_audio_channels(session_dir)?.unwrap_or(2);

    let encoder = if channels == 1 {
        RecorderEncoder::Mono(hypr_mp3::MonoStreamEncoder::new(super::super::SAMPLE_RATE)?)
    } else {
        RecorderEncoder::Stereo(hypr_mp3::StereoStreamEncoder::new(
            super::super::SAMPLE_RATE,
        )?)
    };

    Ok(MemorySink {
        final_path,
        encoder,
        retained: RetainedAudio::new(retention),
    })
}

pub(super) fn persist_memory_sink(sink: &MemorySink) -> Result<(), ActorProcessingErr> {
    if sink.retained.is_empty() {
        return Ok(());
    }

    let session_dir = sink
        .final_path
        .parent()
        .ok_or_else(|| std::io::Error::other("memory sink final path missing parent"))?;

    if !disk::has_existing_audio(session_dir) {
        let mut file = File::create(&sink.final_path)?;
        for bytes in sink.retained.iter_bytes() {
            file.write_all(bytes)?;
        }

        if let Ok(file) = File::open(&sink.final_path) {
            let _ = file.sync_all();
        }
        if let Ok(dir) = File::open(session_dir) {
            let _ = dir.sync_all();
        }

        return Ok(());
    }

    disk::persist_encoded_audio_chunks(session_dir, sink.retained.iter_bytes())
}

impl MemorySink {
    pub(super) fn encode_single(&mut self, samples: &[f32]) -> Result<(), ActorProcessingErr> {
        let mut bytes = Vec::new();
        self.encoder.encode_single(samples, &mut bytes)?;
        self.retained
            .push(bytes, duration_for_frames(samples.len()));
        Ok(())
    }

    pub(super) fn encode_dual(
        &mut self,
        mic: &[f32],
        spk: &[f32],
    ) -> Result<(), ActorProcessingErr> {
        let mut bytes = Vec::new();
        self.encoder.encode_dual(mic, spk, &mut bytes)?;
        self.retained
            .push(bytes, duration_for_frames(mic.len().max(spk.len())));
        Ok(())
    }

    pub(super) fn finalize(&mut self) -> Result<(), ActorProcessingErr> {
        let mut bytes = Vec::new();
        self.encoder.flush(&mut bytes)?;
        self.retained.push(bytes, Duration::ZERO);
        Ok(())
    }
}

fn duration_for_frames(frames: usize) -> Duration {
    Duration::from_secs_f64(frames as f64 / super::super::SAMPLE_RATE as f64)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn keep_last_retention_persists_decodable_suffix() -> Result<(), ActorProcessingErr> {
        let temp = tempdir()?;
        let session_dir = temp.path().join("session");
        std::fs::create_dir_all(&session_dir)?;

        let mut sink = create_memory_sink_with_retention(
            &session_dir,
            MemoryRetentionPolicy::KeepLast(Duration::from_secs(1)),
        )?;

        let chunk = vec![0.25; (super::super::super::SAMPLE_RATE / 2) as usize];
        sink.encode_single(&chunk)?;
        sink.encode_single(&chunk)?;
        sink.encode_single(&chunk)?;
        sink.finalize()?;

        persist_memory_sink(&sink)?;

        let wav_path = session_dir.join("decoded.wav");
        hypr_mp3::decode_to_wav(&session_dir.join("audio.mp3"), &wav_path)
            .map_err(|error| -> ActorProcessingErr { Box::new(error) })?;

        let reader = hound::WavReader::open(&wav_path)?;
        let decoded_frames = reader.duration();

        assert!(decoded_frames > 0);
        assert!(decoded_frames <= super::super::super::SAMPLE_RATE + 4_000);
        Ok(())
    }
}
