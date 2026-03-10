use std::fs::File;
use std::path::{Path, PathBuf};

use ractor::ActorProcessingErr;

use super::{RecorderEncoder, disk};

pub(super) struct MemorySink {
    pub(super) final_path: PathBuf,
    pub(super) encoder: RecorderEncoder,
    pub(super) data: Vec<u8>,
}

pub(super) fn create_memory_sink(session_dir: &Path) -> Result<MemorySink, ActorProcessingErr> {
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
        data: Vec::new(),
    })
}

pub(super) fn persist_memory_sink(sink: &MemorySink) -> Result<(), ActorProcessingErr> {
    if sink.data.is_empty() {
        return Ok(());
    }

    let session_dir = sink
        .final_path
        .parent()
        .ok_or_else(|| std::io::Error::other("memory sink final path missing parent"))?;

    if !disk::has_existing_audio(session_dir) {
        std::fs::write(&sink.final_path, &sink.data)?;

        if let Ok(file) = File::open(&sink.final_path) {
            let _ = file.sync_all();
        }
        if let Ok(dir) = File::open(session_dir) {
            let _ = dir.sync_all();
        }

        return Ok(());
    }

    disk::persist_encoded_audio(session_dir, &sink.data)
}
