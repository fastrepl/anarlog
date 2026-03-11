use std::time::{Duration, UNIX_EPOCH};

use bytes::Bytes;
use ractor::{ActorProcessingErr, ActorRef};

use owhisper_client::{
    AdapterKind, ArgmaxAdapter, AssemblyAIAdapter, CactusAdapter, DashScopeAdapter,
    DeepgramAdapter, ElevenLabsAdapter, FireworksAdapter, GladiaAdapter, HyprnoteAdapter,
    MistralAdapter, OpenAIAdapter, RealtimeSttAdapter, SonioxAdapter, hypr_ws_client,
};
use owhisper_interface::stream::Extra;
use owhisper_interface::{ControlMessage, MixedMessage};

use super::stream::process_stream;
use super::{ChannelSender, DEVICE_FINGERPRINT_HEADER, ListenerArgs, ListenerMsg, actor_error};
use crate::SessionErrorEvent;

pub(super) async fn spawn_rx_task(
    args: ListenerArgs,
    myself: ActorRef<ListenerMsg>,
) -> Result<
    (
        ChannelSender,
        tokio::task::JoinHandle<()>,
        tokio::sync::oneshot::Sender<()>,
        String,
    ),
    ActorProcessingErr,
> {
    let adapter_kind =
        AdapterKind::from_url_and_languages(&args.base_url, &args.languages, Some(&args.model));
    let is_dual = matches!(args.mode, crate::actors::ChannelMode::MicAndSpeaker);

    let result = match (adapter_kind, is_dual) {
        (AdapterKind::Argmax, false) => {
            spawn_rx_task_single_with_adapter::<ArgmaxAdapter>(args, myself).await
        }
        (AdapterKind::Argmax, true) => {
            spawn_rx_task_dual_with_adapter::<ArgmaxAdapter>(args, myself).await
        }
        (AdapterKind::Soniox, false) => {
            spawn_rx_task_single_with_adapter::<SonioxAdapter>(args, myself).await
        }
        (AdapterKind::Soniox, true) => {
            spawn_rx_task_dual_with_adapter::<SonioxAdapter>(args, myself).await
        }
        (AdapterKind::Fireworks, false) => {
            spawn_rx_task_single_with_adapter::<FireworksAdapter>(args, myself).await
        }
        (AdapterKind::Fireworks, true) => {
            spawn_rx_task_dual_with_adapter::<FireworksAdapter>(args, myself).await
        }
        (AdapterKind::Deepgram, false) => {
            spawn_rx_task_single_with_adapter::<DeepgramAdapter>(args, myself).await
        }
        (AdapterKind::Deepgram, true) => {
            spawn_rx_task_dual_with_adapter::<DeepgramAdapter>(args, myself).await
        }
        (AdapterKind::AssemblyAI, false) => {
            spawn_rx_task_single_with_adapter::<AssemblyAIAdapter>(args, myself).await
        }
        (AdapterKind::AssemblyAI, true) => {
            spawn_rx_task_dual_with_adapter::<AssemblyAIAdapter>(args, myself).await
        }
        (AdapterKind::OpenAI, false) => {
            spawn_rx_task_single_with_adapter::<OpenAIAdapter>(args, myself).await
        }
        (AdapterKind::OpenAI, true) => {
            spawn_rx_task_dual_with_adapter::<OpenAIAdapter>(args, myself).await
        }
        (AdapterKind::Gladia, false) => {
            spawn_rx_task_single_with_adapter::<GladiaAdapter>(args, myself).await
        }
        (AdapterKind::Gladia, true) => {
            spawn_rx_task_dual_with_adapter::<GladiaAdapter>(args, myself).await
        }
        (AdapterKind::ElevenLabs, false) => {
            spawn_rx_task_single_with_adapter::<ElevenLabsAdapter>(args, myself).await
        }
        (AdapterKind::ElevenLabs, true) => {
            spawn_rx_task_dual_with_adapter::<ElevenLabsAdapter>(args, myself).await
        }
        (AdapterKind::DashScope, false) => {
            spawn_rx_task_single_with_adapter::<DashScopeAdapter>(args, myself).await
        }
        (AdapterKind::DashScope, true) => {
            spawn_rx_task_dual_with_adapter::<DashScopeAdapter>(args, myself).await
        }
        (AdapterKind::Mistral, false) => {
            spawn_rx_task_single_with_adapter::<MistralAdapter>(args, myself).await
        }
        (AdapterKind::Mistral, true) => {
            spawn_rx_task_dual_with_adapter::<MistralAdapter>(args, myself).await
        }
        (AdapterKind::Hyprnote, false) => {
            spawn_rx_task_single_with_adapter::<HyprnoteAdapter>(args, myself).await
        }
        (AdapterKind::Hyprnote, true) => {
            spawn_rx_task_dual_with_adapter::<HyprnoteAdapter>(args, myself).await
        }
        (AdapterKind::Cactus, false) => {
            spawn_rx_task_single_with_adapter::<CactusAdapter>(args, myself).await
        }
        (AdapterKind::Cactus, true) => {
            spawn_rx_task_dual_with_adapter::<CactusAdapter>(args, myself).await
        }
    }?;

    Ok((result.0, result.1, result.2, adapter_kind.to_string()))
}

fn build_listen_params(args: &ListenerArgs) -> owhisper_interface::ListenParams {
    let redemption_time_ms = if args.onboarding { "60" } else { "400" };
    owhisper_interface::ListenParams {
        model: Some(args.model.clone()),
        languages: args.languages.clone(),
        sample_rate: super::super::SAMPLE_RATE,
        keywords: args.keywords.clone(),
        custom_query: Some(std::collections::HashMap::from([(
            "redemption_time_ms".to_string(),
            redemption_time_ms.to_string(),
        )])),
        ..Default::default()
    }
}

fn build_extra(args: &ListenerArgs) -> (f64, Extra) {
    let session_offset_secs = args.session_started_at.elapsed().as_secs_f64();
    let started_unix_millis = args
        .session_started_at_unix
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_millis()
        .min(u64::MAX as u128) as u64;

    let extra = Extra {
        started_unix_millis,
    };

    (session_offset_secs, extra)
}

fn desktop_connect_policy() -> hypr_ws_client::client::WebSocketConnectPolicy {
    hypr_ws_client::client::WebSocketConnectPolicy {
        connect_timeout: Duration::from_secs(4),
        max_attempts: 2,
        retry_delay: Duration::from_secs(1),
    }
}

async fn spawn_rx_task_single_with_adapter<A: RealtimeSttAdapter>(
    args: ListenerArgs,
    myself: ActorRef<ListenerMsg>,
) -> Result<
    (
        ChannelSender,
        tokio::task::JoinHandle<()>,
        tokio::sync::oneshot::Sender<()>,
    ),
    ActorProcessingErr,
> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let (session_offset_secs, extra) = build_extra(&args);

    let (tx, rx) = tokio::sync::mpsc::channel::<MixedMessage<Bytes, ControlMessage>>(32);

    let client = owhisper_client::ListenClient::builder()
        .adapter::<A>()
        .api_base(args.base_url.clone())
        .api_key(args.api_key.clone())
        .params(build_listen_params(&args))
        .connect_policy(desktop_connect_policy())
        .extra_header(DEVICE_FINGERPRINT_HEADER, hypr_host::fingerprint())
        .build_single()
        .await;

    let outbound = tokio_stream::wrappers::ReceiverStream::new(rx);

    let (listen_stream, handle) = match client.from_realtime_audio(outbound).await {
        Err(e) => {
            tracing::error!(
                hyprnote.session.id = %args.session_id,
                error.message = ?e,
                "listen_ws_connect_failed(single)"
            );
            args.runtime.emit_error(SessionErrorEvent::ConnectionError {
                session_id: args.session_id.clone(),
                error: format!("listen_ws_connect_failed: {:?}", e),
            });
            return Err(actor_error(format!("listen_ws_connect_failed: {:?}", e)));
        }
        Ok(res) => res,
    };

    let rx_task = tokio::spawn(async move {
        futures_util::pin_mut!(listen_stream);
        process_stream(
            listen_stream,
            handle,
            myself,
            shutdown_rx,
            session_offset_secs,
            extra,
        )
        .await;
    });

    Ok((ChannelSender::Single(tx), rx_task, shutdown_tx))
}

async fn spawn_rx_task_dual_with_adapter<A: RealtimeSttAdapter>(
    args: ListenerArgs,
    myself: ActorRef<ListenerMsg>,
) -> Result<
    (
        ChannelSender,
        tokio::task::JoinHandle<()>,
        tokio::sync::oneshot::Sender<()>,
    ),
    ActorProcessingErr,
> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let (session_offset_secs, extra) = build_extra(&args);

    let (tx, rx) = tokio::sync::mpsc::channel::<MixedMessage<(Bytes, Bytes), ControlMessage>>(32);

    let client = owhisper_client::ListenClient::builder()
        .adapter::<A>()
        .api_base(args.base_url.clone())
        .api_key(args.api_key.clone())
        .params(build_listen_params(&args))
        .connect_policy(desktop_connect_policy())
        .extra_header(DEVICE_FINGERPRINT_HEADER, hypr_host::fingerprint())
        .build_dual()
        .await;

    let outbound = tokio_stream::wrappers::ReceiverStream::new(rx);

    let (listen_stream, handle) = match client.from_realtime_audio(outbound).await {
        Err(e) => {
            tracing::error!(
                hyprnote.session.id = %args.session_id,
                error.message = ?e,
                "listen_ws_connect_failed(dual)"
            );
            args.runtime.emit_error(SessionErrorEvent::ConnectionError {
                session_id: args.session_id.clone(),
                error: format!("listen_ws_connect_failed: {:?}", e),
            });
            return Err(actor_error(format!("listen_ws_connect_failed: {:?}", e)));
        }
        Ok(res) => res,
    };

    let rx_task = tokio::spawn(async move {
        futures_util::pin_mut!(listen_stream);
        process_stream(
            listen_stream,
            handle,
            myself,
            shutdown_rx,
            session_offset_secs,
            extra,
        )
        .await;
    });

    Ok((ChannelSender::Dual(tx), rx_task, shutdown_tx))
}
