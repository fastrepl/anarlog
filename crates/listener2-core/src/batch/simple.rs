mod direct;
mod local;

pub(super) use direct::run_direct_batch_for_adapter_kind;
pub(super) use local::{run_apple_speech_batch, run_soniqo_batch};

#[cfg(test)]
use direct::{
    DIRECT_BATCH_TIMEOUT_CEILING, DIRECT_BATCH_TIMEOUT_FLOOR, direct_batch_timeout_for_audio,
    prepare_anarlog_batch_upload, run_direct_batch_with_timeout,
};
#[cfg(test)]
use local::{
    SONIQO_DIRECT_MIC_MIN_RMS, SONIQO_PARAKEET_MAX_CHUNK_SAMPLES, SONIQO_PROGRESS_MAX,
    SONIQO_PROGRESS_PLANNED, audio_rms, collapse_identical_channels,
    collect_soniqo_channel_transcripts, soniqo_batch_progress, soniqo_channel_chunks,
    soniqo_diarization_speaker_count, soniqo_language_hint, uses_resilient_soniqo_chunking,
};

#[cfg(test)]
mod tests;
