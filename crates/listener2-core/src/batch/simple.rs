mod direct;
mod local;

pub(super) use direct::run_direct_batch_for_adapter_kind;
pub(super) use local::{
    ResampledChannelFile, resample_audio_to_channel_files_until, run_apple_speech_batch,
    run_soniqo_batch,
};

#[cfg(test)]
use super::upload::segment_plan;
#[cfg(test)]
use direct::{
    DIRECT_BATCH_TIMEOUT_CEILING, DIRECT_BATCH_TIMEOUT_FLOOR, direct_batch_timeout_for_audio,
    merge_segment_responses, prepare_anarlog_batch_upload, run_direct_batch,
    run_direct_batch_with_timeout,
};
#[cfg(test)]
use local::{
    FixedSoniqoFileChunkIterator, LOCAL_BATCH_CANCELLED, MAX_LOCAL_BATCH_CHANNELS,
    SONIQO_DIARIZATION_MAX_SAMPLES, SONIQO_DIRECT_MIC_MIN_RMS, SONIQO_PARAKEET_MAX_CHUNK_SAMPLES,
    SONIQO_PROGRESS_MAX, SONIQO_PROGRESS_PLANNED, SoniqoChunkStrategy, audio_rms,
    collect_soniqo_channel_transcripts, ensure_soniqo_diarization_within_limit,
    resample_audio_to_channel_files, soniqo_batch_progress, soniqo_chunk_strategy,
    soniqo_diarization_plan_within_limit, soniqo_diarization_speaker_count, soniqo_language_hint,
};

#[cfg(test)]
mod tests;
