use std::path::PathBuf;
use std::time::Duration;

use owhisper_client::{
    AdapterKind, ArgmaxAdapter, AssemblyAIAdapter, CactusAdapter, DashScopeAdapter,
    DeepgramAdapter, ElevenLabsAdapter, FireworksAdapter, GladiaAdapter, HyprnoteAdapter,
    MistralAdapter, OpenAIAdapter, RealtimeSttAdapter, SonioxAdapter,
};
use owhisper_interface::{ControlMessage, MixedMessage};
use ractor::{ActorProcessingErr, ActorRef};
use tokio_stream::{self as tokio_stream, StreamExt as TokioStreamExt};
use tracing::Instrument;

use super::actor::{
    BatchArgs, BatchMsg, BatchStartNotifier, process_batch_stream, process_provider_stream,
    report_stream_start_failure,
};

const DEFAULT_CHUNK_MS: u64 = 500;
const DEFAULT_DELAY_MS: u64 = 20;
const DEVICE_FINGERPRINT_HEADER: &str = "x-device-fingerprint";

pub(super) async fn spawn_batch_task(
    args: BatchArgs,
    myself: ActorRef<BatchMsg>,
) -> Result<
    (
        tokio::task::JoinHandle<()>,
        tokio::sync::oneshot::Sender<()>,
    ),
    ActorProcessingErr,
> {
    let adapter_kind = AdapterKind::from_url_and_languages(
        &args.base_url,
        &args.listen_params.languages,
        args.listen_params.model.as_deref(),
    );

    match adapter_kind {
        AdapterKind::Argmax => spawn_argmax_streaming_batch_task(args, myself).await,
        AdapterKind::Soniox => spawn_batch_task_with_adapter::<SonioxAdapter>(args, myself).await,
        AdapterKind::Fireworks => {
            spawn_batch_task_with_adapter::<FireworksAdapter>(args, myself).await
        }
        AdapterKind::Deepgram => {
            spawn_batch_task_with_adapter::<DeepgramAdapter>(args, myself).await
        }
        AdapterKind::AssemblyAI => {
            spawn_batch_task_with_adapter::<AssemblyAIAdapter>(args, myself).await
        }
        AdapterKind::OpenAI => spawn_batch_task_with_adapter::<OpenAIAdapter>(args, myself).await,
        AdapterKind::Gladia => spawn_batch_task_with_adapter::<GladiaAdapter>(args, myself).await,
        AdapterKind::ElevenLabs => {
            spawn_batch_task_with_adapter::<ElevenLabsAdapter>(args, myself).await
        }
        AdapterKind::DashScope => {
            spawn_batch_task_with_adapter::<DashScopeAdapter>(args, myself).await
        }
        AdapterKind::Mistral => spawn_batch_task_with_adapter::<MistralAdapter>(args, myself).await,
        AdapterKind::Hyprnote => {
            spawn_batch_task_with_adapter::<HyprnoteAdapter>(args, myself).await
        }
        AdapterKind::Cactus => spawn_cactus_batch_task(args, myself).await,
    }
}

async fn spawn_argmax_streaming_batch_task(
    args: BatchArgs,
    myself: ActorRef<BatchMsg>,
) -> Result<
    (
        tokio::task::JoinHandle<()>,
        tokio::sync::oneshot::Sender<()>,
    ),
    ActorProcessingErr,
> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let span = tracing::info_span!(
        "argmax_streaming_batch",
        hyprnote.session.id = %args.session_id,
        url.full = %args.base_url,
        hyprnote.file.path = %args.file_path,
    );

    let rx_task = tokio::spawn(
        async move {
            tracing::info!("argmax streaming batch task: starting");

            let stream = match ArgmaxAdapter::transcribe_file_streaming(
                &args.base_url,
                &args.api_key,
                &args.listen_params,
                &args.file_path,
                None,
            )
            .await
            {
                Ok(stream) => {
                    notify_start_result(&args.start_notifier, Ok(()));
                    stream
                }
                Err(err) => {
                    report_stream_start_failure(
                        &myself,
                        &args.start_notifier,
                        &err,
                        "argmax streaming batch task failed to start",
                    );
                    return;
                }
            };

            process_provider_stream(stream, myself, shutdown_rx, "argmax streaming batch").await;
            tracing::info!("argmax streaming batch task exited");
        }
        .instrument(span),
    );

    Ok((rx_task, shutdown_tx))
}

async fn spawn_cactus_batch_task(
    args: BatchArgs,
    myself: ActorRef<BatchMsg>,
) -> Result<
    (
        tokio::task::JoinHandle<()>,
        tokio::sync::oneshot::Sender<()>,
    ),
    ActorProcessingErr,
> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let span = tracing::info_span!(
        "cactus_batch",
        hyprnote.session.id = %args.session_id,
        url.full = %args.base_url,
        hyprnote.file.path = %args.file_path,
    );

    let rx_task = tokio::spawn(
        async move {
            let stream = match CactusAdapter::transcribe_file_streaming(
                &args.base_url,
                &args.listen_params,
                &args.file_path,
            )
            .await
            {
                Ok(stream) => {
                    notify_start_result(&args.start_notifier, Ok(()));
                    stream
                }
                Err(err) => {
                    report_stream_start_failure(
                        &myself,
                        &args.start_notifier,
                        &err,
                        "cactus batch failed to start stream",
                    );
                    return;
                }
            };

            process_provider_stream(stream, myself, shutdown_rx, "cactus batch").await;
        }
        .instrument(span),
    );

    Ok((rx_task, shutdown_tx))
}

async fn spawn_batch_task_with_adapter<A: RealtimeSttAdapter>(
    args: BatchArgs,
    myself: ActorRef<BatchMsg>,
) -> Result<
    (
        tokio::task::JoinHandle<()>,
        tokio::sync::oneshot::Sender<()>,
    ),
    ActorProcessingErr,
> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let span = tracing::info_span!(
        "realtime_batch",
        hyprnote.session.id = %args.session_id,
        url.full = %args.base_url,
        hyprnote.file.path = %args.file_path,
    );

    let rx_task = tokio::spawn(
        async move {
            tracing::info!("batch task: loading audio chunks from file");
            let stream_config = BatchStreamConfig::new(DEFAULT_CHUNK_MS, DEFAULT_DELAY_MS);

            let chunked_audio = match load_chunked_audio(
                &args.file_path,
                stream_config,
                &myself,
                &args.start_notifier,
            )
            .await
            {
                Some(chunked_audio) => chunked_audio,
                None => return,
            };

            let audio_duration_secs = compute_audio_duration_secs(
                chunked_audio.frame_count,
                chunked_audio.metadata.sample_rate,
            );
            let channels = chunked_audio.metadata.channels;
            let chunk_count = chunked_audio.chunks.len();
            let listen_params = owhisper_interface::ListenParams {
                channels,
                sample_rate: chunked_audio.metadata.sample_rate,
                ..args.listen_params.clone()
            };
            let chunk_interval = stream_config.chunk_interval();

            if channels >= 2 {
                let client = owhisper_client::ListenClient::builder()
                    .adapter::<A>()
                    .api_base(args.base_url.clone())
                    .api_key(args.api_key.clone())
                    .params(listen_params)
                    .extra_header(DEVICE_FINGERPRINT_HEADER, hypr_host::fingerprint())
                    .build_dual()
                    .await;

                let audio_stream =
                    tokio_stream::iter(chunked_audio.chunks.into_iter().map(|chunk| {
                        let (mic, spk) = split_stereo_i16_bytes(&chunk);
                        MixedMessage::Audio((mic, spk))
                    }));
                let finalize_stream =
                    tokio_stream::iter(vec![MixedMessage::Control(ControlMessage::Finalize)]);
                let outbound = TokioStreamExt::throttle(
                    TokioStreamExt::chain(audio_stream, finalize_stream),
                    chunk_interval,
                );

                tracing::info!(
                    "batch task (dual): starting audio stream with {} chunks + finalize message",
                    chunk_count
                );
                let (listen_stream, _handle) =
                    match client.from_realtime_audio(Box::pin(outbound)).await {
                        Ok(result) => result,
                        Err(err) => {
                            report_stream_start_failure(
                                &myself,
                                &args.start_notifier,
                                &err,
                                "batch task (dual) failed to start audio stream",
                            );
                            return;
                        }
                    };

                notify_start_result(&args.start_notifier, Ok(()));
                futures_util::pin_mut!(listen_stream);
                process_batch_stream(listen_stream, myself, shutdown_rx, audio_duration_secs, 2)
                    .await;
            } else {
                let client = owhisper_client::ListenClient::builder()
                    .adapter::<A>()
                    .api_base(args.base_url.clone())
                    .api_key(args.api_key.clone())
                    .params(listen_params)
                    .extra_header(DEVICE_FINGERPRINT_HEADER, hypr_host::fingerprint())
                    .build_with_channels(channels.clamp(1, 2))
                    .await;

                let audio_stream =
                    tokio_stream::iter(chunked_audio.chunks.into_iter().map(MixedMessage::Audio));
                let finalize_stream =
                    tokio_stream::iter(vec![MixedMessage::Control(ControlMessage::Finalize)]);
                let outbound = TokioStreamExt::throttle(
                    TokioStreamExt::chain(audio_stream, finalize_stream),
                    chunk_interval,
                );

                tracing::info!(
                    "batch task: starting audio stream with {} chunks + finalize message",
                    chunk_count
                );
                let (listen_stream, _handle) =
                    match client.from_realtime_audio(Box::pin(outbound)).await {
                        Ok(result) => result,
                        Err(err) => {
                            report_stream_start_failure(
                                &myself,
                                &args.start_notifier,
                                &err,
                                "batch task failed to start audio stream",
                            );
                            return;
                        }
                    };

                notify_start_result(&args.start_notifier, Ok(()));
                futures_util::pin_mut!(listen_stream);
                process_batch_stream(listen_stream, myself, shutdown_rx, audio_duration_secs, 1)
                    .await;
            }
        }
        .instrument(span),
    );

    Ok((rx_task, shutdown_tx))
}

fn split_stereo_i16_bytes(data: &[u8]) -> (bytes::Bytes, bytes::Bytes) {
    let frame_count = data.len() / 4;
    let mut mic = bytes::BytesMut::with_capacity(frame_count * 2);
    let mut spk = bytes::BytesMut::with_capacity(frame_count * 2);
    for frame in data.chunks_exact(4) {
        mic.extend_from_slice(&frame[0..2]);
        spk.extend_from_slice(&frame[2..4]);
    }
    (mic.freeze(), spk.freeze())
}

#[derive(Clone, Copy)]
struct BatchStreamConfig {
    chunk_ms: u64,
    delay_ms: u64,
}

impl BatchStreamConfig {
    fn new(chunk_ms: u64, delay_ms: u64) -> Self {
        Self {
            chunk_ms: chunk_ms.max(1),
            delay_ms,
        }
    }

    fn chunk_interval(&self) -> Duration {
        Duration::from_millis(self.delay_ms)
    }
}

async fn load_chunked_audio(
    file_path: &str,
    stream_config: BatchStreamConfig,
    myself: &ActorRef<BatchMsg>,
    start_notifier: &BatchStartNotifier,
) -> Option<hypr_audio_utils::ChunkedAudio> {
    let chunk_result = tokio::task::spawn_blocking({
        let path = PathBuf::from(file_path);
        let chunk_ms = stream_config.chunk_ms;
        move || hypr_audio_utils::chunk_audio_file(path, chunk_ms)
    })
    .await;

    match chunk_result {
        Ok(Ok(data)) => {
            tracing::info!("batch task: loaded {} audio chunks", data.chunks.len());
            Some(data)
        }
        Ok(Err(err)) => {
            report_stream_start_failure(
                myself,
                start_notifier,
                &err,
                "batch task failed to load audio chunks",
            );
            None
        }
        Err(join_err) => {
            report_stream_start_failure(
                myself,
                start_notifier,
                &join_err,
                "batch task audio chunk loading panicked",
            );
            None
        }
    }
}

fn compute_audio_duration_secs(frame_count: usize, sample_rate: u32) -> f64 {
    if frame_count == 0 || sample_rate == 0 {
        0.0
    } else {
        frame_count as f64 / sample_rate as f64
    }
}

pub(super) fn notify_start_result(notifier: &BatchStartNotifier, result: crate::Result<()>) {
    if let Ok(mut guard) = notifier.lock()
        && let Some(sender) = guard.take()
    {
        let _ = sender.send(result);
    }
}
