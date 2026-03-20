use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use hypr_audio_utils::{Source, f32_to_i16_bytes, resample_audio, source_from_path};
use owhisper_interface::batch::Response as BatchResponse;
use owhisper_interface::stream::StreamResponse;
use owhisper_interface::{ControlMessage, ListenParams, MixedMessage};
use tokio_stream::StreamExt as TokioStreamExt;

use crate::ListenClientBuilder;
use crate::adapter::deepgram_compat::build_batch_url;
use crate::adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware};
use crate::error::Error;

use super::{ArgmaxAdapter, keywords::ArgmaxKeywordStrategy, language::ArgmaxLanguageStrategy};

impl BatchSttAdapter for ArgmaxAdapter {
    fn provider_name(&self) -> &'static str {
        "argmax"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        ArgmaxAdapter::is_supported_languages_batch(languages, model)
    }

    fn transcribe_file<'a, P: AsRef<Path> + Send + 'a>(
        &'a self,
        client: &'a ClientWithMiddleware,
        api_base: &'a str,
        api_key: &'a str,
        params: &'a ListenParams,
        file_path: P,
    ) -> BatchFuture<'a> {
        let path = file_path.as_ref().to_path_buf();
        Box::pin(do_transcribe_file(client, api_base, api_key, params, path))
    }
}

async fn do_transcribe_file(
    client: &ClientWithMiddleware,
    api_base: &str,
    api_key: &str,
    params: &ListenParams,
    file_path: PathBuf,
) -> Result<BatchResponse, Error> {
    let (audio_data, sample_rate) = decode_audio_to_linear16(file_path).await?;

    let url = {
        let mut url = build_batch_url(
            api_base,
            params,
            &ArgmaxLanguageStrategy,
            &ArgmaxKeywordStrategy,
        );
        url.query_pairs_mut()
            .append_pair("sample_rate", &sample_rate.to_string());
        url
    };

    let content_type = format!("audio/raw;encoding=linear16;rate={}", sample_rate);

    let response = client
        .post(url)
        .header("Authorization", format!("Token {}", api_key))
        .header("Accept", "application/json")
        .header("Content-Type", content_type)
        .body(audio_data)
        .send()
        .await?;

    let status = response.status();
    if status.is_success() {
        Ok(response.json().await?)
    } else {
        Err(Error::UnexpectedStatus {
            status,
            body: response.text().await.unwrap_or_default(),
        })
    }
}

async fn decode_audio_to_linear16(path: PathBuf) -> Result<(bytes::Bytes, u32), Error> {
    tokio::task::spawn_blocking(move || -> Result<(bytes::Bytes, u32), Error> {
        let decoder =
            source_from_path(&path).map_err(|err| Error::AudioProcessing(err.to_string()))?;

        let channels: u16 = decoder.channels().into();
        let sample_rate: u32 = decoder.sample_rate().into();

        let samples = resample_audio(decoder, sample_rate)
            .map_err(|err| Error::AudioProcessing(err.to_string()))?;

        let samples = if channels == 1 {
            samples
        } else {
            let channels_usize = channels as usize;
            let mut mono = Vec::with_capacity(samples.len() / channels_usize);
            for frame in samples.chunks(channels_usize) {
                if frame.is_empty() {
                    continue;
                }
                let sum: f32 = frame.iter().copied().sum();
                mono.push(sum / frame.len() as f32);
            }
            mono
        };

        if samples.is_empty() {
            return Err(Error::AudioProcessing(
                "audio file contains no samples".to_string(),
            ));
        }

        let bytes = f32_to_i16_bytes(samples.into_iter());

        Ok((bytes, sample_rate))
    })
    .await?
}

const DEFAULT_CHUNK_MS: u64 = 500;
const DEFAULT_DELAY_MS: u64 = 20;

#[derive(Clone, Copy)]
pub struct StreamingBatchConfig {
    pub chunk_ms: u64,
    pub delay_ms: u64,
}

impl Default for StreamingBatchConfig {
    fn default() -> Self {
        Self {
            chunk_ms: DEFAULT_CHUNK_MS,
            delay_ms: DEFAULT_DELAY_MS,
        }
    }
}

impl StreamingBatchConfig {
    pub fn new(chunk_ms: u64, delay_ms: u64) -> Self {
        Self {
            chunk_ms: chunk_ms.max(1),
            delay_ms,
        }
    }

    fn chunk_interval(&self) -> Duration {
        Duration::from_millis(self.delay_ms)
    }
}

pub use crate::adapter::{StreamingBatchEvent, StreamingBatchStream};

impl ArgmaxAdapter {
    pub async fn transcribe_file_streaming<P: AsRef<Path>>(
        api_base: &str,
        api_key: &str,
        params: &ListenParams,
        file_path: P,
        config: Option<StreamingBatchConfig>,
    ) -> Result<StreamingBatchStream, Error> {
        let config = config.unwrap_or_default();
        let path = file_path.as_ref().to_path_buf();

        let chunked_audio = tokio::task::spawn_blocking({
            let chunk_ms = config.chunk_ms;
            move || hypr_audio_utils::chunk_audio_file(path, chunk_ms)
        })
        .await
        .map_err(|e| Error::AudioProcessing(format!("chunk task panicked: {:?}", e)))?
        .map_err(|e| Error::AudioProcessing(format!("{:?}", e)))?;

        let frame_count = chunked_audio.frame_count;
        let metadata = chunked_audio.metadata;
        let audio_duration_secs = if frame_count == 0 || metadata.sample_rate == 0 {
            0.0
        } else {
            frame_count as f64 / metadata.sample_rate as f64
        };

        let channel_count = metadata.channels.clamp(1, 2);
        let listen_params = ListenParams {
            channels: channel_count,
            sample_rate: metadata.sample_rate,
            ..params.clone()
        };

        let client = ListenClientBuilder::default()
            .adapter::<ArgmaxAdapter>()
            .api_base(api_base)
            .api_key(api_key)
            .params(listen_params)
            .build_with_channels(channel_count)
            .await;

        let audio_stream =
            tokio_stream::iter(chunked_audio.chunks.into_iter().map(MixedMessage::Audio));
        let finalize_stream =
            tokio_stream::iter(vec![MixedMessage::Control(ControlMessage::Finalize)]);
        let outbound = TokioStreamExt::throttle(
            TokioStreamExt::chain(audio_stream, finalize_stream),
            config.chunk_interval(),
        );

        let (listen_stream, _handle) = client
            .from_realtime_audio(Box::pin(outbound))
            .await
            .map_err(|e| Error::WebSocket(format!("{:?}", e)))?;

        let mapped_stream = StreamExt::map(listen_stream, move |result| {
            result
                .map(|response| {
                    let percentage = compute_percentage(&response, audio_duration_secs);
                    StreamingBatchEvent {
                        response,
                        percentage,
                    }
                })
                .map_err(|e| Error::WebSocket(format!("{:?}", e)))
        });

        Ok(Box::pin(mapped_stream))
    }
}

fn compute_percentage(response: &StreamResponse, audio_duration_secs: f64) -> f64 {
    let transcript_end = transcript_end_from_response(response);
    match transcript_end {
        Some(end) if audio_duration_secs > 0.0 => (end / audio_duration_secs).clamp(0.0, 1.0),
        _ => 0.0,
    }
}

fn transcript_end_from_response(response: &StreamResponse) -> Option<f64> {
    let StreamResponse::TranscriptResponse {
        start,
        duration,
        channel,
        ..
    } = response
    else {
        return None;
    };

    let mut end = (*start + *duration).max(0.0);

    for alternative in &channel.alternatives {
        for word in &alternative.words {
            if word.end.is_finite() {
                end = end.max(word.end);
            }
        }
    }

    if end.is_finite() { Some(end) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_client::create_client;

    #[tokio::test]
    #[ignore]
    async fn test_argmax_batch_transcription() {
        let client = create_client();
        let adapter = ArgmaxAdapter::default();
        let params = ListenParams::default();

        let audio_path = std::path::PathBuf::from(hypr_data::english_1::AUDIO_PATH);

        let result = adapter
            .transcribe_file(
                &client,
                "http://localhost:50060/v1",
                "",
                &params,
                &audio_path,
            )
            .await
            .expect("transcription failed");

        assert!(!result.results.channels.is_empty());
        assert!(!result.results.channels[0].alternatives.is_empty());
        assert!(
            !result.results.channels[0].alternatives[0]
                .transcript
                .is_empty()
        );
    }
}
