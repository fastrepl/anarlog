mod adapters;
mod stream;

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use bytes::Bytes;
use ractor::{Actor, ActorName, ActorProcessingErr, ActorRef, RpcReplyPort, SupervisionEvent};
use tokio::time::error::Elapsed;
use tracing::Instrument;

use anlg_transcript::IdentityAssignment;
use owhisper_interface::stream::StreamResponse;
use owhisper_interface::{ControlMessage, MixedMessage};

use super::session::{LiveConfirmed, session_span};
use crate::{
    DegradedError, ListenerRuntime, LiveTranscriptEngine, SessionDataEvent, SessionErrorEvent,
    SessionProgressEvent,
};

use adapters::spawn_rx_task;

pub(super) const LISTEN_STREAM_TIMEOUT: Duration = Duration::from_secs(15 * 60);
pub(super) const FINALIZE_STREAM_TIMEOUT: Duration = Duration::from_secs(5);
const STREAM_TASK_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(7);
pub(super) const DEVICE_FINGERPRINT_HEADER: &str = "x-device-fingerprint";

pub enum ListenerMsg {
    AudioSingle(Bytes, RpcReplyPort<ListenerAudioResult>),
    AudioDual(Bytes, Bytes, RpcReplyPort<ListenerAudioResult>),
    UpdateConfig(ListenerConfigUpdate),
    StreamResponse(StreamResponse, RpcReplyPort<()>),
    StreamError(String),
    StreamEnded,
    StreamTimeout(Elapsed),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListenerAudioResult {
    Accepted,
    Backpressured,
    Closed,
    ModeMismatch,
}

#[derive(Clone)]
pub struct ListenerConfigUpdate {
    pub languages: Vec<anlg_language::Language>,
    pub participant_human_ids: Vec<String>,
    pub self_human_id: Option<String>,
    pub speaker_assignments: Vec<IdentityAssignment>,
}

#[derive(Clone)]
pub struct ListenerArgs {
    pub runtime: Arc<dyn ListenerRuntime>,
    pub languages: Vec<anlg_language::Language>,
    pub onboarding: bool,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub keywords: Vec<String>,
    pub transcription_mode: crate::TranscriptionMode,
    pub mode: crate::actors::ChannelMode,
    pub session_started_at: Instant,
    pub session_started_at_unix: SystemTime,
    pub stream_offset_secs: Option<f64>,
    pub session_id: String,
    pub participant_human_ids: Vec<String>,
    pub self_human_id: Option<String>,
    pub speaker_assignments: Vec<IdentityAssignment>,
    pub live_confirmed: Arc<LiveConfirmed>,
}

pub struct ListenerState {
    pub args: ListenerArgs,
    transcript: LiveTranscriptEngine,
    tx: ChannelSender,
    rx_task: tokio::task::JoinHandle<Vec<StreamResponse>>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    progress: Option<stream::StreamProgress>,
}

pub(super) enum ChannelSender {
    Single(tokio::sync::mpsc::Sender<MixedMessage<Bytes, ControlMessage>>),
    Dual(tokio::sync::mpsc::Sender<MixedMessage<(Bytes, Bytes), ControlMessage>>),
}

pub struct ListenerActor;

impl ListenerActor {
    pub fn name(session_id: &str) -> ActorName {
        format!("listener_actor_{session_id}")
    }
}

#[derive(Debug)]
pub(super) struct ListenerInitError {
    pub(super) message: String,
    pub(super) degraded: Option<DegradedError>,
    pub(super) retry_after: Option<Duration>,
}

impl std::fmt::Display for ListenerInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ListenerInitError {}

pub(super) fn actor_error(msg: impl Into<String>) -> ActorProcessingErr {
    Box::new(ListenerInitError {
        message: msg.into(),
        degraded: None,
        retry_after: None,
    })
}

pub(super) fn actor_error_with_degraded(
    msg: impl Into<String>,
    degraded: DegradedError,
) -> ActorProcessingErr {
    actor_error_with_degraded_retry(msg, degraded, None)
}

pub(super) fn actor_error_with_degraded_retry(
    msg: impl Into<String>,
    degraded: DegradedError,
    retry_after: Option<Duration>,
) -> ActorProcessingErr {
    Box::new(ListenerInitError {
        message: msg.into(),
        degraded: Some(degraded),
        retry_after,
    })
}

#[ractor::async_trait]
impl Actor for ListenerActor {
    type Msg = ListenerMsg;
    type State = ListenerState;
    type Arguments = ListenerArgs;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let session_id = args.session_id.clone();
        let span = session_span(&session_id);

        async {
            args.runtime
                .emit_progress(SessionProgressEvent::Connecting {
                    session_id: session_id.clone(),
                });

            let (tx, rx_task, shutdown_tx, adapter_name) =
                spawn_rx_task(args.clone(), myself).await?;

            args.runtime.emit_progress(SessionProgressEvent::Connected {
                session_id: session_id.clone(),
                adapter: adapter_name.clone(),
            });

            let transcript = resume_transcript(
                LiveTranscriptEngine::with_speaker_assignments(
                    &adapter_name,
                    &args.participant_human_ids,
                    args.self_human_id.as_deref(),
                    args.speaker_assignments.clone(),
                ),
                &args.live_confirmed,
            );

            let state = ListenerState {
                args,
                transcript,
                tx,
                rx_task,
                shutdown_tx: Some(shutdown_tx),
                progress: (!matches!(adapter_name.as_str(), "soniqo" | "apple-speech"))
                    .then(|| stream::StreamProgress::new(Instant::now())),
            };

            Ok(state)
        }
        .instrument(span)
        .await
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        let final_responses = stop_stream_task(
            &mut state.shutdown_tx,
            &mut state.rx_task,
            STREAM_TASK_SHUTDOWN_TIMEOUT,
        )
        .await;

        for response in final_responses {
            let _ = process_stream_response(state, response);
        }

        if let Some(update) = state.transcript.flush() {
            note_confirmed_words(&state.args, &update.transcript_delta);
            if !update.transcript_delta.is_empty() {
                state
                    .args
                    .runtime
                    .emit_data(SessionDataEvent::TranscriptDelta {
                        session_id: state.args.session_id.clone(),
                        delta: Box::new(update.transcript_delta),
                    });
            }
            if let Some(segment_delta) = update.segment_delta {
                state
                    .args
                    .runtime
                    .emit_data(SessionDataEvent::TranscriptSegmentDelta {
                        session_id: state.args.session_id.clone(),
                        delta: Box::new(segment_delta),
                    });
            }
        }

        Ok(())
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        let span = session_span(&state.args.session_id);
        let _guard = span.enter();

        match message {
            ListenerMsg::AudioSingle(audio, reply) => {
                let active_samples = stream::active_audio_samples(&audio);
                let result = match &state.tx {
                    ChannelSender::Single(tx) => match tx.try_send(MixedMessage::Audio(audio)) {
                        Ok(()) => ListenerAudioResult::Accepted,
                        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                            ListenerAudioResult::Backpressured
                        }
                        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                            ListenerAudioResult::Closed
                        }
                    },
                    ChannelSender::Dual(_) => ListenerAudioResult::ModeMismatch,
                };
                let _ = reply.send(result);
                if result == ListenerAudioResult::Accepted
                    && state.progress.as_mut().is_some_and(|progress| {
                        progress.observe_audio(active_samples, Instant::now())
                    })
                {
                    tracing::warn!("listen_stream_stalled_during_audio");
                    stop_with_degraded_error(&myself, DegradedError::ConnectionTimeout);
                }
                if matches!(
                    result,
                    ListenerAudioResult::Closed | ListenerAudioResult::ModeMismatch
                ) {
                    stop_with_degraded_error(
                        &myself,
                        DegradedError::StreamError {
                            message: "listener audio channel unavailable".to_string(),
                        },
                    );
                }
            }

            ListenerMsg::AudioDual(mic, spk, reply) => {
                let active_samples =
                    stream::active_audio_samples(&mic).max(stream::active_audio_samples(&spk));
                let result = match &state.tx {
                    ChannelSender::Dual(tx) => match tx.try_send(MixedMessage::Audio((mic, spk))) {
                        Ok(()) => ListenerAudioResult::Accepted,
                        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                            ListenerAudioResult::Backpressured
                        }
                        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                            ListenerAudioResult::Closed
                        }
                    },
                    ChannelSender::Single(_) => ListenerAudioResult::ModeMismatch,
                };
                let _ = reply.send(result);
                if result == ListenerAudioResult::Accepted
                    && state.progress.as_mut().is_some_and(|progress| {
                        progress.observe_audio(active_samples, Instant::now())
                    })
                {
                    tracing::warn!("listen_stream_stalled_during_audio");
                    stop_with_degraded_error(&myself, DegradedError::ConnectionTimeout);
                }
                if matches!(
                    result,
                    ListenerAudioResult::Closed | ListenerAudioResult::ModeMismatch
                ) {
                    stop_with_degraded_error(
                        &myself,
                        DegradedError::StreamError {
                            message: "listener audio channel unavailable".to_string(),
                        },
                    );
                }
            }

            ListenerMsg::UpdateConfig(update) => {
                state.args.languages = update.languages;
                state.args.participant_human_ids = update.participant_human_ids;
                state.args.self_human_id = update.self_human_id;
                state.args.speaker_assignments = update.speaker_assignments;
                if let Some(segment_delta) = state.transcript.update_identities(
                    &state.args.participant_human_ids,
                    state.args.self_human_id.as_deref(),
                    state.args.speaker_assignments.clone(),
                ) {
                    state
                        .args
                        .runtime
                        .emit_data(SessionDataEvent::TranscriptSegmentDelta {
                            session_id: state.args.session_id.clone(),
                            delta: Box::new(segment_delta),
                        });
                }
            }

            ListenerMsg::StreamResponse(response, reply) => {
                let degraded = process_stream_response(state, response);
                let _ = reply.send(());
                if let Some(degraded) = degraded {
                    stop_with_degraded_error(&myself, degraded);
                }
            }

            ListenerMsg::StreamError(error) => {
                tracing::info!("listen_stream_error: {}", error);
                stop_with_degraded_error(&myself, DegradedError::StreamError { message: error });
            }

            ListenerMsg::StreamEnded => {
                tracing::info!("listen_stream_ended");
                stop_with_degraded_error(
                    &myself,
                    DegradedError::UpstreamUnavailable {
                        message: "stream ended".to_string(),
                    },
                );
            }

            ListenerMsg::StreamTimeout(elapsed) => {
                tracing::info!("listen_stream_timeout: {}", elapsed);
                stop_with_degraded_error(&myself, DegradedError::ConnectionTimeout);
            }
        }
        Ok(())
    }

    async fn handle_supervisor_evt(
        &self,
        myself: ActorRef<Self::Msg>,
        message: SupervisionEvent,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        let span = session_span(&state.args.session_id);
        let _guard = span.enter();
        tracing::info!("supervisor_event: {:?}", message);

        match message {
            SupervisionEvent::ActorStarted(_) | SupervisionEvent::ProcessGroupChanged(_) => {}
            SupervisionEvent::ActorTerminated(_, _, _) => {}
            SupervisionEvent::ActorFailed(_cell, _) => {
                myself.stop(None);
            }
        }
        Ok(())
    }
}

async fn stop_stream_task(
    shutdown_tx: &mut Option<tokio::sync::oneshot::Sender<()>>,
    rx_task: &mut tokio::task::JoinHandle<Vec<StreamResponse>>,
    timeout: Duration,
) -> Vec<StreamResponse> {
    let Some(shutdown_tx) = shutdown_tx.take() else {
        return Vec::new();
    };

    let _ = shutdown_tx.send(());
    match tokio::time::timeout(timeout, &mut *rx_task).await {
        Ok(Ok(responses)) => discard_final_stream_errors(responses),
        Ok(Err(error)) => {
            tracing::warn!(error.message = ?error, "listener_stream_task_join_failed");
            Vec::new()
        }
        Err(_) => {
            tracing::warn!("listener_stream_task_shutdown_timed_out");
            rx_task.abort();
            Vec::new()
        }
    }
}

fn discard_final_stream_errors(responses: Vec<StreamResponse>) -> Vec<StreamResponse> {
    responses
        .into_iter()
        .filter_map(|response| match response {
            StreamResponse::ErrorResponse {
                error_code,
                provider,
                ..
            } => {
                let error_code =
                    error_code.map_or_else(|| "unknown".to_string(), |code| code.to_string());
                tracing::warn!(
                    error.code = %error_code,
                    anarlog.stt.provider.name = %provider,
                    "stream_provider_error_during_shutdown"
                );
                None
            }
            response => Some(response),
        })
        .collect()
}

fn process_stream_response(
    state: &mut ListenerState,
    mut response: StreamResponse,
) -> Option<DegradedError> {
    if let StreamResponse::ErrorResponse {
        error_code,
        error_message,
        provider,
    } = &response
    {
        let error_code_tag =
            error_code.map_or_else(|| "unknown".to_string(), |code| code.to_string());
        tracing::error!(
            error.code = %error_code_tag,
            error.message = %error_message,
            anarlog.stt.provider.name = %provider,
            "stream_provider_error"
        );
        state
            .args
            .runtime
            .emit_error(SessionErrorEvent::ConnectionError {
                session_id: state.args.session_id.clone(),
                error: format!(
                    "[{}] {} (code: {})",
                    provider,
                    error_message,
                    error_code
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "none".to_string())
                ),
            });
        return Some(classify_provider_error(
            *error_code,
            error_message,
            provider,
        ));
    }

    match state.args.mode {
        crate::actors::ChannelMode::MicOnly => response.remap_channel_index(0, 2),
        crate::actors::ChannelMode::SpeakerOnly => response.remap_channel_index(1, 2),
        crate::actors::ChannelMode::MicAndSpeaker => {}
    }

    if let Some(update) = state.transcript.process(&response) {
        if let Some(progress) = &mut state.progress {
            progress.observe_delta(&update.transcript_delta, Instant::now());
        }
        note_confirmed_words(&state.args, &update.transcript_delta);
        state
            .args
            .runtime
            .emit_data(SessionDataEvent::TranscriptDelta {
                session_id: state.args.session_id.clone(),
                delta: Box::new(update.transcript_delta),
            });
        if let Some(segment_delta) = update.segment_delta {
            state
                .args
                .runtime
                .emit_data(SessionDataEvent::TranscriptSegmentDelta {
                    session_id: state.args.session_id.clone(),
                    delta: Box::new(segment_delta),
                });
        }
    }

    None
}

fn note_confirmed_words(args: &ListenerArgs, delta: &crate::LiveTranscriptDelta) {
    for word in &delta.new_words {
        args.live_confirmed
            .note(word.channel, word.end_ms.max(0) as u64);
    }
}

// A reconnect replays the last few seconds of audio; words the previous
// stream already finalized in that window must not be emitted again.
fn resume_transcript(
    engine: LiveTranscriptEngine,
    live_confirmed: &LiveConfirmed,
) -> LiveTranscriptEngine {
    live_confirmed
        .channels()
        .fold(engine, |engine: LiveTranscriptEngine, (channel, end_ms)| {
            engine.with_confirmed_through(channel, end_ms)
        })
}

fn classify_provider_error(
    error_code: Option<i32>,
    error_message: &str,
    provider: &str,
) -> DegradedError {
    match error_code {
        Some(401) | Some(403) => DegradedError::AuthenticationFailed {
            provider: provider.to_string(),
        },
        Some(400) | Some(402) | Some(404) | Some(422) => DegradedError::ProviderConfiguration {
            provider: provider.to_string(),
            message: error_message.to_string(),
        },
        _ => DegradedError::StreamError {
            message: format!("{provider}: {error_message}"),
        },
    }
}

fn stop_with_degraded_error(myself: &ActorRef<ListenerMsg>, error: DegradedError) {
    let reason = serde_json::to_string(&error).ok();
    myself.stop(reason);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn final_response(channel: i32, text: &str, start_ms: u64, end_ms: u64) -> StreamResponse {
        let start = start_ms as f64 / 1000.0;
        let end = end_ms as f64 / 1000.0;
        serde_json::from_value(serde_json::json!({
            "type": "Results", "start": start, "duration": end - start,
            "is_final": true, "speech_final": true, "from_finalize": false,
            "channel_index": [channel, 2],
            "channel": {"alternatives": [{"transcript": text, "confidence": 1.0,
                "words": [{"word": format!(" {text}"), "start": start, "end": end, "confidence": 1.0, "speaker": 0}]}]},
            "metadata": {"request_id": "test", "model_uuid": "test",
                "model_info": {"name": "test", "version": "1", "arch": "test"}}
        }))
        .unwrap()
    }

    fn engine(live_confirmed: &LiveConfirmed) -> LiveTranscriptEngine {
        resume_transcript(
            LiveTranscriptEngine::with_speaker_assignments("deepgram", &[], None, vec![]),
            live_confirmed,
        )
    }

    type Emitted = Vec<(i32, String, i64, i64)>;

    fn collect(
        live_confirmed: &LiveConfirmed,
        update: Option<crate::LiveTranscriptUpdate>,
    ) -> Emitted {
        let Some(update) = update else {
            return vec![];
        };
        for word in &update.transcript_delta.new_words {
            live_confirmed.note(word.channel, word.end_ms.max(0) as u64);
        }
        update
            .transcript_delta
            .new_words
            .iter()
            .map(|w| (w.channel, w.text.trim().to_string(), w.start_ms, w.end_ms))
            .collect()
    }

    fn finalize(
        engine: &mut LiveTranscriptEngine,
        live_confirmed: &LiveConfirmed,
        response: &StreamResponse,
    ) -> Emitted {
        collect(live_confirmed, engine.process(response))
    }

    // The old listener flushes its held word when it stops; the new one
    // starts from whatever the old one confirmed.
    fn reconnect(
        old: &mut LiveTranscriptEngine,
        live_confirmed: &LiveConfirmed,
    ) -> (Emitted, LiveTranscriptEngine) {
        let flushed = collect(live_confirmed, old.flush());
        (flushed, engine(live_confirmed))
    }

    #[test]
    fn reconnect_replay_does_not_finalize_words_twice() {
        let live_confirmed = LiveConfirmed::default();

        let mut first = engine(&live_confirmed);
        finalize(
            &mut first,
            &live_confirmed,
            &final_response(0, "hello", 0, 1000),
        );
        let (flushed, mut second) = reconnect(&mut first, &live_confirmed);
        assert_eq!(flushed, vec![(0, "hello".to_string(), 0, 1000)]);

        let replayed = finalize(
            &mut second,
            &live_confirmed,
            &final_response(0, "hello", 0, 1000),
        );
        assert!(
            replayed.is_empty(),
            "replayed word was emitted again: {replayed:?}"
        );

        finalize(
            &mut second,
            &live_confirmed,
            &final_response(0, "world", 1000, 1500),
        );
        finalize(
            &mut second,
            &live_confirmed,
            &final_response(1, "reply", 200, 900),
        );
        let rest = collect(&live_confirmed, second.flush());
        assert_eq!(
            rest,
            vec![
                (0, "world".to_string(), 1000, 1500),
                (1, "reply".to_string(), 200, 900),
            ]
        );
    }

    #[test]
    fn one_hour_with_a_reconnect_every_fifteen_minutes_yields_each_word_once() {
        const WORD_MS: u64 = 1000;
        const SPOKEN_MS: u64 = 400;
        const RECONNECT_EVERY_MS: u64 = 15 * 60 * 1000;
        const REPLAY_MS: u64 = 5000;
        const HOUR_MS: u64 = 60 * 60 * 1000;

        let live_confirmed = LiveConfirmed::default();
        let mut engine = engine(&live_confirmed);
        let mut emitted = Vec::new();
        let mut at = 0;
        while at < HOUR_MS {
            if at > 0 && at % RECONNECT_EVERY_MS == 0 {
                let (flushed, next) = reconnect(&mut engine, &live_confirmed);
                emitted.extend(flushed);
                engine = next;
                let mut replay = at - REPLAY_MS;
                while replay < at {
                    for channel in 0..2 {
                        let response = final_response(
                            channel,
                            &format!("w{replay}"),
                            replay,
                            replay + SPOKEN_MS,
                        );
                        emitted.extend(finalize(&mut engine, &live_confirmed, &response));
                    }
                    replay += WORD_MS;
                }
            }
            for channel in 0..2 {
                let response = final_response(channel, &format!("w{at}"), at, at + SPOKEN_MS);
                emitted.extend(finalize(&mut engine, &live_confirmed, &response));
            }
            at += WORD_MS;
        }
        emitted.extend(collect(&live_confirmed, engine.flush()));

        let expected = (HOUR_MS / WORD_MS) as usize * 2;
        assert_eq!(emitted.len(), expected);
        let mut unique = emitted.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            unique.len(),
            expected,
            "duplicate words after reconnect replay"
        );
    }

    #[tokio::test]
    async fn stream_task_shutdown_aborts_a_hung_task_after_the_deadline() {
        let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();
        let mut shutdown_tx = Some(shutdown_tx);
        let mut task = tokio::spawn(std::future::pending::<Vec<StreamResponse>>());

        let responses =
            stop_stream_task(&mut shutdown_tx, &mut task, Duration::from_millis(20)).await;

        assert!(responses.is_empty());
        assert!(shutdown_tx.is_none());
        let join_result = tokio::time::timeout(Duration::from_secs(1), &mut task)
            .await
            .expect("aborted stream task should stop promptly");
        assert!(join_result.unwrap_err().is_cancelled());
    }

    #[tokio::test]
    async fn stream_task_shutdown_discards_queued_provider_errors() {
        let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();
        let mut shutdown_tx = Some(shutdown_tx);
        let mut task = tokio::spawn(async {
            vec![StreamResponse::ErrorResponse {
                error_code: Some(400),
                error_message: "invalid request".to_string(),
                provider: "openai".to_string(),
            }]
        });

        let responses = stop_stream_task(&mut shutdown_tx, &mut task, Duration::from_secs(1)).await;

        assert!(responses.is_empty());
    }

    #[test]
    fn permanent_provider_errors_do_not_use_the_retryable_stream_error() {
        assert!(matches!(
            classify_provider_error(Some(401), "invalid key", "openai"),
            DegradedError::AuthenticationFailed { .. }
        ));
        assert!(matches!(
            classify_provider_error(Some(400), "invalid model", "openai"),
            DegradedError::ProviderConfiguration { .. }
        ));
        assert!(matches!(
            classify_provider_error(Some(402), "quota exhausted", "openai"),
            DegradedError::ProviderConfiguration { .. }
        ));
        assert!(matches!(
            classify_provider_error(Some(429), "rate limited", "openai"),
            DegradedError::StreamError { .. }
        ));
        assert!(matches!(
            classify_provider_error(Some(500), "server error", "openai"),
            DegradedError::StreamError { .. }
        ));
    }
}
