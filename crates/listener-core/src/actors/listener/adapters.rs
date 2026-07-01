use std::str::FromStr;
use std::time::{Duration, UNIX_EPOCH};

use bytes::Bytes;
use ractor::{ActorProcessingErr, ActorRef};

use owhisper_client::{
    AdapterKind, ArgmaxAdapter, AssemblyAIAdapter, CartesiaAdapter, DashScopeAdapter,
    DeepgramAdapter, ElevenLabsAdapter, FireworksAdapter, GladiaAdapter, HyprnoteAdapter,
    MistralAdapter, RealtimeSttAdapter, SonioxAdapter, hypr_ws_client,
};
use owhisper_interface::stream::Extra;
use owhisper_interface::{ControlMessage, MixedMessage};

use super::stream::process_stream;
use super::{ChannelSender, DEVICE_FINGERPRINT_HEADER, ListenerArgs, ListenerMsg, actor_error};
use crate::SessionErrorEvent;

const SONIQO_ECHO_GATE_MIN_SAMPLES: usize = 512;
const SONIQO_ECHO_GATE_MAX_LAG_SAMPLES: isize = 1600;
const SONIQO_ECHO_GATE_LAG_STEP_SAMPLES: isize = 80;
const SONIQO_ECHO_GATE_MIN_MIC_RMS: f32 = 0.0025;
const SONIQO_ECHO_GATE_MIN_SPEAKER_RMS: f32 = 0.01;
const SONIQO_ECHO_GATE_MIN_CORRELATION: f32 = 0.55;
const SONIQO_ECHO_GATE_MAX_RESIDUAL_RATIO: f32 = 0.45;

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

        if !model.supports_languages(&args.languages) {
            return Err(actor_error(format!(
                "unsupported_language: {} does not support all requested spoken languages ({})",
                model.as_str(),
                format_languages(&args.languages)
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
        Cartesia => CartesiaAdapter,
        Soniox => SonioxAdapter,
        Fireworks => FireworksAdapter,
        Deepgram => DeepgramAdapter,
        AssemblyAI => AssemblyAIAdapter,
        Gladia => GladiaAdapter,
        ElevenLabs => ElevenLabsAdapter,
        DashScope => DashScopeAdapter,
        Mistral => MistralAdapter,
        Hyprnote => HyprnoteAdapter,
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
    if matches!(args.mode, crate::actors::ChannelMode::MicAndSpeaker) {
        spawn_soniqo_rx_task_dual(model, args, myself).await
    } else {
        let source = if matches!(args.mode, crate::actors::ChannelMode::SpeakerOnly) {
            hypr_transcribe_soniqo::TranscriptSource::System
        } else {
            hypr_transcribe_soniqo::TranscriptSource::Microphone
        };

        spawn_soniqo_rx_task_single(model, args, myself, source).await
    }
}

async fn spawn_soniqo_rx_task_single(
    model: hypr_transcribe_soniqo::SoniqoModel,
    args: ListenerArgs,
    myself: ActorRef<ListenerMsg>,
    source: hypr_transcribe_soniqo::TranscriptSource,
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
    let (tx, mut rx) = tokio::sync::mpsc::channel::<MixedMessage<Bytes, ControlMessage>>(32);

    let session = start_soniqo_live_session(model).await?;

    let rx_task = tokio::spawn(async move {
        let mut session = session;
        let mut buffer = Vec::<f32>::new();
        let mut cursor = 0.0;
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
                        MixedMessage::Audio(audio) => {
                            buffer.extend(i16_bytes_to_f32(&audio));
                        }
                        MixedMessage::Control(ControlMessage::CloseStream) => {
                            break;
                        }
                        MixedMessage::Control(ControlMessage::Finalize | ControlMessage::KeepAlive) => {}
                    }
                }
                _ = interval.tick() => {
                    match flush_soniqo_buffer(
                        session,
                        model,
                        source,
                        &mut buffer,
                        &mut cursor,
                        &myself,
                        session_offset_secs,
                        &extra,
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

        match flush_soniqo_buffer(
            session,
            model,
            source,
            &mut buffer,
            &mut cursor,
            &myself,
            session_offset_secs,
            &extra,
        )
        .await
        {
            Ok(next_session) => session = next_session,
            Err(error) => {
                tracing::warn!(%error, "soniqo_final_flush_failed");
                return;
            }
        }

        match finalize_soniqo_source(
            session,
            model,
            source,
            cursor,
            myself,
            session_offset_secs,
            extra,
        )
        .await
        {
            Ok(next_session) => session = next_session,
            Err(error) => {
                tracing::warn!(%error, "soniqo_final_finalize_failed");
                return;
            }
        }

        let _ = tokio::task::spawn_blocking(move || session.stop()).await;
    });

    Ok((ChannelSender::Single(tx), rx_task, shutdown_tx))
}

async fn spawn_soniqo_rx_task_dual(
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
    let (tx, mut rx) =
        tokio::sync::mpsc::channel::<MixedMessage<(Bytes, Bytes), ControlMessage>>(32);

    let session = start_soniqo_live_session(model).await?;

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
                        MixedMessage::Audio((mic, spk)) => {
                            let mut mic_samples = i16_bytes_to_f32(&mic);
                            let spk_samples = i16_bytes_to_f32(&spk);
                            if suppress_soniqo_echo_dominant_mic(
                                &mut mic_samples,
                                &spk_samples,
                            ) {
                                tracing::debug!("soniqo_echo_dominant_mic_chunk_suppressed");
                            }
                            mic_buffer.extend(mic_samples);
                            spk_buffer.extend(spk_samples);
                        }
                        MixedMessage::Control(ControlMessage::CloseStream) => {
                            break;
                        }
                        MixedMessage::Control(ControlMessage::Finalize | ControlMessage::KeepAlive) => {}
                    }
                }
                _ = interval.tick() => {
                    match flush_soniqo_buffer(
                        session,
                        model,
                        hypr_transcribe_soniqo::TranscriptSource::Microphone,
                        &mut mic_buffer,
                        &mut mic_cursor,
                        &myself,
                        session_offset_secs,
                        &extra,
                    )
                    .await
                    {
                        Ok(next_session) => session = next_session,
                        Err(error) => {
                            let _ = myself.cast(ListenerMsg::StreamError(error));
                            return;
                        }
                    }

                    match flush_soniqo_buffer(
                        session,
                        model,
                        hypr_transcribe_soniqo::TranscriptSource::System,
                        &mut spk_buffer,
                        &mut spk_cursor,
                        &myself,
                        session_offset_secs,
                        &extra,
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

        match flush_soniqo_buffer(
            session,
            model,
            hypr_transcribe_soniqo::TranscriptSource::Microphone,
            &mut mic_buffer,
            &mut mic_cursor,
            &myself,
            session_offset_secs,
            &extra,
        )
        .await
        {
            Ok(next_session) => session = next_session,
            Err(error) => {
                tracing::warn!(%error, "soniqo_final_mic_flush_failed");
                return;
            }
        }

        match flush_soniqo_buffer(
            session,
            model,
            hypr_transcribe_soniqo::TranscriptSource::System,
            &mut spk_buffer,
            &mut spk_cursor,
            &myself,
            session_offset_secs,
            &extra,
        )
        .await
        {
            Ok(next_session) => session = next_session,
            Err(error) => {
                tracing::warn!(%error, "soniqo_final_system_flush_failed");
                return;
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

    Ok((ChannelSender::Dual(tx), rx_task, shutdown_tx))
}

async fn start_soniqo_live_session(
    model: hypr_transcribe_soniqo::SoniqoModel,
) -> Result<hypr_transcribe_soniqo::LiveTranscriptionSession, ActorProcessingErr> {
    tokio::task::spawn_blocking(move || {
        hypr_transcribe_soniqo::LiveTranscriptionSession::start(model)
    })
    .await
    .map_err(|e| actor_error(format!("soniqo_live_start_join_failed: {e}")))?
    .map_err(|e| actor_error(format!("soniqo_live_start_failed: {e}")))
}

async fn flush_soniqo_buffer(
    session: hypr_transcribe_soniqo::LiveTranscriptionSession,
    model: hypr_transcribe_soniqo::SoniqoModel,
    source: hypr_transcribe_soniqo::TranscriptSource,
    buffer: &mut Vec<f32>,
    cursor: &mut f64,
    myself: &ActorRef<ListenerMsg>,
    session_offset_secs: f64,
    extra: &Extra,
) -> Result<hypr_transcribe_soniqo::LiveTranscriptionSession, String> {
    if buffer.is_empty() {
        return Ok(session);
    }

    let samples = std::mem::take(buffer);
    let duration = samples.len() as f64 / super::super::SAMPLE_RATE as f64;
    let start = *cursor;
    *cursor += duration;

    flush_soniqo_source(
        session,
        model,
        source,
        samples,
        start,
        duration,
        myself.clone(),
        session_offset_secs,
        extra.clone(),
    )
    .await
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

fn suppress_soniqo_echo_dominant_mic(mic: &mut [f32], speaker: &[f32]) -> bool {
    let Some(score) = best_soniqo_echo_score(mic, speaker) else {
        return false;
    };

    if score.correlation < SONIQO_ECHO_GATE_MIN_CORRELATION
        || score.residual_ratio > SONIQO_ECHO_GATE_MAX_RESIDUAL_RATIO
        || score.mic_rms < SONIQO_ECHO_GATE_MIN_MIC_RMS
        || score.speaker_rms < SONIQO_ECHO_GATE_MIN_SPEAKER_RMS
    {
        return false;
    }

    mic.fill(0.0);
    true
}

#[derive(Debug, Clone, Copy)]
struct SoniqoEchoScore {
    correlation: f32,
    residual_ratio: f32,
    mic_rms: f32,
    speaker_rms: f32,
}

fn best_soniqo_echo_score(mic: &[f32], speaker: &[f32]) -> Option<SoniqoEchoScore> {
    let len = mic.len().min(speaker.len());
    if len < SONIQO_ECHO_GATE_MIN_SAMPLES {
        return None;
    }

    let max_lag =
        SONIQO_ECHO_GATE_MAX_LAG_SAMPLES.min((len - SONIQO_ECHO_GATE_MIN_SAMPLES) as isize);
    let mut best = soniqo_echo_score_at_lag(&mic[..len], &speaker[..len], 0);
    let mut lag = SONIQO_ECHO_GATE_LAG_STEP_SAMPLES;

    while lag <= max_lag {
        for lag in [-lag, lag] {
            if let Some(score) = soniqo_echo_score_at_lag(&mic[..len], &speaker[..len], lag) {
                if best
                    .map(|current| score.correlation > current.correlation)
                    .unwrap_or(true)
                {
                    best = Some(score);
                }
            }
        }
        lag += SONIQO_ECHO_GATE_LAG_STEP_SAMPLES;
    }

    best
}

fn soniqo_echo_score_at_lag(mic: &[f32], speaker: &[f32], lag: isize) -> Option<SoniqoEchoScore> {
    let len = mic.len().min(speaker.len());
    let (mic_start, speaker_start) = if lag >= 0 {
        (lag as usize, 0)
    } else {
        (0, lag.unsigned_abs())
    };
    let overlap = len.saturating_sub(mic_start.max(speaker_start));
    if overlap < SONIQO_ECHO_GATE_MIN_SAMPLES {
        return None;
    }

    let mut mic_energy = 0.0;
    let mut speaker_energy = 0.0;
    let mut cross_energy = 0.0;
    for idx in 0..overlap {
        let mic_sample = mic[mic_start + idx];
        let speaker_sample = speaker[speaker_start + idx];
        mic_energy += mic_sample * mic_sample;
        speaker_energy += speaker_sample * speaker_sample;
        cross_energy += mic_sample * speaker_sample;
    }

    if mic_energy <= f32::EPSILON || speaker_energy <= f32::EPSILON {
        return None;
    }

    let overlap_f32 = overlap as f32;
    let mic_rms = (mic_energy / overlap_f32).sqrt();
    let speaker_rms = (speaker_energy / overlap_f32).sqrt();
    let correlation = cross_energy.abs() / (mic_energy * speaker_energy).sqrt().max(1e-6);
    let residual_energy =
        (mic_energy - (cross_energy * cross_energy / speaker_energy.max(1e-6))).max(0.0);
    let residual_ratio = residual_energy.sqrt() / mic_energy.sqrt().max(1e-6);

    Some(SoniqoEchoScore {
        correlation,
        residual_ratio,
        mic_rms,
        speaker_rms,
    })
}

fn build_listen_params(args: &ListenerArgs) -> owhisper_interface::ListenParams {
    let redemption_time_ms = if args.onboarding { "60" } else { "400" };
    let custom_query = std::collections::HashMap::from([(
        "redemption_time_ms".to_string(),
        redemption_time_ms.to_string(),
    )]);
    let num_speakers = expected_speakers(args);

    owhisper_interface::ListenParams {
        model: Some(args.model.clone()),
        languages: args.languages.clone(),
        sample_rate: super::super::SAMPLE_RATE,
        keywords: args.keywords.clone(),
        num_speakers,
        max_speakers: num_speakers,
        custom_query: Some(custom_query),
        ..Default::default()
    }
}

fn expected_speakers(args: &ListenerArgs) -> Option<u32> {
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

fn format_languages(languages: &[hypr_language::Language]) -> String {
    if languages.is_empty() {
        return "none".to_string();
    }

    languages
        .iter()
        .map(hypr_language::Language::bcp47_code)
        .collect::<Vec<_>>()
        .join(", ")
}

fn build_extra(args: &ListenerArgs) -> (f64, Extra) {
    let session_offset_secs = args
        .stream_offset_secs
        .unwrap_or_else(|| args.session_started_at.elapsed().as_secs_f64());
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
            stream_offset_secs: None,
            session_id: "session".to_string(),
            participant_human_ids: vec![],
            self_human_id: None,
        }
    }

    fn test_signal(len: usize, seed: u32) -> Vec<f32> {
        let mut state = seed;
        (0..len)
            .map(|idx| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                let noise = (state as f32 / u32::MAX as f32) * 2.0 - 1.0;
                let pulse = if idx % 257 == 0 { 0.8 } else { 0.0 };
                0.45 * noise + pulse
            })
            .collect()
    }

    fn delayed(input: &[f32], delay_samples: usize, gain: f32) -> Vec<f32> {
        let mut out = vec![0.0; input.len()];
        for idx in delay_samples..input.len() {
            out[idx] = input[idx - delay_samples] * gain;
        }
        out
    }

    #[test]
    fn suppresses_soniqo_mic_when_chunk_is_echo_dominant() {
        let speaker = test_signal(1920, 0x1234_5678);
        let mut mic = delayed(&speaker, 160, 0.35);

        assert!(suppress_soniqo_echo_dominant_mic(&mut mic, &speaker));
        assert!(mic.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn suppresses_soniqo_mic_when_aec_residual_has_longer_capture_lag() {
        let speaker = test_signal(2400, 0x1234_5678);
        let mut mic = delayed(&speaker, 1120, 0.25);

        assert!(suppress_soniqo_echo_dominant_mic(&mut mic, &speaker));
        assert!(mic.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn suppresses_soniqo_mic_when_minimum_chunk_matches_zero_lag() {
        let speaker = test_signal(SONIQO_ECHO_GATE_MIN_SAMPLES, 0x1234_5678);
        let mut mic: Vec<f32> = speaker.iter().map(|sample| sample * 0.35).collect();

        assert!(suppress_soniqo_echo_dominant_mic(&mut mic, &speaker));
        assert!(mic.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn best_soniqo_echo_score_considers_zero_lag_when_step_grid_skips_it() {
        let speaker = test_signal(1024, 0x1234_5678);
        let mic: Vec<f32> = speaker.iter().map(|sample| sample * 0.35).collect();

        let score = best_soniqo_echo_score(&mic, &speaker).expect("echo score");

        assert!(score.correlation > 0.99);
    }

    #[test]
    fn keeps_soniqo_mic_when_independent_speech_dominates() {
        let speaker = test_signal(1920, 0x1234_5678);
        let local = test_signal(1920, 0x8765_4321);
        let mut mic = delayed(&speaker, 160, 0.15);
        for (mic_sample, local_sample) in mic.iter_mut().zip(local) {
            *mic_sample += 0.45 * local_sample;
        }
        let original = mic.clone();

        assert!(!suppress_soniqo_echo_dominant_mic(&mut mic, &speaker));
        assert_eq!(mic, original);
    }

    #[test]
    fn keeps_soniqo_mic_when_speaker_reference_is_quiet() {
        let speaker = vec![0.001; 1920];
        let mut mic = test_signal(1920, 0x1234_5678);
        let original = mic.clone();

        assert!(!suppress_soniqo_echo_dominant_mic(&mut mic, &speaker));
        assert_eq!(mic, original);
    }

    #[test]
    fn expected_speakers_counts_distinct_participants() {
        let mut args = listener_args("https://api.assemblyai.com", "u3-rt-pro");
        args.participant_human_ids = vec!["remote".to_string(), "self".to_string()];
        args.self_human_id = Some("self".to_string());

        assert_eq!(expected_speakers(&args), Some(2));
    }

    #[test]
    fn build_extra_prefers_explicit_stream_offset() {
        let mut args = listener_args("https://api.deepgram.com", "nova-3");
        args.stream_offset_secs = Some(12.5);

        let (offset_secs, _) = build_extra(&args);

        assert_eq!(offset_secs, 12.5);
    }

    #[test]
    fn build_listen_params_sets_num_speakers_without_assemblyai_custom_query() {
        let mut args = listener_args("https://api.assemblyai.com", "u3-rt-pro");
        args.participant_human_ids = vec!["remote".to_string()];
        args.self_human_id = Some("self".to_string());

        let params = build_listen_params(&args);
        let custom_query = params.custom_query.expect("custom query");

        assert_eq!(params.num_speakers, Some(2));
        assert_eq!(params.max_speakers, Some(2));
        assert!(!custom_query.contains_key("speaker_labels"));
        assert!(!custom_query.contains_key("max_speakers"));
    }

    #[test]
    fn build_listen_params_does_not_add_assemblyai_hints_for_other_providers() {
        let mut args = listener_args("https://api.deepgram.com/v1", "nova-3");
        args.participant_human_ids = vec!["remote".to_string()];
        args.self_human_id = Some("self".to_string());

        let params = build_listen_params(&args);
        let custom_query = params.custom_query.expect("custom query");

        assert_eq!(params.num_speakers, Some(2));
        assert_eq!(params.max_speakers, Some(2));
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
        let args = listener_args("http://localhost:50060/v1", "whisper-small");

        assert_eq!(soniqo_model_for_args(&args).unwrap(), None);
    }

    #[test]
    fn format_languages_uses_bcp47_codes() {
        let languages = vec!["en-US".parse().unwrap(), hypr_language::ISO639::Fr.into()];

        assert_eq!(format_languages(&languages), "en-US, fr");
    }
}
