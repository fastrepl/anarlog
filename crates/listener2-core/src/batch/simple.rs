use std::path::Path;
use std::time::Instant;

use owhisper_client::{
    AdapterKind, AquaVoiceAdapter, ArgmaxAdapter, AssemblyAIAdapter, BatchSttAdapter,
    DeepgramAdapter, ElevenLabsAdapter, FireworksAdapter, GladiaAdapter, HyprnoteAdapter,
    MistralAdapter, OpenAIAdapter, PyannoteAdapter, SonioxAdapter,
};
use tracing::Instrument;

use hypr_audio_utils::Source;
use hypr_transcribe_core::{
    TARGET_SAMPLE_RATE, channel_duration_sec, chunk_channel_audio, split_resampled_channels,
};

use super::{BatchParams, BatchRunMode, BatchRunOutput, format_user_friendly_error, session_span};

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
    }, unsupported: [DashScope, Cactus])
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
        let language_label = language.as_deref().unwrap_or("auto").to_string();
        let started_at = Instant::now();

        tracing::info!(
            hyprnote.stt.provider.name = "soniqo",
            hyprnote.stt.model = %model,
            hyprnote.stt.language = %language_label,
            file.extension = %file_extension,
            "soniqo_batch_start"
        );

        let transcribed = tokio::task::spawn_blocking(move || {
            transcribe_soniqo_file(model, &file_path, language.as_deref())
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

    if channel_count <= 1 {
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

    channel_samples
        .into_iter()
        .enumerate()
        .map(|(channel_index, samples)| {
            transcribe_soniqo_channel(model, channel_index, &samples, language)
        })
        .collect()
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

        let text = transcribe_soniqo_samples(model, &chunk.samples, language)
            .map_err(|e| {
                tracing::error!(
                    hyprnote.stt.provider.name = "soniqo",
                    hyprnote.stt.model = %model,
                    channel.index = channel_index,
                    chunk.index = chunk_index,
                    elapsed_ms = chunk_started_at.elapsed().as_millis() as u64,
                    error = %e,
                    "soniqo_chunk_native_inference_failed"
                );
                e
            })?
            .text;

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
}
