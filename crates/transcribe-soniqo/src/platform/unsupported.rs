use std::path::{Path, PathBuf};

use crate::{
    DiarizationSegment, Error, FileTranscript, LivePartial, ModelDownloadState, Result,
    SoniqoModel, TranscriptSource,
};

pub(crate) fn model_cache_dir(_model: SoniqoModel) -> Result<PathBuf> {
    Err(Error::UnsupportedPlatform)
}

pub(crate) fn diarization_cache_dir() -> Result<PathBuf> {
    Err(Error::UnsupportedPlatform)
}

pub(crate) fn model_download_state(_model: SoniqoModel) -> Result<ModelDownloadState> {
    Err(Error::UnsupportedPlatform)
}

pub(crate) fn start_model_download(_model: SoniqoModel) -> Result<()> {
    Err(Error::UnsupportedPlatform)
}

pub(crate) fn reset_model(_model: SoniqoModel) -> Result<()> {
    Err(Error::UnsupportedPlatform)
}

pub(crate) fn transcribe_file(
    _model: SoniqoModel,
    _path: &Path,
    _language: &str,
) -> Result<FileTranscript> {
    Err(Error::UnsupportedPlatform)
}

pub(crate) fn diarize_samples(
    _model: SoniqoModel,
    _samples: &[f32],
    _exact_speakers: usize,
) -> Result<Vec<DiarizationSegment>> {
    Err(Error::UnsupportedPlatform)
}

pub(crate) fn live_start(_model: SoniqoModel) -> Result<()> {
    Err(Error::UnsupportedPlatform)
}

pub(crate) fn live_append(_source: TranscriptSource, _samples: &[f32]) -> Result<Vec<LivePartial>> {
    Err(Error::UnsupportedPlatform)
}

pub(crate) fn live_finalize(_source: TranscriptSource) -> Result<Vec<LivePartial>> {
    Err(Error::UnsupportedPlatform)
}

pub(crate) fn live_stop() -> Result<()> {
    Ok(())
}
