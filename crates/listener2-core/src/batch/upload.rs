use std::path::PathBuf;
use std::time::Duration;

use anlg_audio_utils::Source;
use owhisper_client::BatchUploadLimit;

pub(in crate::batch) fn temporary_audio_directory(
    source: &str,
) -> std::io::Result<tempfile::TempDir> {
    let parent = std::path::Path::new(source).parent();
    if let Some(parent) = parent
        && parent
            .file_name()
            .is_some_and(|name| name == "audio-recovery")
    {
        // Derived audio must share the capture's deletion boundary, including
        // cancellation while a blocking encoder is still finishing.
        tempfile::tempdir_in(parent)
    } else {
        tempfile::tempdir()
    }
}

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

    let temp_dir = temporary_audio_directory(file_path).map_err(|_error| {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_uploads_stay_inside_the_capture_deletion_boundary() {
        let root = tempfile::tempdir().unwrap();
        let recovery = root.path().join("audio-recovery");
        std::fs::create_dir(&recovery).unwrap();
        let source = recovery.join("chunk.mp3");
        let temporary = temporary_audio_directory(source.to_str().unwrap()).unwrap();
        assert!(temporary.path().starts_with(&recovery));
        std::fs::write(temporary.path().join("derived.wav"), b"private audio").unwrap();
        std::fs::remove_dir_all(&recovery).unwrap();
        assert!(!temporary.path().exists());
        assert!(temporary_audio_directory(source.to_str().unwrap()).is_err());
    }
}
