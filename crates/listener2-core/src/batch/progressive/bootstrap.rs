use owhisper_client::{ArgmaxAdapter, CactusAdapter, OpenAIAdapter, WhisperCppAdapter};
use ractor::{ActorProcessingErr, ActorRef};
use tracing::Instrument;

use super::ProgressiveProvider;
use super::actor::{
    BatchArgs, BatchMsg, BatchStartNotifier, process_provider_stream, report_stream_start_failure,
};

pub(super) async fn spawn_progressive_batch_task(
    args: BatchArgs,
    myself: ActorRef<BatchMsg>,
) -> Result<
    (
        tokio::task::JoinHandle<()>,
        tokio::sync::oneshot::Sender<()>,
    ),
    ActorProcessingErr,
> {
    match args.progressive_provider {
        ProgressiveProvider::Argmax => spawn_argmax_progressive_batch_task(args, myself).await,
        ProgressiveProvider::Cactus => spawn_cactus_batch_task(args, myself).await,
        ProgressiveProvider::OpenAI => spawn_openai_batch_task(args, myself).await,
        ProgressiveProvider::WhisperCpp => spawn_whispercpp_batch_task(args, myself).await,
    }
}

async fn spawn_argmax_progressive_batch_task(
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
        "argmax_progressive_batch",
        hyprnote.session.id = %args.session_id,
        url.full = %args.base_url,
        hyprnote.file.path = %args.file_path,
    );

    let rx_task = tokio::spawn(
        async move {
            tracing::info!("argmax progressive batch task: starting");

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
                        &args.provider_label,
                        &err,
                        "argmax progressive batch task failed to start",
                    );
                    return;
                }
            };

            process_provider_stream(
                stream,
                myself,
                shutdown_rx,
                &args.provider_label,
                "argmax progressive batch",
            )
            .await;
            tracing::info!("argmax progressive batch task exited");
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
        "cactus_progressive_batch",
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
                        &args.provider_label,
                        &err,
                        "cactus progressive batch failed to start",
                    );
                    return;
                }
            };

            process_provider_stream(
                stream,
                myself,
                shutdown_rx,
                &args.provider_label,
                "cactus progressive batch",
            )
            .await;
        }
        .instrument(span),
    );

    Ok((rx_task, shutdown_tx))
}

async fn spawn_whispercpp_batch_task(
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
        "whispercpp_progressive_batch",
        hyprnote.session.id = %args.session_id,
        url.full = %args.base_url,
        hyprnote.file.path = %args.file_path,
    );

    let rx_task = tokio::spawn(
        async move {
            let stream = match WhisperCppAdapter::transcribe_file_streaming(
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
                        &args.provider_label,
                        &err,
                        "whispercpp progressive batch failed to start",
                    );
                    return;
                }
            };

            process_provider_stream(
                stream,
                myself,
                shutdown_rx,
                &args.provider_label,
                "whispercpp progressive batch",
            )
            .await;
        }
        .instrument(span),
    );

    Ok((rx_task, shutdown_tx))
}

async fn spawn_openai_batch_task(
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
        "openai_progressive_batch",
        hyprnote.session.id = %args.session_id,
        url.full = %args.base_url,
        hyprnote.file.path = %args.file_path,
    );

    let rx_task = tokio::spawn(
        async move {
            let stream = match OpenAIAdapter::transcribe_file_streaming(
                &args.base_url,
                &args.api_key,
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
                        &args.provider_label,
                        &err,
                        "openai progressive batch failed to start",
                    );
                    return;
                }
            };

            process_provider_stream(
                stream,
                myself,
                shutdown_rx,
                &args.provider_label,
                "openai progressive batch",
            )
            .await;
        }
        .instrument(span),
    );

    Ok((rx_task, shutdown_tx))
}

pub(super) fn notify_start_result(notifier: &BatchStartNotifier, result: crate::Result<()>) {
    if let Ok(mut guard) = notifier.lock()
        && let Some(sender) = guard.take()
    {
        let _ = sender.send(result);
    }
}
