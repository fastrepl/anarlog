use std::path::Path;
use std::time::Instant;

use owhisper_client::{
    AdapterKind, AquaVoiceAdapter, ArgmaxAdapter, AssemblyAIAdapter, BatchSttAdapter,
    CartesiaAdapter, DeepgramAdapter, ElevenLabsAdapter, FireworksAdapter, GladiaAdapter,
    HyprnoteAdapter, MistralAdapter, OpenAIAdapter, PyannoteAdapter, SonioxAdapter,
};
use tracing::Instrument;

use hypr_audio_chunking::AudioChunk;
use hypr_audio_utils::Source;
use hypr_transcribe_core::{
    TARGET_SAMPLE_RATE, channel_duration_sec, chunk_channel_audio, split_resampled_channels,
};

use super::{BatchParams, BatchRunMode, BatchRunOutput, format_user_friendly_error, session_span};

const SONIQO_PARAKEET_MAX_CHUNK_SAMPLES: usize = TARGET_SAMPLE_RATE as usize * 39 / 2;

macro_rules! dispatch_batch {
    ($ak:expr, $params:expr, $lp:expr,
     { $($var:ident => $adapter:ty),+ $(,)? },
     unsupported: [$($unsup:ident),* $(,)?]
    ) => {
        match $ak {
            $(AdapterKind::$var => {
                run_direct_batch::<$adapter>(&AdapterKind::$var.to_string(), $params, $lp).await
            })+
            $(AdapterKind::$unsup => {
                Err(crate::BatchFailure::DirectBatchUnsupported {
                    provider: AdapterKind::$unsup.to_string(),
                }.into())
            })*
        }
    };
}

pub(super) async fn run_direct_batch_for_adapter_kind(
    adapter_kind: AdapterKind,
    params: BatchParams,
    listen_params: owhisper_interface::ListenParams,
) -> crate::Result<BatchRunOutput> {
    dispatch_batch!(adapter_kind, params, listen_params, {
        Argmax => ArgmaxAdapter,
        Cartesia => CartesiaAdapter,
        Deepgram => DeepgramAdapter,
        Soniox => SonioxAdapter,
        AssemblyAI => AssemblyAIAdapter,
        Fireworks => FireworksAdapter,
        OpenAI => OpenAIAdapter,
        Gladia => GladiaAdapter,
        ElevenLabs => ElevenLabsAdapter,
        Pyannote => PyannoteAdapter,
        Mistral => MistralAdapter,
        Hyprnote => HyprnoteAdapter,
        AquaVoice => AquaVoiceAdapter,
    }, unsupported: [DashScope])
}

async fn run_direct_batch<A: BatchSttAdapter>(
    provider: &str,
    params: BatchParams,
    listen_params: owhisper_interface::ListenParams,
) -> crate::Result<BatchRunOutput> {
    let span = session_span(&params.session_id);

    async {
        let client = owhisper_client::BatchClient::<A>::builder()
            .api_base(params.base_url.clone())
            .api_key(params.api_key.clone())
            .params(listen_params)
            .build();

        tracing::debug!("transcribing file: {}", params.file_path);
        let response = match client.transcribe_file(&params.file_path).await {
            Ok(response) => response,
            Err(err) => {
                let raw_error = format!("{err:?}");
                let message = format_user_friendly_error(&raw_error);
                tracing::error!(
                    error = %raw_error,
                    hyprnote.error.user_message = %message,
                    "batch transcription failed"
                );
                return Err(crate::BatchFailure::DirectRequestFailed {
                    provider: provider.to_string(),
                    message,
                }
                .into());
            }
        };
        tracing::info!("batch transcription completed");

        Ok(BatchRunOutput {
            session_id: params.session_id,
            mode: BatchRunMode::Direct,
            response,
        })
    }
    .instrument(span)
    .await
}

pub(super) async fn run_soniqo_batch(
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
            .parse::<hypr_transcribe_soniqo::SoniqoModel>()
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
            .map(hypr_language::Language::bcp47_code);
        let language_hint = soniqo_language_hint(language.as_deref());
        let language_label = language.as_deref().unwrap_or("auto").to_string();
        let language_hint_label = language_hint.as_deref().unwrap_or("auto").to_string();
        let started_at = Instant::now();

        tracing::info!(
            hyprnote.stt.provider.name = "soniqo",
            hyprnote.stt.model = %model,
            hyprnote.stt.language = %language_label,
            hyprnote.stt.language_hint = %language_hint_label,
            file.extension = %file_extension,
            "soniqo_batch_start"
        );

        let transcribed = tokio::task::spawn_blocking(move || {
            transcribe_soniqo_file(model, &file_path, language_hint.as_deref())
        })
        .await
        .map_err(|e| {
            tracing::error!(
                hyprnote.stt.provider.name = "soniqo",
                hyprnote.stt.model = %model,
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
                hyprnote.stt.provider.name = "soniqo",
                hyprnote.stt.model = %model,
                error = %e,
                hyprnote.error.user_message = %message,
                "soniqo_batch_failed"
            );
            crate::BatchFailure::DirectRequestFailed {
                provider: "soniqo".to_string(),
                message,
            }
        })?;

        tracing::info!(
            hyprnote.stt.provider.name = "soniqo",
            hyprnote.stt.model = %model,
            elapsed_ms = started_at.elapsed().as_millis() as u64,
            transcript.channel_count = transcribed.len(),
            "soniqo_batch_completed"
        );

        let response = hypr_transcribe_soniqo::batch_response_from_channels(model, transcribed);

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
    model: hypr_transcribe_soniqo::SoniqoModel,
    file_path: &str,
    language: Option<&str>,
) -> std::result::Result<Vec<hypr_transcribe_soniqo::FileTranscript>, String> {
    let source = hypr_audio_utils::source_from_path(file_path).map_err(|e| e.to_string())?;
    let channel_count = u16::from(source.channels()).max(1) as usize;
    let sample_rate = u32::from(source.sample_rate());
    let duration_ms = source
        .total_duration()
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64);

    tracing::info!(
        hyprnote.stt.provider.name = "soniqo",
        hyprnote.stt.model = %model,
        hyprnote.stt.language = %language.unwrap_or("auto"),
        audio.channel_count = channel_count,
        audio.sample_rate_hz = sample_rate,
        audio.duration_ms = duration_ms.unwrap_or_default(),
        audio.duration_known = duration_ms.is_some(),
        "soniqo_audio_file_loaded"
    );

    if channel_count <= 1 && !uses_resilient_soniqo_chunking(model) {
        tracing::info!(
            hyprnote.stt.provider.name = "soniqo",
            hyprnote.stt.model = %model,
            "soniqo_single_channel_native_inference_start"
        );
        return hypr_transcribe_soniqo::transcribe_file(model, file_path, language)
            .map(|transcript| vec![transcript])
            .map_err(|e| e.to_string());
    }

    let resample_started_at = Instant::now();
    let samples =
        hypr_audio_utils::resample_audio(source, TARGET_SAMPLE_RATE).map_err(|e| e.to_string())?;
    tracing::info!(
        hyprnote.stt.provider.name = "soniqo",
        hyprnote.stt.model = %model,
        elapsed_ms = resample_started_at.elapsed().as_millis() as u64,
        audio.source_sample_rate_hz = sample_rate,
        audio.target_sample_rate_hz = TARGET_SAMPLE_RATE,
        audio.resampled_sample_count = samples.len(),
        "soniqo_audio_resampled"
    );

    let channel_samples =
        collapse_identical_channels(split_resampled_channels(&samples, channel_count));
    tracing::info!(
        hyprnote.stt.provider.name = "soniqo",
        hyprnote.stt.model = %model,
        audio.source_channel_count = channel_count,
        audio.transcribed_channel_count = channel_samples.len(),
        "soniqo_channels_prepared"
    );

    collect_soniqo_channel_transcripts(channel_samples.into_iter().enumerate().map(
        |(channel_index, samples)| {
            transcribe_soniqo_channel(model, channel_index, &samples, language)
        },
    ))
}

fn soniqo_language_hint(language: Option<&str>) -> Option<String> {
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

fn uses_resilient_soniqo_chunking(model: hypr_transcribe_soniqo::SoniqoModel) -> bool {
    matches!(model, hypr_transcribe_soniqo::SoniqoModel::ParakeetBatch)
}

fn collect_soniqo_channel_transcripts<I>(
    transcripts: I,
) -> std::result::Result<Vec<hypr_transcribe_soniqo::FileTranscript>, String>
where
    I: IntoIterator<Item = std::result::Result<hypr_transcribe_soniqo::FileTranscript, String>>,
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
                    hyprnote.stt.provider.name = "soniqo",
                    error = %error,
                    "soniqo_channel_transcription_failed"
                );
                output.push(hypr_transcribe_soniqo::FileTranscript {
                    text: String::new(),
                    duration_seconds: 0.05,
                });
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

fn transcribe_soniqo_channel(
    model: hypr_transcribe_soniqo::SoniqoModel,
    channel_index: usize,
    samples: &[f32],
    language: Option<&str>,
) -> std::result::Result<hypr_transcribe_soniqo::FileTranscript, String> {
    let duration_seconds = channel_duration_sec(samples);
    let chunks =
        chunk_channel_audio::<hypr_audio_chunking::Error>(samples).map_err(|e| e.to_string())?;
    let chunks = split_soniqo_native_chunks(model, chunks);
    tracing::info!(
        hyprnote.stt.provider.name = "soniqo",
        hyprnote.stt.model = %model,
        channel.index = channel_index,
        channel.duration_seconds = duration_seconds,
        channel.sample_count = samples.len(),
        chunk.count = chunks.len(),
        "soniqo_channel_chunked"
    );

    let mut texts = Vec::new();
    let mut successful_chunks = 0usize;
    let mut failed_chunks = 0usize;

    for (chunk_index, chunk) in chunks.into_iter().enumerate() {
        let chunk_duration_ms =
            (chunk.sample_end - chunk.sample_start) * 1000 / TARGET_SAMPLE_RATE as usize;
        let chunk_started_at = Instant::now();
        tracing::info!(
            hyprnote.stt.provider.name = "soniqo",
            hyprnote.stt.model = %model,
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
                    hyprnote.stt.provider.name = "soniqo",
                    hyprnote.stt.model = %model,
                    channel.index = channel_index,
                    chunk.index = chunk_index,
                    elapsed_ms = chunk_started_at.elapsed().as_millis() as u64,
                    error = %e,
                    "soniqo_chunk_native_inference_failed"
                );
                continue;
            }
        };

        tracing::info!(
            hyprnote.stt.provider.name = "soniqo",
            hyprnote.stt.model = %model,
            channel.index = channel_index,
            chunk.index = chunk_index,
            elapsed_ms = chunk_started_at.elapsed().as_millis() as u64,
            transcript.text_chars = text.chars().count(),
            "soniqo_chunk_native_inference_completed"
        );

        let text = text.trim();
        if !text.is_empty() {
            texts.push(text.to_string());
        }
    }

    if successful_chunks == 0 && failed_chunks > 0 {
        return Err(format!(
            "Soniqo failed to transcribe all {failed_chunks} chunk(s) for channel {channel_index}."
        ));
    }

    if failed_chunks > 0 {
        tracing::warn!(
            hyprnote.stt.provider.name = "soniqo",
            hyprnote.stt.model = %model,
            channel.index = channel_index,
            chunk.success_count = successful_chunks,
            chunk.failed_count = failed_chunks,
            "soniqo_channel_completed_with_chunk_failures"
        );
    }

    Ok(hypr_transcribe_soniqo::FileTranscript {
        text: texts.join(" "),
        duration_seconds,
    })
}

fn transcribe_soniqo_samples(
    model: hypr_transcribe_soniqo::SoniqoModel,
    samples: &[f32],
    language: Option<&str>,
) -> std::result::Result<hypr_transcribe_soniqo::FileTranscript, String> {
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

    hypr_transcribe_soniqo::transcribe_file(model, file.path(), language).map_err(|e| e.to_string())
}

fn split_soniqo_native_chunks(
    model: hypr_transcribe_soniqo::SoniqoModel,
    chunks: Vec<AudioChunk>,
) -> Vec<AudioChunk> {
    let max_samples = match model {
        hypr_transcribe_soniqo::SoniqoModel::ParakeetBatch => SONIQO_PARAKEET_MAX_CHUNK_SAMPLES,
        _ => return chunks,
    };

    chunks
        .into_iter()
        .flat_map(|chunk| split_audio_chunk(chunk, max_samples))
        .collect()
}

fn split_audio_chunk(chunk: AudioChunk, max_samples: usize) -> Vec<AudioChunk> {
    if chunk.samples.len() <= max_samples {
        return vec![chunk];
    }

    let AudioChunk {
        samples,
        sample_start,
        ..
    } = chunk;

    samples
        .chunks(max_samples)
        .enumerate()
        .map(|(index, window)| {
            let sample_start = sample_start + index * max_samples;
            let sample_end = sample_start + window.len();
            AudioChunk {
                samples: window.to_vec(),
                sample_start,
                sample_end,
            }
        })
        .collect()
}

fn collapse_identical_channels(channels: Vec<Vec<f32>>) -> Vec<Vec<f32>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_effectively_identical_stereo_channels() {
        let channels =
            collapse_identical_channels(vec![vec![0.1, 0.2, 0.3], vec![0.1001, 0.2001, 0.3001]]);

        assert_eq!(channels, vec![vec![0.1, 0.2, 0.3]]);
    }

    #[test]
    fn keeps_distinct_stereo_channels() {
        let channels = collapse_identical_channels(vec![vec![0.1, 0.2], vec![0.9, 0.8]]);

        assert_eq!(channels, vec![vec![0.1, 0.2], vec![0.9, 0.8]]);
    }

    #[test]
    fn splits_parakeet_chunks_below_largest_coreml_shape() {
        let sample_start = TARGET_SAMPLE_RATE as usize * 4;
        let oversized = SONIQO_PARAKEET_MAX_CHUNK_SAMPLES + TARGET_SAMPLE_RATE as usize;
        let chunks = split_soniqo_native_chunks(
            hypr_transcribe_soniqo::SoniqoModel::ParakeetBatch,
            vec![AudioChunk {
                samples: vec![0.0; oversized],
                sample_start,
                sample_end: sample_start + oversized,
            }],
        );

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].sample_start, sample_start);
        assert_eq!(
            chunks[0].sample_end,
            sample_start + SONIQO_PARAKEET_MAX_CHUNK_SAMPLES
        );
        assert_eq!(chunks[1].sample_start, chunks[0].sample_end);
        assert_eq!(chunks[1].samples.len(), TARGET_SAMPLE_RATE as usize);
    }

    #[test]
    fn leaves_non_parakeet_chunks_unchanged() {
        let oversized = SONIQO_PARAKEET_MAX_CHUNK_SAMPLES + TARGET_SAMPLE_RATE as usize;
        let chunks = split_soniqo_native_chunks(
            hypr_transcribe_soniqo::SoniqoModel::Omnilingual,
            vec![AudioChunk {
                samples: vec![0.0; oversized],
                sample_start: 0,
                sample_end: oversized,
            }],
        );

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].samples.len(), oversized);
    }

    #[test]
    fn soniqo_language_hint_uses_base_language_code() {
        assert_eq!(soniqo_language_hint(Some("de-DE")).as_deref(), Some("de"));
        assert_eq!(soniqo_language_hint(Some("en_US")).as_deref(), Some("en"));
        assert_eq!(soniqo_language_hint(Some(" fr ")).as_deref(), Some("fr"));
        assert_eq!(soniqo_language_hint(Some("")).as_deref(), None);
        assert_eq!(soniqo_language_hint(None).as_deref(), None);
    }

    #[test]
    fn parakeet_batch_uses_resilient_chunking() {
        assert!(uses_resilient_soniqo_chunking(
            hypr_transcribe_soniqo::SoniqoModel::ParakeetBatch
        ));
        assert!(!uses_resilient_soniqo_chunking(
            hypr_transcribe_soniqo::SoniqoModel::Omnilingual
        ));
    }

    #[test]
    fn collect_soniqo_channel_transcripts_keeps_channel_slots() {
        let transcripts = collect_soniqo_channel_transcripts([
            Ok(hypr_transcribe_soniqo::FileTranscript {
                text: "hello".to_string(),
                duration_seconds: 1.0,
            }),
            Err("native chunk failed".to_string()),
        ])
        .unwrap();

        assert_eq!(transcripts.len(), 2);
        assert_eq!(transcripts[0].text, "hello");
        assert_eq!(transcripts[1].text, "");
    }

    #[test]
    fn collect_soniqo_channel_transcripts_preserves_later_channel_index() {
        let transcripts = collect_soniqo_channel_transcripts([
            Err("first failed".to_string()),
            Ok(hypr_transcribe_soniqo::FileTranscript {
                text: "system audio".to_string(),
                duration_seconds: 1.0,
            }),
        ])
        .unwrap();

        let response = hypr_transcribe_soniqo::batch_response_from_channels(
            hypr_transcribe_soniqo::SoniqoModel::ParakeetBatch,
            transcripts,
        );
        let alternative = &response.results.channels[1].alternatives[0];

        assert_eq!(response.results.channels.len(), 2);
        assert_eq!(response.results.channels[0].alternatives[0].transcript, "");
        assert_eq!(alternative.transcript, "system audio");
        assert_eq!(alternative.words[0].channel, 1);
    }

    #[test]
    fn collect_soniqo_channel_transcripts_errors_when_all_channels_fail() {
        let error = collect_soniqo_channel_transcripts([
            Err("first failed".to_string()),
            Err("second failed".to_string()),
        ])
        .unwrap_err();

        assert_eq!(error, "Soniqo failed to transcribe all 2 audio channel(s).");
    }
}
