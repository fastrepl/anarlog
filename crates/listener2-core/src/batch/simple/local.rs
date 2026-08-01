use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use owhisper_interface::batch_stream::BatchStreamEvent;
use tracing::Instrument;

use anlg_audio_chunking::AudioChunk;
use anlg_audio_utils::Source;
use anlg_transcribe_core::{
    TARGET_SAMPLE_RATE, channel_duration_sec, chunk_channel_audio, split_resampled_channels,
};

use super::super::{
    BatchParams, BatchRunMode, BatchRunOutput, format_user_friendly_error, session_span,
};
use crate::{BatchEvent, BatchRuntime};

pub(super) const SONIQO_PARAKEET_MAX_CHUNK_SAMPLES: usize = TARGET_SAMPLE_RATE as usize * 59 / 2;
pub(super) const SONIQO_PROGRESS_PLANNED: f64 = 0.05;
const SONIQO_PROGRESS_RANGE: f64 = 0.90;
pub(super) const SONIQO_PROGRESS_MAX: f64 = 0.95;
pub(super) const SONIQO_DIRECT_MIC_MIN_RMS: f64 = 0.0008;

pub(in crate::batch) async fn run_apple_speech_batch(
    runtime: Arc<dyn BatchRuntime>,
    params: BatchParams,
    listen_params: owhisper_interface::ListenParams,
) -> crate::Result<BatchRunOutput> {
    let span = session_span(&params.session_id);

    async {
        let locale = anlg_transcribe_speechanalyzer::resolve_session_locale(
            &listen_params.languages,
        )
        .ok_or_else(|| crate::BatchFailure::DirectRequestFailed {
            provider: "apple-speech".to_string(),
            message:
                "Add this language in System Settings > General > Language & Region to transcribe it with Apple Speech."
                    .to_string(),
        })?;
        let file_path = params.file_path.clone();
        let started_at = Instant::now();

        tracing::info!(
            anarlog.stt.provider.name = "apple-speech",
            anarlog.stt.language = %locale,
            "apple_speech_batch_start"
        );

        let session_id = params.session_id.clone();
        let progress_runtime = runtime.clone();
        let transcribed = tokio::task::spawn_blocking(move || {
            let progress = SoniqoProgressReporter {
                runtime: progress_runtime,
                session_id,
            };
            transcribe_apple_speech_file(&file_path, &locale, Some(&progress))
        })
        .await
        .map_err(|e| crate::BatchFailure::DirectRequestFailed {
            provider: "apple-speech".to_string(),
            message: format!("Apple Speech transcription task failed: {e}"),
        })?
        .map_err(|e| {
            let message = format_user_friendly_error(&e);
            tracing::error!(
                anarlog.stt.provider.name = "apple-speech",
                error = %e,
                anarlog.error.user_message = %message,
                "apple_speech_batch_failed"
            );
            crate::BatchFailure::DirectRequestFailed {
                provider: "apple-speech".to_string(),
                message,
            }
        })?;

        tracing::info!(
            anarlog.stt.provider.name = "apple-speech",
            elapsed_ms = started_at.elapsed().as_millis() as u64,
            transcript.channel_count = transcribed.len(),
            "apple_speech_batch_completed"
        );

        Ok(BatchRunOutput {
            session_id: params.session_id,
            mode: BatchRunMode::Direct,
            response: anlg_transcribe_speechanalyzer::batch_response_from_transcripts(transcribed),
        })
    }
    .instrument(span)
    .await
}

/// Transcribes each recorded channel separately so mic and system audio stay attributable.
fn transcribe_apple_speech_file(
    file_path: &str,
    locale: &str,
    progress: Option<&SoniqoProgressReporter>,
) -> std::result::Result<Vec<anlg_transcribe_speechanalyzer::FileTranscript>, String> {
    let source = anlg_audio_utils::source_from_path(file_path).map_err(|e| e.to_string())?;
    let channel_count = u16::from(source.channels()).max(1) as usize;
    let samples =
        anlg_audio_utils::resample_audio(source, TARGET_SAMPLE_RATE).map_err(|e| e.to_string())?;
    let channels = collapse_identical_channels(split_resampled_channels(&samples, channel_count));

    if let Some(progress) = progress {
        progress.emit(SONIQO_PROGRESS_PLANNED);
    }

    let total = channels.len().max(1);
    let mut transcripts = Vec::with_capacity(channels.len());

    for (index, channel) in channels.into_iter().enumerate() {
        let transcript = anlg_transcribe_speechanalyzer::transcribe_samples(&channel, locale)
            .map_err(|e| e.to_string())?;
        transcripts.push(transcript);

        if let Some(progress) = progress {
            progress.emit(soniqo_batch_progress(index + 1, total));
        }
    }

    Ok(transcripts)
}

pub(in crate::batch) async fn run_soniqo_batch(
    runtime: Arc<dyn BatchRuntime>,
    params: BatchParams,
    listen_params: owhisper_interface::ListenParams,
) -> crate::Result<BatchRunOutput> {
    let span = session_span(&params.session_id);

    async {
        let model = listen_params
            .model
            .as_deref()
            .ok_or_else(|| crate::BatchFailure::DirectRequestFailed {
                provider: "soniqo".to_string(),
                message: "Missing Soniqo model.".to_string(),
            })?
            .parse::<anlg_transcribe_soniqo::SoniqoModel>()
            .map_err(|e| crate::BatchFailure::DirectRequestFailed {
                provider: "soniqo".to_string(),
                message: e.to_string(),
            })?
            .batch_model();

        let file_path = params.file_path.clone();
        let file_extension = Path::new(&file_path)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_string();
        let language = listen_params
            .languages
            .first()
            .map(anlg_language::Language::bcp47_code);
        let language_hint = soniqo_language_hint(language.as_deref());
        let num_speakers = listen_params.num_speakers;
        let language_label = language.as_deref().unwrap_or("auto").to_string();
        let language_hint_label = language_hint.as_deref().unwrap_or("auto").to_string();
        let started_at = Instant::now();

        tracing::info!(
            anarlog.stt.provider.name = "soniqo",
            anarlog.stt.model = %model,
            anarlog.stt.language = %language_label,
            anarlog.stt.language_hint = %language_hint_label,
            file.extension = %file_extension,
            "soniqo_batch_start"
        );

        let session_id = params.session_id.clone();
        let transcribed = tokio::task::spawn_blocking(move || {
            let progress = SoniqoProgressReporter {
                runtime,
                session_id,
            };
            transcribe_soniqo_file(
                model,
                &file_path,
                language_hint.as_deref(),
                num_speakers,
                Some(&progress),
            )
        })
        .await
        .map_err(|e| {
            tracing::error!(
                anarlog.stt.provider.name = "soniqo",
                anarlog.stt.model = %model,
                error = %e,
                "soniqo_batch_task_join_failed"
            );
            crate::BatchFailure::DirectRequestFailed {
                provider: "soniqo".to_string(),
                message: format!("Soniqo transcription task failed: {e}"),
            }
        })?
        .map_err(|e| {
            let message = format_user_friendly_error(&e);
            tracing::error!(
                anarlog.stt.provider.name = "soniqo",
                anarlog.stt.model = %model,
                error = %e,
                anarlog.error.user_message = %message,
                "soniqo_batch_failed"
            );
            crate::BatchFailure::DirectRequestFailed {
                provider: "soniqo".to_string(),
                message,
            }
        })?;

        tracing::info!(
            anarlog.stt.provider.name = "soniqo",
            anarlog.stt.model = %model,
            elapsed_ms = started_at.elapsed().as_millis() as u64,
            transcript.channel_count = transcribed.len(),
            "soniqo_batch_completed"
        );

        let response = anlg_transcribe_soniqo::batch_response_from_channels(model, transcribed);

        Ok(BatchRunOutput {
            session_id: params.session_id,
            mode: BatchRunMode::Direct,
            response,
        })
    }
    .instrument(span)
    .await
}

fn transcribe_soniqo_file(
    model: anlg_transcribe_soniqo::SoniqoModel,
    file_path: &str,
    language: Option<&str>,
    num_speakers: Option<u32>,
    progress: Option<&SoniqoProgressReporter>,
) -> std::result::Result<Vec<anlg_transcribe_soniqo::FileTranscript>, String> {
    let source = anlg_audio_utils::source_from_path(file_path).map_err(|e| e.to_string())?;
    let channel_count = u16::from(source.channels()).max(1) as usize;
    let sample_rate = u32::from(source.sample_rate());
    let duration_ms = source
        .total_duration()
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64);

    tracing::info!(
        anarlog.stt.provider.name = "soniqo",
        anarlog.stt.model = %model,
        anarlog.stt.language = %language.unwrap_or("auto"),
        audio.channel_count = channel_count,
        audio.sample_rate_hz = sample_rate,
        audio.duration_ms = duration_ms.unwrap_or_default(),
        audio.duration_known = duration_ms.is_some(),
        "soniqo_audio_file_loaded"
    );

    if channel_count <= 1 && !uses_resilient_soniqo_chunking(model) {
        if let Some(progress) = progress {
            progress.emit(SONIQO_PROGRESS_PLANNED);
        }
        tracing::info!(
            anarlog.stt.provider.name = "soniqo",
            anarlog.stt.model = %model,
            "soniqo_single_channel_native_inference_start"
        );
        return anlg_transcribe_soniqo::transcribe_file(model, file_path, language)
            .map(|transcript| {
                if let Some(progress) = progress {
                    progress.emit(SONIQO_PROGRESS_MAX);
                }
                vec![transcript]
            })
            .map_err(|e| e.to_string());
    }

    let resample_started_at = Instant::now();
    let samples =
        anlg_audio_utils::resample_audio(source, TARGET_SAMPLE_RATE).map_err(|e| e.to_string())?;
    tracing::info!(
        anarlog.stt.provider.name = "soniqo",
        anarlog.stt.model = %model,
        elapsed_ms = resample_started_at.elapsed().as_millis() as u64,
        audio.source_sample_rate_hz = sample_rate,
        audio.target_sample_rate_hz = TARGET_SAMPLE_RATE,
        audio.resampled_sample_count = samples.len(),
        "soniqo_audio_resampled"
    );

    let channel_samples =
        collapse_identical_channels(split_resampled_channels(&samples, channel_count));
    tracing::info!(
        anarlog.stt.provider.name = "soniqo",
        anarlog.stt.model = %model,
        audio.source_channel_count = channel_count,
        audio.transcribed_channel_count = channel_samples.len(),
        "soniqo_channels_prepared"
    );

    let transcribed_channel_count = channel_samples.len();
    let plans = channel_samples
        .iter()
        .enumerate()
        .map(|(channel_index, samples)| {
            soniqo_channel_plan(
                model,
                channel_index,
                samples,
                transcribed_channel_count == 2 && channel_index == 0,
            )
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let total_chunks = plans.iter().map(|plan| plan.chunks.len()).sum::<usize>();
    let mut completed_chunks = 0usize;

    if let Some(progress) = progress {
        progress.emit(soniqo_batch_progress(0, total_chunks));
    }

    let mut transcripts = collect_soniqo_channel_transcripts(plans.into_iter().map(|plan| {
        transcribe_soniqo_channel_chunks(model, plan, language, || {
            completed_chunks += 1;
            if let Some(progress) = progress {
                progress.emit(soniqo_batch_progress(completed_chunks, total_chunks));
            }
        })
    }))?;

    for (channel_index, (transcript, samples)) in transcripts
        .iter_mut()
        .zip(channel_samples.iter())
        .enumerate()
    {
        let Some(speaker_count) =
            soniqo_diarization_speaker_count(num_speakers, channel_samples.len(), channel_index)
        else {
            continue;
        };

        let started_at = Instant::now();
        match anlg_transcribe_soniqo::diarize_samples(model, samples, speaker_count) {
            Ok(segments) => {
                tracing::info!(
                    anarlog.stt.provider.name = "soniqo",
                    anarlog.stt.model = %model,
                    channel.index = channel_index,
                    speaker.count = speaker_count,
                    segment.count = segments.len(),
                    elapsed_ms = started_at.elapsed().as_millis() as u64,
                    "soniqo_channel_diarization_completed"
                );
                transcript.speaker_segments = segments;
            }
            Err(error) => {
                tracing::warn!(
                    anarlog.stt.provider.name = "soniqo",
                    anarlog.stt.model = %model,
                    channel.index = channel_index,
                    speaker.count = speaker_count,
                    elapsed_ms = started_at.elapsed().as_millis() as u64,
                    error = %error,
                    "soniqo_channel_diarization_failed"
                );
            }
        }
    }

    Ok(transcripts)
}

struct SoniqoProgressReporter {
    runtime: Arc<dyn BatchRuntime>,
    session_id: String,
}

impl SoniqoProgressReporter {
    fn emit(&self, percentage: f64) {
        self.runtime.emit(BatchEvent::BatchResponseStreamed {
            session_id: self.session_id.clone(),
            event: BatchStreamEvent::Progress {
                percentage,
                partial_text: None,
            },
        });
    }
}

struct SoniqoChannelPlan {
    channel_index: usize,
    duration_seconds: f64,
    is_direct_mic: bool,
    chunks: Vec<AudioChunk>,
}

pub(super) fn soniqo_language_hint(language: Option<&str>) -> Option<String> {
    let language = language?.trim();
    if language.is_empty() {
        return None;
    }

    language
        .split(['-', '_'])
        .next()
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase())
}

pub(super) fn uses_resilient_soniqo_chunking(model: anlg_transcribe_soniqo::SoniqoModel) -> bool {
    matches!(model, anlg_transcribe_soniqo::SoniqoModel::ParakeetBatch)
}

pub(super) fn soniqo_batch_progress(completed_chunks: usize, total_chunks: usize) -> f64 {
    if total_chunks == 0 {
        return SONIQO_PROGRESS_PLANNED;
    }

    let ratio = completed_chunks as f64 / total_chunks as f64;
    (SONIQO_PROGRESS_PLANNED + ratio * SONIQO_PROGRESS_RANGE).min(SONIQO_PROGRESS_MAX)
}

pub(super) fn soniqo_diarization_speaker_count(
    num_speakers: Option<u32>,
    channel_count: usize,
    channel_index: usize,
) -> Option<usize> {
    let total = usize::try_from(num_speakers?).ok()?;

    let count = match channel_count {
        1 => total,
        2 if channel_index == 1 => total.saturating_sub(1),
        _ => return None,
    };

    (count >= 2).then_some(count)
}

pub(super) fn collect_soniqo_channel_transcripts<I>(
    transcripts: I,
) -> std::result::Result<Vec<anlg_transcribe_soniqo::FileTranscript>, String>
where
    I: IntoIterator<Item = std::result::Result<anlg_transcribe_soniqo::FileTranscript, String>>,
{
    let mut output = Vec::new();
    let mut successful_channels = 0usize;
    let mut failed_channels = 0usize;

    for transcript in transcripts {
        match transcript {
            Ok(transcript) => {
                successful_channels += 1;
                output.push(transcript);
            }
            Err(error) => {
                failed_channels += 1;
                tracing::warn!(
                    anarlog.stt.provider.name = "soniqo",
                    error = %error,
                    "soniqo_channel_transcription_failed"
                );
                output.push(anlg_transcribe_soniqo::FileTranscript::new(
                    String::new(),
                    0.05,
                ));
            }
        }
    }

    if successful_channels == 0 && failed_channels > 0 {
        return Err(format!(
            "Soniqo failed to transcribe all {failed_channels} audio channel(s)."
        ));
    }

    Ok(output)
}

fn soniqo_channel_plan(
    model: anlg_transcribe_soniqo::SoniqoModel,
    channel_index: usize,
    samples: &[f32],
    is_direct_mic: bool,
) -> std::result::Result<SoniqoChannelPlan, String> {
    let duration_seconds = channel_duration_sec(samples);
    let chunks = soniqo_channel_chunks(model, samples)?;
    tracing::info!(
        anarlog.stt.provider.name = "soniqo",
        anarlog.stt.model = %model,
        channel.index = channel_index,
        channel.duration_seconds = duration_seconds,
        channel.sample_count = samples.len(),
        chunk.count = chunks.len(),
        "soniqo_channel_chunked"
    );

    Ok(SoniqoChannelPlan {
        channel_index,
        duration_seconds,
        is_direct_mic,
        chunks,
    })
}

fn transcribe_soniqo_channel_chunks(
    model: anlg_transcribe_soniqo::SoniqoModel,
    plan: SoniqoChannelPlan,
    language: Option<&str>,
    mut on_chunk_completed: impl FnMut(),
) -> std::result::Result<anlg_transcribe_soniqo::FileTranscript, String> {
    let mut texts = Vec::new();
    let mut transcript_chunks = Vec::new();
    let mut successful_chunks = 0usize;
    let mut failed_chunks = 0usize;
    let channel_index = plan.channel_index;
    let is_direct_mic = plan.is_direct_mic;

    for (chunk_index, chunk) in plan.chunks.into_iter().enumerate() {
        let chunk_duration_ms =
            (chunk.sample_end - chunk.sample_start) * 1000 / TARGET_SAMPLE_RATE as usize;
        let chunk_rms = audio_rms(&chunk.samples);
        if is_direct_mic && chunk_rms < SONIQO_DIRECT_MIC_MIN_RMS {
            on_chunk_completed();
            tracing::info!(
                anarlog.stt.provider.name = "soniqo",
                anarlog.stt.model = %model,
                channel.index = channel_index,
                chunk.index = chunk_index,
                audio.rms = chunk_rms,
                audio.minimum_rms = SONIQO_DIRECT_MIC_MIN_RMS,
                "soniqo_direct_mic_chunk_skipped"
            );
            continue;
        }

        let chunk_started_at = Instant::now();
        tracing::info!(
            anarlog.stt.provider.name = "soniqo",
            anarlog.stt.model = %model,
            channel.index = channel_index,
            chunk.index = chunk_index,
            chunk.sample_start = chunk.sample_start,
            chunk.sample_end = chunk.sample_end,
            chunk.sample_count = chunk.samples.len(),
            chunk.duration_ms = chunk_duration_ms,
            "soniqo_chunk_native_inference_start"
        );

        let text = match transcribe_soniqo_samples(model, &chunk.samples, language) {
            Ok(transcript) => {
                successful_chunks += 1;
                transcript.text
            }
            Err(e) => {
                failed_chunks += 1;
                tracing::warn!(
                    anarlog.stt.provider.name = "soniqo",
                    anarlog.stt.model = %model,
                    channel.index = channel_index,
                    chunk.index = chunk_index,
                    elapsed_ms = chunk_started_at.elapsed().as_millis() as u64,
                    error = %e,
                    "soniqo_chunk_native_inference_failed"
                );
                on_chunk_completed();
                continue;
            }
        };
        on_chunk_completed();

        tracing::info!(
            anarlog.stt.provider.name = "soniqo",
            anarlog.stt.model = %model,
            channel.index = channel_index,
            chunk.index = chunk_index,
            elapsed_ms = chunk_started_at.elapsed().as_millis() as u64,
            transcript.text_chars = text.chars().count(),
            "soniqo_chunk_native_inference_completed"
        );

        let text = text.trim();
        if !text.is_empty() {
            texts.push(text.to_string());
            transcript_chunks.push(anlg_transcribe_soniqo::FileTranscriptChunk {
                text: text.to_string(),
                start_seconds: chunk.sample_start as f64 / TARGET_SAMPLE_RATE as f64,
                duration_seconds: (chunk.sample_end - chunk.sample_start) as f64
                    / TARGET_SAMPLE_RATE as f64,
            });
        }
    }

    if successful_chunks == 0 && failed_chunks > 0 {
        return Err(format!(
            "Soniqo failed to transcribe all {failed_chunks} chunk(s) for channel {channel_index}."
        ));
    }

    if failed_chunks > 0 {
        tracing::warn!(
            anarlog.stt.provider.name = "soniqo",
            anarlog.stt.model = %model,
            channel.index = channel_index,
            chunk.success_count = successful_chunks,
            chunk.failed_count = failed_chunks,
            "soniqo_channel_completed_with_chunk_failures"
        );
    }

    if transcript_chunks.is_empty() {
        return Ok(anlg_transcribe_soniqo::FileTranscript::new(
            texts.join(" "),
            plan.duration_seconds,
        ));
    }

    Ok(anlg_transcribe_soniqo::FileTranscript::from_chunks(
        transcript_chunks,
        plan.duration_seconds,
    ))
}

pub(super) fn audio_rms(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }

    let sum_of_squares = samples
        .iter()
        .map(|sample| f64::from(*sample).powi(2))
        .sum::<f64>();
    (sum_of_squares / samples.len() as f64).sqrt()
}

fn transcribe_soniqo_samples(
    model: anlg_transcribe_soniqo::SoniqoModel,
    samples: &[f32],
    language: Option<&str>,
) -> std::result::Result<anlg_transcribe_soniqo::FileTranscript, String> {
    let file = tempfile::Builder::new()
        .prefix("soniqo_channel_")
        .suffix(".wav")
        .tempfile()
        .map_err(|e| e.to_string())?;
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    {
        let mut writer = hound::WavWriter::create(file.path(), spec).map_err(|e| e.to_string())?;
        for sample in samples {
            writer.write_sample(*sample).map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())?;
    }

    anlg_transcribe_soniqo::transcribe_file(model, file.path(), language).map_err(|e| e.to_string())
}

pub(super) fn soniqo_channel_chunks(
    model: anlg_transcribe_soniqo::SoniqoModel,
    samples: &[f32],
) -> std::result::Result<Vec<AudioChunk>, String> {
    if model == anlg_transcribe_soniqo::SoniqoModel::ParakeetBatch {
        return Ok(split_audio_samples(
            samples,
            SONIQO_PARAKEET_MAX_CHUNK_SAMPLES,
        ));
    }

    chunk_channel_audio::<anlg_audio_chunking::Error>(samples).map_err(|e| e.to_string())
}

fn split_audio_samples(samples: &[f32], max_samples: usize) -> Vec<AudioChunk> {
    samples
        .chunks(max_samples)
        .enumerate()
        .map(|(index, window)| {
            let sample_start = index * max_samples;
            let sample_end = sample_start + window.len();
            AudioChunk {
                samples: window.to_vec(),
                sample_start,
                sample_end,
            }
        })
        .collect()
}

pub(super) fn collapse_identical_channels(channels: Vec<Vec<f32>>) -> Vec<Vec<f32>> {
    if channels.len() != 2 || !channels_are_effectively_identical(&channels[0], &channels[1]) {
        return channels;
    }

    channels.into_iter().take(1).collect()
}

fn channels_are_effectively_identical(left: &[f32], right: &[f32]) -> bool {
    if left.len().abs_diff(right.len()) > 1 {
        return false;
    }

    let compared = left.len().min(right.len());
    if compared == 0 {
        return true;
    }

    let mean_abs_diff = left
        .iter()
        .zip(right.iter())
        .map(|(a, b)| (a - b).abs())
        .sum::<f32>()
        / compared as f32;

    mean_abs_diff < 0.0005
}
