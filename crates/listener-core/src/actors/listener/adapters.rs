use std::time::{Duration, UNIX_EPOCH};

use bytes::Bytes;
use ractor::{ActorProcessingErr, ActorRef};

use owhisper_client::{
    AdapterKind, ArgmaxAdapter, AssemblyAIAdapter, CactusAdapter, DashScopeAdapter,
    DeepgramAdapter, ElevenLabsAdapter, FireworksAdapter, GladiaAdapter, HyprnoteAdapter,
    MistralAdapter, RealtimeSttAdapter, SonioxAdapter, hypr_ws_client,
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

    macro_rules! dispatch_realtime {
        ($ak:expr, $is_dual:expr, $args:expr, $myself:expr,
         { $($var:ident => $adapter:ty),+ $(,)? },
         batch_only: [$($bo:ident),* $(,)?]
        ) => {
            match ($ak, $is_dual) {
                $(
                    (AdapterKind::$var, false) => {
                        spawn_rx_task_single_with_adapter::<$adapter>($args, $myself).await
                    }
                    (AdapterKind::$var, true) => {
                        spawn_rx_task_dual_with_adapter::<$adapter>($args, $myself).await
                    }
                )+
                $(
                    (AdapterKind::$bo, _) => {
                        return Err(actor_error(
                            concat!("provider_batch_only: ", stringify!($bo), " only supports batch transcription")
                        ));
                    }
                )*
            }
        };
    }

    let result = dispatch_realtime!(adapter_kind, is_dual, args, myself, {
        Argmax => ArgmaxAdapter,
        Soniox => SonioxAdapter,
        Fireworks => FireworksAdapter,
        Deepgram => DeepgramAdapter,
        AssemblyAI => AssemblyAIAdapter,
        Gladia => GladiaAdapter,
        ElevenLabs => ElevenLabsAdapter,
        DashScope => DashScopeAdapter,
        Mistral => MistralAdapter,
        Hyprnote => HyprnoteAdapter,
        Cactus => CactusAdapter,
    }, batch_only: [OpenAI, AquaVoice, Pyannote])?;

    Ok((result.0, result.1, result.2, adapter_kind.to_string()))
}

fn build_listen_params(args: &ListenerArgs) -> owhisper_interface::ListenParams {
    let adapter_kind =
        AdapterKind::from_url_and_languages(&args.base_url, &args.languages, Some(&args.model));
    let redemption_time_ms = if args.onboarding { "60" } else { "400" };
    let mut custom_query = std::collections::HashMap::from([(
        "redemption_time_ms".to_string(),
        redemption_time_ms.to_string(),
    )]);

    if adapter_kind == AdapterKind::AssemblyAI
        && let Some(expected_speakers) = assemblyai_expected_speakers(args)
    {
        custom_query.insert("speaker_labels".to_string(), "true".to_string());
        custom_query.insert("max_speakers".to_string(), expected_speakers.to_string());
    }

    owhisper_interface::ListenParams {
        model: Some(args.model.clone()),
        languages: args.languages.clone(),
        sample_rate: super::super::SAMPLE_RATE,
        keywords: args.keywords.clone(),
        custom_query: Some(custom_query),
        ..Default::default()
    }
}

fn assemblyai_expected_speakers(args: &ListenerArgs) -> Option<u32> {
    let mut participants = args.participant_human_ids.clone();

    if let Some(self_human_id) = &args.self_human_id
        && !participants.iter().any(|id| id == self_human_id)
    {
        participants.push(self_human_id.clone());
    }

    participants.sort();
    participants.dedup();

    (participants.len() > 1).then_some(participants.len() as u32)
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Instant, SystemTime};

    use super::*;

    struct NoopRuntime;

    impl hypr_storage::StorageRuntime for NoopRuntime {
        fn global_base(&self) -> Result<std::path::PathBuf, hypr_storage::Error> {
            Ok(std::path::PathBuf::from("/tmp"))
        }

        fn vault_base(&self) -> Result<std::path::PathBuf, hypr_storage::Error> {
            Ok(std::path::PathBuf::from("/tmp"))
        }
    }

    impl crate::ListenerRuntime for NoopRuntime {
        fn emit_lifecycle(&self, _event: crate::SessionLifecycleEvent) {}

        fn emit_progress(&self, _event: crate::SessionProgressEvent) {}

        fn emit_data(&self, _event: crate::SessionDataEvent) {}

        fn emit_error(&self, _event: crate::SessionErrorEvent) {}
    }

    fn listener_args(base_url: &str, model: &str) -> ListenerArgs {
        ListenerArgs {
            runtime: Arc::new(NoopRuntime),
            languages: vec![hypr_language::ISO639::En.into()],
            onboarding: false,
            model: model.to_string(),
            base_url: base_url.to_string(),
            api_key: String::new(),
            keywords: vec![],
            mode: crate::actors::ChannelMode::MicOnly,
            session_started_at: Instant::now(),
            session_started_at_unix: SystemTime::now(),
            session_id: "session".to_string(),
            participant_human_ids: vec![],
            self_human_id: None,
        }
    }

    #[test]
    fn assemblyai_expected_speakers_counts_distinct_participants() {
        let mut args = listener_args("https://api.assemblyai.com", "u3-rt-pro");
        args.participant_human_ids = vec!["remote".to_string(), "self".to_string()];
        args.self_human_id = Some("self".to_string());

        assert_eq!(assemblyai_expected_speakers(&args), Some(2));
    }

    #[test]
    fn build_listen_params_adds_assemblyai_diarization_hints() {
        let mut args = listener_args("https://api.assemblyai.com", "u3-rt-pro");
        args.participant_human_ids = vec!["remote".to_string()];
        args.self_human_id = Some("self".to_string());

        let params = build_listen_params(&args);
        let custom_query = params.custom_query.expect("custom query");

        assert_eq!(
            custom_query.get("speaker_labels").map(String::as_str),
            Some("true")
        );
        assert_eq!(
            custom_query.get("max_speakers").map(String::as_str),
            Some("2")
        );
    }

    #[test]
    fn build_listen_params_does_not_add_assemblyai_hints_for_other_providers() {
        let mut args = listener_args("https://api.deepgram.com/v1", "nova-3");
        args.participant_human_ids = vec!["remote".to_string()];
        args.self_human_id = Some("self".to_string());

        let params = build_listen_params(&args);
        let custom_query = params.custom_query.expect("custom query");

        assert!(!custom_query.contains_key("speaker_labels"));
        assert!(!custom_query.contains_key("max_speakers"));
    }
}
