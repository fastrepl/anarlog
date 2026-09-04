use std::path::PathBuf;
use std::time::Duration;

use anlg_audio_utils::Source;
use owhisper_client::BatchUploadLimit;

pub(super) fn audio_duration(file_path: &str) -> Option<Duration> {
    anlg_audio_utils::source_from_path(file_path)
        .ok()
        .and_then(|source| source.total_duration())
}

/// Recordings past a provider's upload cap or per-request duration cap are
/// re-encoded as mono MP3 segments and sent one request at a time.
pub(super) fn segment_plan(
    file_path: &str,
    audio_duration: Option<Duration>,
    limit: Option<BatchUploadLimit>,
) -> Option<Duration> {
    let limit = limit?;
    let size = std::fs::metadata(file_path).ok()?.len();
    let too_long = audio_duration.is_some_and(|duration| duration > limit.max_duration);

    (size > limit.max_bytes || too_long).then_some(limit.max_duration)
}

pub(super) struct SegmentedUpload {
    _temp_dir: tempfile::TempDir,
    paths: Vec<PathBuf>,
}

impl SegmentedUpload {
    pub(super) fn paths(&self) -> &[PathBuf] {
        &self.paths
    }
}

pub(super) async fn split_batch_upload(
    file_path: &str,
    segment_duration: Duration,
    provider: &str,
) -> crate::Result<SegmentedUpload> {
    let failure = |message: &str| crate::BatchFailure::DirectRequestFailed {
        provider: provider.to_string(),
        message: message.to_string(),
    };

    let temp_dir = tempfile::tempdir().map_err(|_error| {
        tracing::error!(
            error.type = "temp_dir_create_failed",
            "batch_audio_segment_temp_dir_failed"
        );
        failure("Anarlog couldn't prepare this recording for transcription.")
    })?;

    let source = PathBuf::from(file_path);
    let output_dir = temp_dir.path().to_path_buf();
    let paths = tokio::task::spawn_blocking(move || {
        anlg_mp3::encode_mono_segments(&source, &output_dir, segment_duration)
    })
    .await
    .map_err(|_error| {
        tracing::error!(
            error.type = "local_task_join_failed",
            "batch_audio_segment_task_failed"
        );
        failure("Anarlog couldn't prepare this recording for transcription.")
    })?
    .map_err(|_error| {
        tracing::error!(
            error.type = "audio_encode_failed",
            "batch_audio_segment_failed"
        );
        failure("Anarlog couldn't split this recording for transcription.")
    })?;

    if paths.is_empty() {
        return Err(failure("This recording has no audio to transcribe.").into());
    }

    tracing::info!(
        segments = paths.len(),
        segment_seconds = segment_duration.as_secs(),
        "batch audio split for provider upload limits"
    );

    Ok(SegmentedUpload {
        _temp_dir: temp_dir,
        paths,
    })
}
