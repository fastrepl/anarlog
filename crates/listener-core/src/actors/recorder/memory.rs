use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use ractor::ActorProcessingErr;

use super::RecorderEncoder;

pub(super) struct MemorySink {
    pub(super) final_path: PathBuf,
    pub(super) encoder: RecorderEncoder,
    pub(super) data: Vec<u8>,
}

pub(super) fn create_memory_sink(session_dir: &Path) -> Result<MemorySink, ActorProcessingErr> {
    let final_path = session_dir.join("audio.mp3");
    let channels = if final_path.exists() {
        infer_audio_channels(&final_path)?
    } else {
        2
    };

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

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&sink.final_path)?;
    file.write_all(&sink.data)?;
    file.sync_all()?;
    Ok(())
}

fn infer_audio_channels(path: &Path) -> Result<u16, ActorProcessingErr> {
    use hypr_audio_utils::Source;

    let source = hypr_audio_utils::source_from_path(path).map_err(super::into_actor_err)?;
    Ok(source.channels())
}
