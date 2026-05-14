use std::str::FromStr;
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
use super::{
    ChannelSender, DEVICE_FINGERPRINT_HEADER, ListenerArgs, ListenerMsg, SoniqoAudioMsg,
    actor_error,
};
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
    if args.transcription_mode != crate::TranscriptionMode::Live {
        return Err(actor_error(
            "listener_batch_mode: live listener is disabled for batch transcription",
        ));
    }

    if let Some(model) = soniqo_model_for_args(&args)? {
        if !model.is_available_on_current_platform() {
            return Err(actor_error(
                "unsupported_platform: Soniqo realtime transcription requires macOS Apple Silicon",
            ));
        }

        if !model.supports_live() {
            return Err(actor_error(format!(
                "provider_batch_only: {} only supports batch transcription",
                model.as_str()
            )));
        }

        let result = spawn_soniqo_rx_task(model, args, myself).await?;
        return Ok((result.0, result.1, result.2, "soniqo".to_string()));
    }

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

fn soniqo_model_for_args(
    args: &ListenerArgs,
) -> Result<Option<hypr_transcribe_soniqo::SoniqoModel>, ActorProcessingErr> {
    if let Some(model) =
        hypr_transcribe_soniqo::local_model_from_request(&args.base_url, &args.model)
    {
        return Ok(Some(model));
    }

    if hypr_transcribe_soniqo::is_local_base_url(&args.base_url) {
        return hypr_transcribe_soniqo::SoniqoModel::from_str(&args.model)
            .map(Some)
            .map_err(|e| actor_error(format!("soniqo_model_invalid: {e}")));
    }

    Ok(None)
}

async fn spawn_soniqo_rx_task(
    model: hypr_transcribe_soniqo::SoniqoModel,
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
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let (session_offset_secs, extra) = build_extra(&args);
    let (tx, mut rx) = tokio::sync::mpsc::channel::<SoniqoAudioMsg>(32);

    let session = tokio::task::spawn_blocking(move || {
        hypr_transcribe_soniqo::LiveTranscriptionSession::start(model)
    })
    .await
    .map_err(|e| actor_error(format!("soniqo_live_start_join_failed: {e}")))?
    .map_err(|e| actor_error(format!("soniqo_live_start_failed: {e}")))?;

    let rx_task = tokio::spawn(async move {
        let mut session = session;
        let mut mic_buffer = Vec::<f32>::new();
        let mut spk_buffer = Vec::<f32>::new();
        let mut mic_cursor = 0.0;
        let mut spk_cursor = 0.0;
        let mut interval = tokio::time::interval(Duration::from_millis(250));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    break;
                }
                maybe_msg = rx.recv() => {
                    let Some(msg) = maybe_msg else {
                        break;
                    };

                    match msg {
                        SoniqoAudioMsg::Single(source, audio) => {
                            match source {
                                hypr_transcribe_soniqo::TranscriptSource::Microphone => {
                                    mic_buffer.extend(i16_bytes_to_f32(&audio));
                                }
                                hypr_transcribe_soniqo::TranscriptSource::System => {
                                    spk_buffer.extend(i16_bytes_to_f32(&audio));
                                }
                            }
                        }
                        SoniqoAudioMsg::Dual(mic, spk) => {
                            mic_buffer.extend(i16_bytes_to_f32(&mic));
                            spk_buffer.extend(i16_bytes_to_f32(&spk));
                        }
                    }
                }
                _ = interval.tick() => {
                    if !mic_buffer.is_empty() {
                        let samples = std::mem::take(&mut mic_buffer);
                        let duration = samples.len() as f64 / super::super::SAMPLE_RATE as f64;
                        let start = mic_cursor;
                        mic_cursor += duration;

                        match flush_soniqo_source(
                            session,
                            model,
                            hypr_transcribe_soniqo::TranscriptSource::Microphone,
                            samples,
                            start,
                            duration,
                            myself.clone(),
                            session_offset_secs,
                            extra.clone(),
                        )
                        .await
                        {
                            Ok(next_session) => session = next_session,
                            Err(error) => {
                                let _ = myself.cast(ListenerMsg::StreamError(error));
                                return;
                            }
                        }
                    }

                    if !spk_buffer.is_empty() {
                        let samples = std::mem::take(&mut spk_buffer);
                        let duration = samples.len() as f64 / super::super::SAMPLE_RATE as f64;
                        let start = spk_cursor;
                        spk_cursor += duration;

                        match flush_soniqo_source(
                            session,
                            model,
                            hypr_transcribe_soniqo::TranscriptSource::System,
                            samples,
                            start,
                            duration,
                            myself.clone(),
                            session_offset_secs,
                            extra.clone(),
                        )
                        .await
                        {
                            Ok(next_session) => session = next_session,
                            Err(error) => {
                                let _ = myself.cast(ListenerMsg::StreamError(error));
                                return;
                            }
                        }
                    }
                }
            }
        }

        if !mic_buffer.is_empty() {
            let samples = std::mem::take(&mut mic_buffer);
            let duration = samples.len() as f64 / super::super::SAMPLE_RATE as f64;
            let start = mic_cursor;
            mic_cursor += duration;
            match flush_soniqo_source(
                session,
                model,
                hypr_transcribe_soniqo::TranscriptSource::Microphone,
                samples,
                start,
                duration,
                myself.clone(),
                session_offset_secs,
                extra.clone(),
            )
            .await
            {
                Ok(next_session) => session = next_session,
                Err(error) => {
                    tracing::warn!(%error, "soniqo_final_mic_flush_failed");
                    return;
                }
            }
        }

        if !spk_buffer.is_empty() {
            let samples = std::mem::take(&mut spk_buffer);
            let duration = samples.len() as f64 / super::super::SAMPLE_RATE as f64;
            let start = spk_cursor;
            spk_cursor += duration;
            match flush_soniqo_source(
                session,
                model,
                hypr_transcribe_soniqo::TranscriptSource::System,
                samples,
                start,
                duration,
                myself.clone(),
                session_offset_secs,
                extra.clone(),
            )
            .await
            {
                Ok(next_session) => session = next_session,
                Err(error) => {
                    tracing::warn!(%error, "soniqo_final_system_flush_failed");
                    return;
                }
            }
        }

        match finalize_soniqo_source(
            session,
            model,
            hypr_transcribe_soniqo::TranscriptSource::Microphone,
            mic_cursor,
            myself.clone(),
            session_offset_secs,
            extra.clone(),
        )
        .await
        {
            Ok(next_session) => session = next_session,
            Err(error) => {
                tracing::warn!(%error, "soniqo_final_mic_finalize_failed");
                return;
            }
        }

        match finalize_soniqo_source(
            session,
            model,
            hypr_transcribe_soniqo::TranscriptSource::System,
            spk_cursor,
            myself,
            session_offset_secs,
            extra,
        )
        .await
        {
            Ok(next_session) => session = next_session,
            Err(error) => {
                tracing::warn!(%error, "soniqo_final_system_finalize_failed");
                return;
            }
        }

        let _ = tokio::task::spawn_blocking(move || session.stop()).await;
    });

    Ok((ChannelSender::Soniqo(tx), rx_task, shutdown_tx))
}

async fn flush_soniqo_source(
    session: hypr_transcribe_soniqo::LiveTranscriptionSession,
    model: hypr_transcribe_soniqo::SoniqoModel,
    source: hypr_transcribe_soniqo::TranscriptSource,
    samples: Vec<f32>,
    start: f64,
    duration: f64,
    myself: ActorRef<ListenerMsg>,
    session_offset_secs: f64,
    extra: Extra,
) -> Result<hypr_transcribe_soniqo::LiveTranscriptionSession, String> {
    let joined = tokio::task::spawn_blocking(move || {
        let mut session = session;
        let result = session.append(source, &samples);
        (session, result)
    })
    .await
    .map_err(|e| format!("soniqo_live_append_join_failed: {e}"))?;

    let (session, partials) = joined;
    let partials = partials.map_err(|e| format!("soniqo_live_append_failed: {e}"))?;

    for partial in partials {
        let mut response = partial.into_stream_response(model, start, duration);
        response.apply_offset(session_offset_secs);
        response.set_extra(&extra);

        let _ = myself.cast(ListenerMsg::StreamResponse(response));
    }

    Ok(session)
}

async fn finalize_soniqo_source(
    session: hypr_transcribe_soniqo::LiveTranscriptionSession,
    model: hypr_transcribe_soniqo::SoniqoModel,
    source: hypr_transcribe_soniqo::TranscriptSource,
    start: f64,
    myself: ActorRef<ListenerMsg>,
    session_offset_secs: f64,
    extra: Extra,
) -> Result<hypr_transcribe_soniqo::LiveTranscriptionSession, String> {
    let joined = tokio::task::spawn_blocking(move || {
        let mut session = session;
        let result = session.finalize(source);
        (session, result)
    })
    .await
    .map_err(|e| format!("soniqo_live_finalize_join_failed: {e}"))?;

    let (session, partials) = joined;
    let partials = partials.map_err(|e| format!("soniqo_live_finalize_failed: {e}"))?;

    for partial in partials {
        let mut response = partial.into_stream_response(model, start, 0.05);
        response.apply_offset(session_offset_secs);
        response.set_extra(&extra);

        let _ = myself.cast(ListenerMsg::StreamResponse(response));
    }

    Ok(session)
}

fn i16_bytes_to_f32(bytes: &Bytes) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / i16::MAX as f32)
        .collect()
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
            transcription_mode: crate::TranscriptionMode::Live,
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

    #[test]
    fn soniqo_model_for_args_accepts_loopback_base_url() {
        let args = listener_args("http://localhost:50060/v1", "soniqo-parakeet-streaming");

        assert_eq!(
            soniqo_model_for_args(&args).unwrap(),
            Some(hypr_transcribe_soniqo::SoniqoModel::ParakeetStreaming)
        );
    }

    #[test]
    fn soniqo_model_for_args_ignores_loopback_non_soniqo_model() {
        let args = listener_args("http://localhost:50060/v1", "cactus-whisper-small-int8");

        assert_eq!(soniqo_model_for_args(&args).unwrap(), None);
    }
}
