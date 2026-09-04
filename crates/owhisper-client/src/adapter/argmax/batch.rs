use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anlg_audio_utils::{
    Source, f32_to_i16_bytes, for_each_resampled_channel_block, source_from_path,
};
use futures_util::StreamExt;
use owhisper_interface::batch::Response as BatchResponse;
use owhisper_interface::batch_stream::BatchStreamEvent;
use owhisper_interface::stream::StreamResponse;
use owhisper_interface::{ControlMessage, ListenParams, MixedMessage};
use tokio::io::AsyncReadExt;
use tokio_stream::StreamExt as TokioStreamExt;

use crate::adapter::deepgram_compat::build_batch_url;
use crate::adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware};
use crate::error::Error;
use crate::{ListenClientBuilder, ListenClientInput};

use super::{ArgmaxAdapter, keywords::ArgmaxKeywordStrategy, language::ArgmaxLanguageStrategy};

impl BatchSttAdapter for ArgmaxAdapter {
    fn provider_name(&self) -> &'static str {
        "argmax"
    }

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
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
    let prepared = prepare_linear16(file_path, ChannelLayout::Mono).await?;
    let sample_rate = prepared.sample_rate;

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

    let content_length = prepared.byte_len;
    let audio_file = tokio::fs::File::from_std(prepared.file);
    let response = client
        .post(url)
        .header("Authorization", format!("Token {}", api_key))
        .header("Accept", "application/json")
        .header("Content-Type", content_type)
        .header("Content-Length", content_length.to_string())
        .body(audio_file)
        .send()
        .await?;

    let status = response.status();
    if status.is_success() {
        Ok(response.json().await?)
    } else {
        Err(Error::UnexpectedStatus {
            status,
            body: crate::adapter::http::error_body(response).await,
        })
    }
}

#[derive(Clone, Copy)]
enum ChannelLayout {
    Mono,
    Interleaved,
}

struct PreparedLinear16 {
    file: std::fs::File,
    sample_rate: u32,
    channel_count: usize,
    frame_count: usize,
    byte_len: u64,
}

#[derive(Debug, thiserror::Error)]
enum PrepareLinear16Error {
    #[error(transparent)]
    Audio(#[from] anlg_audio_utils::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

async fn prepare_linear16(path: PathBuf, layout: ChannelLayout) -> Result<PreparedLinear16, Error> {
    tokio::task::spawn_blocking(move || prepare_linear16_blocking(&path, layout))
        .await?
        .map_err(|error| Error::AudioProcessing(error.to_string()))
}

fn prepare_linear16_blocking(
    path: &Path,
    layout: ChannelLayout,
) -> Result<PreparedLinear16, PrepareLinear16Error> {
    let decoder = source_from_path(path)?;
    let sample_rate: u32 = decoder.sample_rate().into();
    let mut file = tempfile::tempfile()?;
    let info = write_linear16(decoder, sample_rate, layout, &mut file)?;

    if info.frame_count == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "audio file contains no samples",
        )
        .into());
    }

    let byte_len = file.stream_position()?;
    file.seek(SeekFrom::Start(0))?;

    Ok(PreparedLinear16 {
        file,
        sample_rate: info.sample_rate,
        channel_count: match layout {
            ChannelLayout::Mono => 1,
            ChannelLayout::Interleaved => info.channels,
        },
        frame_count: info.frame_count,
        byte_len,
    })
}

fn write_linear16<S, W>(
    source: S,
    sample_rate: u32,
    layout: ChannelLayout,
    writer: &mut W,
) -> Result<anlg_audio_utils::ResampledAudioInfo, PrepareLinear16Error>
where
    S: Source,
    W: Write,
{
    for_each_resampled_channel_block(source, sample_rate, |channels| {
        let frame_count = channels.first().map_or(0, |channel| channel.len());
        let samples = match layout {
            ChannelLayout::Mono => f32_to_i16_bytes((0..frame_count).map(|frame| {
                channels.iter().map(|channel| channel[frame]).sum::<f32>() / channels.len() as f32
            })),
            ChannelLayout::Interleaved => f32_to_i16_bytes(
                (0..frame_count)
                    .flat_map(|frame| channels.iter().map(move |channel| channel[frame])),
            ),
        };
        writer.write_all(&samples)?;
        Ok(())
    })
}

const DEFAULT_CHUNK_MS: u64 = 500;
const DEFAULT_DELAY_MS: u64 = 20;
const MAX_STREAMING_CHUNK_BYTES: usize = 256 * 1024;

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

pub use crate::adapter::StreamingBatchStream;

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

        let prepared = prepare_linear16(path, ChannelLayout::Interleaved).await?;
        let audio_duration_secs = if prepared.sample_rate == 0 {
            0.0
        } else {
            prepared.frame_count as f64 / prepared.sample_rate as f64
        };

        let channel_count = prepared.channel_count.clamp(1, 2) as u8;
        let listen_params = ListenParams {
            channels: channel_count,
            sample_rate: prepared.sample_rate,
            ..params.clone()
        };

        let client = ListenClientBuilder::default()
            .adapter::<ArgmaxAdapter>()
            .api_base(api_base)
            .api_key(api_key)
            .params(listen_params)
            .build_with_channels(channel_count)
            .await?;

        let chunk_bytes = streaming_chunk_bytes(
            prepared.sample_rate,
            prepared.channel_count,
            config.chunk_ms,
        );
        let audio_stream =
            linear16_file_stream(tokio::fs::File::from_std(prepared.file), chunk_bytes);
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
                    to_batch_stream_event(response, percentage)
                })
                .map_err(|e| Error::WebSocket(format!("{:?}", e)))
        });

        Ok(Box::pin(mapped_stream))
    }
}

fn streaming_chunk_bytes(sample_rate: u32, channel_count: usize, chunk_ms: u64) -> usize {
    let bytes_per_frame = channel_count.max(1).saturating_mul(size_of::<i16>());
    let frames = (chunk_ms as u128)
        .saturating_mul(sample_rate as u128)
        .div_ceil(1000)
        .max(1);
    let requested = frames
        .saturating_mul(bytes_per_frame as u128)
        .min(usize::MAX as u128) as usize;

    requested.clamp(bytes_per_frame, MAX_STREAMING_CHUNK_BYTES) / bytes_per_frame * bytes_per_frame
}

fn linear16_file_stream(
    file: tokio::fs::File,
    chunk_bytes: usize,
) -> impl futures_util::Stream<Item = ListenClientInput> + Send {
    futures_util::stream::unfold(file, move |mut file| async move {
        let mut bytes = vec![0; chunk_bytes];
        match file.read(&mut bytes).await {
            Ok(0) => None,
            Ok(read) => {
                bytes.truncate(read);
                Some((MixedMessage::Audio(bytes.into()), file))
            }
            Err(_error) => {
                tracing::warn!(error.type = "audio_spool_read_failed", "argmax_linear16_spool_read_failed");
                None
            }
        }
    })
}

fn to_batch_stream_event(response: StreamResponse, percentage: f64) -> BatchStreamEvent {
    match response {
        StreamResponse::TranscriptResponse { .. } => BatchStreamEvent::Segment {
            response,
            percentage,
        },
        StreamResponse::TerminalResponse {
            request_id,
            created,
            duration,
            channels,
        } => BatchStreamEvent::Terminal {
            request_id,
            created,
            duration,
            channels,
        },
        StreamResponse::ErrorResponse {
            error_code,
            error_message,
            provider,
        } => BatchStreamEvent::Error {
            error_code,
            error_message,
            provider,
        },
        other => BatchStreamEvent::Segment {
            response: other,
            percentage,
        },
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
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[derive(Default)]
    struct TrackingWriter {
        total_bytes: usize,
        max_write_bytes: usize,
        write_count: usize,
    }

    impl Write for TrackingWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.total_bytes += bytes.len();
            self.max_write_bytes = self.max_write_bytes.max(bytes.len());
            self.write_count += 1;
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn long_linear16_input_is_written_in_bounded_blocks() {
        let frame_count = 16_000 * 60;
        let source = rodio::source::Zero::new_samples(
            rodio::nz!(2u16),
            rodio::nz!(16_000u32),
            frame_count * 2,
        );
        let mut writer = TrackingWriter::default();

        let info = write_linear16(source, 16_000, ChannelLayout::Mono, &mut writer).unwrap();

        assert_eq!(info.frame_count, frame_count);
        assert_eq!(writer.total_bytes, frame_count * size_of::<i16>());
        assert!(writer.write_count > 100);
        assert!(writer.max_write_bytes <= 1024 * size_of::<i16>());
    }

    #[test]
    fn linear16_writer_preserves_interleaved_channel_order() {
        let source = rodio::buffer::SamplesBuffer::new(
            rodio::nz!(2u16),
            rodio::nz!(16_000u32),
            vec![0.25, -0.25, 0.5, -0.5],
        );
        let mut output = Vec::new();

        let info = write_linear16(source, 16_000, ChannelLayout::Interleaved, &mut output).unwrap();
        let samples = output
            .chunks_exact(2)
            .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
            .collect::<Vec<_>>();

        assert_eq!(info.channels, 2);
        assert_eq!(info.frame_count, 2);
        assert_eq!(samples, vec![8192, -8192, 16384, -16384]);
    }

    #[tokio::test]
    async fn progressive_file_stream_keeps_chunks_bounded_and_ordered() {
        let total_bytes = MAX_STREAMING_CHUNK_BYTES * 3 + 124;
        let mut file = tempfile::tempfile().unwrap();
        for offset in (0..total_bytes).step_by(4096) {
            let write_len = (total_bytes - offset).min(4096);
            let bytes = (offset..offset + write_len)
                .map(|index| index as u8)
                .collect::<Vec<_>>();
            file.write_all(&bytes).unwrap();
        }
        file.seek(SeekFrom::Start(0)).unwrap();

        let mut stream = Box::pin(linear16_file_stream(
            tokio::fs::File::from_std(file),
            MAX_STREAMING_CHUNK_BYTES,
        ));
        let mut offset = 0usize;
        let mut chunk_count = 0usize;

        while let Some(message) = futures_util::StreamExt::next(&mut stream).await {
            let MixedMessage::Audio(bytes) = message else {
                panic!("file stream emitted a control message");
            };
            assert!(bytes.len() <= MAX_STREAMING_CHUNK_BYTES);
            for (index, byte) in bytes.iter().enumerate() {
                assert_eq!(*byte, (offset + index) as u8);
            }
            offset += bytes.len();
            chunk_count += 1;
        }

        assert_eq!(offset, total_bytes);
        assert_eq!(chunk_count, 4);
    }

    #[test]
    fn streaming_chunk_size_is_capped_and_frame_aligned() {
        let chunk_bytes = streaming_chunk_bytes(192_000, 8, u64::MAX);

        assert!(chunk_bytes <= MAX_STREAMING_CHUNK_BYTES);
        assert_eq!(chunk_bytes % (8 * size_of::<i16>()), 0);
    }

    #[tokio::test]
    async fn batch_request_streams_exact_linear16_body_length() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/listen"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "metadata": {},
                "results": { "channels": [] }
            })))
            .mount(&server)
            .await;

        do_transcribe_file(
            &create_client(),
            &server.uri(),
            "test-key",
            &ListenParams::default(),
            anlg_data::english_1::AUDIO_PATH.into(),
        )
        .await
        .unwrap();

        let request = server.received_requests().await.unwrap().pop().unwrap();
        let content_length = request.headers["content-length"]
            .to_str()
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let content_type = request.headers["content-type"].to_str().unwrap();
        let sample_rate = request
            .url
            .query_pairs()
            .find(|(key, _)| key == "sample_rate")
            .map(|(_, value)| value.into_owned())
            .unwrap();

        assert!(!request.body.is_empty());
        assert_eq!(content_length, request.body.len());
        assert_eq!(
            content_type,
            format!("audio/raw;encoding=linear16;rate={sample_rate}")
        );
        assert_eq!(request.body.len() % size_of::<i16>(), 0);
    }

    #[tokio::test]
    #[ignore]
    async fn test_argmax_batch_transcription() {
        let client = create_client();
        let adapter = ArgmaxAdapter::default();
        let params = ListenParams::default();

        let audio_path = std::path::PathBuf::from(anlg_data::english_1::AUDIO_PATH);

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
