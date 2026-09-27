use std::hash::{DefaultHasher, Hash, Hasher};

use futures_util::StreamExt;
use owhisper_client::FinalizeHandle;
use owhisper_interface::stream::{Extra, StreamResponse};
use ractor::{ActorRef, rpc::CallResult};

use super::{FINALIZE_STREAM_TIMEOUT, LISTEN_STREAM_TIMEOUT, ListenerMsg};

const PROVIDER_RESPONSE_ACK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);
const FINALIZE_HANDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);
const MAX_FINAL_RESPONSES: usize = 64;
const TRANSCRIPT_PROGRESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const MIN_ACTIVE_AUDIO_SAMPLES: usize = crate::actors::SAMPLE_RATE as usize * 5;
const MAX_UNFINALIZED_AUDIO_SAMPLES: usize = crate::actors::SAMPLE_RATE as usize * 90;

pub(super) struct StreamProgress {
    last_progress_at: std::time::Instant,
    last_response_at: std::time::Instant,
    last_partials_hash: Option<u64>,
    active_samples: usize,
    unfinalized_samples: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StallReason {
    NoProgress,
    UnfinalizedCap,
}

/// What tripped the watchdog. A provider that kept answering (`secs_since_response` near zero)
/// while `secs_since_progress` grew was alive but hearing nothing; a large `secs_since_response`
/// is a wedged stream.
#[derive(Clone, Copy, Debug)]
pub(super) struct StallDiagnostics {
    pub(super) reason: StallReason,
    pub(super) active_audio_secs: f64,
    pub(super) unfinalized_audio_secs: f64,
    pub(super) secs_since_progress: f64,
    pub(super) secs_since_response: f64,
}

impl StreamProgress {
    pub(super) fn new(now: std::time::Instant) -> Self {
        Self {
            last_progress_at: now,
            last_response_at: now,
            last_partials_hash: None,
            active_samples: 0,
            unfinalized_samples: 0,
        }
    }

    pub(super) fn observe_audio(&mut self, samples: usize, now: std::time::Instant) -> bool {
        self.active_samples = self.active_samples.saturating_add(samples);
        self.unfinalized_samples = self.unfinalized_samples.saturating_add(samples);
        self.no_progress(now) || self.unfinalized_samples >= MAX_UNFINALIZED_AUDIO_SAMPLES
    }

    /// Any provider message, including ones carrying no transcript progress.
    pub(super) fn observe_response(&mut self, now: std::time::Instant) {
        self.last_response_at = now;
    }

    pub(super) fn diagnostics(&self, now: std::time::Instant) -> StallDiagnostics {
        let sample_rate = crate::actors::SAMPLE_RATE as f64;
        StallDiagnostics {
            reason: if self.no_progress(now) {
                StallReason::NoProgress
            } else {
                StallReason::UnfinalizedCap
            },
            active_audio_secs: self.active_samples as f64 / sample_rate,
            unfinalized_audio_secs: self.unfinalized_samples as f64 / sample_rate,
            secs_since_progress: now.duration_since(self.last_progress_at).as_secs_f64(),
            secs_since_response: now.duration_since(self.last_response_at).as_secs_f64(),
        }
    }

    fn no_progress(&self, now: std::time::Instant) -> bool {
        self.active_samples >= MIN_ACTIVE_AUDIO_SAMPLES
            && now.duration_since(self.last_progress_at) >= TRANSCRIPT_PROGRESS_TIMEOUT
    }

    pub(super) fn observe_delta(
        &mut self,
        delta: &crate::LiveTranscriptDelta,
        now: std::time::Instant,
    ) {
        let finalized = delta
            .new_words
            .iter()
            .any(|word| !word.text.trim().is_empty());
        let mut partials_hash = None;
        let mut hasher = DefaultHasher::new();
        for word in delta
            .partials
            .iter()
            .filter(|word| !word.text.trim().is_empty())
        {
            (&word.text, word.start_ms, word.end_ms, word.channel).hash(&mut hasher);
            partials_hash = Some(hasher.finish());
        }
        if finalized || (partials_hash.is_some() && partials_hash != self.last_partials_hash) {
            self.last_progress_at = now;
            self.active_samples = 0;
        }
        self.last_partials_hash = partials_hash;
        if finalized {
            self.unfinalized_samples = 0;
        }
    }
}

pub(super) fn active_audio_samples(audio: &[u8]) -> usize {
    let samples = audio.len() / 2;
    if samples == 0 {
        return 0;
    }
    let energy = audio
        .chunks_exact(2)
        .map(|bytes| {
            let sample = f64::from(i16::from_le_bytes([bytes[0], bytes[1]])) / 32768.0;
            sample * sample
        })
        .sum::<f64>();

    // Quiet meetings must not reconnect just because there is no transcript.
    if energy / samples as f64 >= 0.005_f64.powi(2) {
        samples
    } else {
        0
    }
}

pub(super) async fn process_stream<S, E, H>(
    mut listen_stream: std::pin::Pin<&mut S>,
    handle: H,
    myself: ActorRef<ListenerMsg>,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    offset_secs: f64,
    extra: Extra,
) -> Vec<StreamResponse>
where
    S: futures_util::Stream<Item = Result<StreamResponse, E>>,
    E: std::fmt::Debug,
    H: FinalizeHandle,
{
    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                return collect_final_responses(
                    listen_stream.as_mut(),
                    &handle,
                    offset_secs,
                    &extra,
                    None,
                ).await;
            }
            result = tokio::time::timeout(LISTEN_STREAM_TIMEOUT, listen_stream.next()) => {
                match result {
                    Ok(Some(Ok(mut response))) => {
                        response.apply_offset(offset_secs);
                        response.set_extra(&extra);

                        let response_for_shutdown = response.clone();
                        let delivery = myself.call(
                            move |reply| ListenerMsg::StreamResponse(response, reply),
                            Some(PROVIDER_RESPONSE_ACK_TIMEOUT),
                        );
                        tokio::pin!(delivery);

                        tokio::select! {
                            biased;
                            result = &mut delivery => {
                                match result {
                                    Ok(CallResult::Success(())) => {}
                                    Ok(CallResult::Timeout) => {
                                        tracing::warn!("provider_response_ack_timed_out");
                                        myself.kill();
                                        return Vec::new();
                                    }
                                    Ok(CallResult::SenderError) | Err(_) => {
                                        tracing::warn!("actor_gone_breaking_stream_loop");
                                        return Vec::new();
                                    }
                                }
                            }
                            _ = &mut shutdown_rx => {
                                return collect_final_responses(
                                    listen_stream.as_mut(),
                                    &handle,
                                    offset_secs,
                                    &extra,
                                    Some(response_for_shutdown),
                                ).await;
                            }
                        }
                    }
                    Ok(Some(Err(error))) => {
                        let _ = myself.send_message(ListenerMsg::StreamError(format!("{error:?}")));
                        return Vec::new();
                    }
                    Ok(None) => {
                        let _ = myself.send_message(ListenerMsg::StreamEnded);
                        return Vec::new();
                    }
                    Err(elapsed) => {
                        let _ = myself.send_message(ListenerMsg::StreamTimeout(elapsed));
                        return Vec::new();
                    }
                }
            }
        }
    }
}

async fn collect_final_responses<S, E, H>(
    mut listen_stream: std::pin::Pin<&mut S>,
    handle: &H,
    offset_secs: f64,
    extra: &Extra,
    initial_response: Option<StreamResponse>,
) -> Vec<StreamResponse>
where
    S: futures_util::Stream<Item = Result<StreamResponse, E>>,
    E: std::fmt::Debug,
    H: FinalizeHandle,
{
    if tokio::time::timeout(FINALIZE_HANDLE_TIMEOUT, handle.finalize())
        .await
        .is_err()
    {
        tracing::warn!("provider_finalize_timed_out");
    }

    let expected_count = handle.expected_finalize_count();
    let mut responses = initial_response.into_iter().collect::<Vec<_>>();
    let mut finalize_count = responses
        .iter()
        .filter(|response| is_from_finalize(response))
        .count();

    if expected_count > 0 && finalize_count >= expected_count {
        return responses;
    }

    let finalize_timeout = tokio::time::sleep(FINALIZE_STREAM_TIMEOUT);
    tokio::pin!(finalize_timeout);

    loop {
        tokio::select! {
            _ = &mut finalize_timeout => {
                tracing::warn!(anarlog.timeout.reached = true, "break_timeout");
                break;
            }
            result = listen_stream.next() => {
                match result {
                    Some(Ok(mut response)) => {
                        if is_from_finalize(&response) {
                            finalize_count += 1;
                        }
                        response.apply_offset(offset_secs);
                        response.set_extra(extra);
                        responses.push(response);

                        if expected_count > 0 && finalize_count >= expected_count {
                            tracing::info!(finalize_count, expected_count, "break_from_finalize");
                            break;
                        }
                        if responses.len() >= MAX_FINAL_RESPONSES {
                            tracing::warn!(
                                collected = responses.len(),
                                expected_count,
                                "final_response_limit_reached"
                            );
                            break;
                        }
                    }
                    Some(Err(error)) => {
                        tracing::warn!(error.message = ?error, "break_from_finalize");
                        break;
                    }
                    None => {
                        tracing::info!(anarlog.stream.ended = true, "break_from_finalize");
                        break;
                    }
                }
            }
        }
    }

    responses
}

fn is_from_finalize(response: &StreamResponse) -> bool {
    matches!(
        response,
        StreamResponse::TranscriptResponse {
            from_finalize: true,
            ..
        }
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use ractor::{Actor, ActorProcessingErr, ActorRef};

    use super::*;
    use crate::actors::{ListenerAudioResult, ListenerConfigUpdate};

    struct ResponseProbe {
        delay: Duration,
        response_tx: tokio::sync::mpsc::UnboundedSender<()>,
    }

    struct StuckResponseProbe;

    #[ractor::async_trait]
    impl Actor for ResponseProbe {
        type Msg = ListenerMsg;
        type State = ();
        type Arguments = ();

        async fn pre_start(
            &self,
            _myself: ActorRef<Self::Msg>,
            _args: Self::Arguments,
        ) -> Result<Self::State, ActorProcessingErr> {
            Ok(())
        }

        async fn handle(
            &self,
            _myself: ActorRef<Self::Msg>,
            message: Self::Msg,
            _state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            match message {
                ListenerMsg::StreamResponse(_, reply) => {
                    tokio::time::sleep(self.delay).await;
                    let _ = self.response_tx.send(());
                    let _ = reply.send(());
                }
                ListenerMsg::AudioSingle(_, reply) | ListenerMsg::AudioDual(_, _, reply) => {
                    let _ = reply.send(ListenerAudioResult::Accepted);
                }
                ListenerMsg::UpdateConfig(ListenerConfigUpdate { .. })
                | ListenerMsg::StreamError(_)
                | ListenerMsg::StreamEnded
                | ListenerMsg::StreamTimeout(_) => {}
            }
            Ok(())
        }
    }

    #[ractor::async_trait]
    impl Actor for StuckResponseProbe {
        type Msg = ListenerMsg;
        type State = ();
        type Arguments = ();

        async fn pre_start(
            &self,
            _myself: ActorRef<Self::Msg>,
            _args: Self::Arguments,
        ) -> Result<Self::State, ActorProcessingErr> {
            Ok(())
        }

        async fn handle(
            &self,
            _myself: ActorRef<Self::Msg>,
            message: Self::Msg,
            _state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            match message {
                ListenerMsg::StreamResponse(_, reply) => {
                    let reply = reply;
                    std::future::pending::<()>().await;
                    drop(reply);
                }
                ListenerMsg::AudioSingle(_, reply) | ListenerMsg::AudioDual(_, _, reply) => {
                    let _ = reply.send(ListenerAudioResult::Accepted);
                }
                ListenerMsg::UpdateConfig(ListenerConfigUpdate { .. })
                | ListenerMsg::StreamError(_)
                | ListenerMsg::StreamEnded
                | ListenerMsg::StreamTimeout(_) => {}
            }
            Ok(())
        }
    }

    struct NoopFinalizeHandle;

    impl FinalizeHandle for NoopFinalizeHandle {
        async fn finalize(&self) {}

        fn expected_finalize_count(&self) -> usize {
            0
        }
    }

    struct StuckFinalizeHandle;

    impl FinalizeHandle for StuckFinalizeHandle {
        async fn finalize(&self) {
            std::future::pending::<()>().await;
        }

        fn expected_finalize_count(&self) -> usize {
            0
        }
    }

    struct ResponseFinalizeHandle {
        response_tx: Mutex<Option<tokio::sync::mpsc::UnboundedSender<Result<StreamResponse, ()>>>>,
    }

    impl FinalizeHandle for ResponseFinalizeHandle {
        async fn finalize(&self) {
            if let Some(response_tx) = self.response_tx.lock().unwrap().take() {
                let _ = response_tx.send(Ok(terminal_response()));
            }
        }

        fn expected_finalize_count(&self) -> usize {
            0
        }
    }

    fn terminal_response() -> StreamResponse {
        StreamResponse::TerminalResponse {
            request_id: "request".to_string(),
            created: "now".to_string(),
            duration: 0.0,
            channels: 1,
        }
    }

    fn extra() -> Extra {
        Extra {
            started_unix_millis: 0,
        }
    }

    fn transcript_response(is_final: bool) -> StreamResponse {
        use owhisper_interface::stream::{Alternatives, Channel, Metadata, Word};

        StreamResponse::TranscriptResponse {
            start: 0.0,
            duration: 1.0,
            is_final,
            speech_final: false,
            from_finalize: false,
            channel: Channel {
                alternatives: vec![Alternatives {
                    transcript: "hello world".to_string(),
                    words: ["hello", "world"]
                        .into_iter()
                        .enumerate()
                        .map(|(index, text)| Word {
                            word: text.to_string(),
                            start: index as f64 * 0.5,
                            end: (index + 1) as f64 * 0.5,
                            confidence: 1.0,
                            speaker: None,
                            punctuated_word: None,
                            language: None,
                        })
                        .collect(),
                    confidence: 1.0,
                    languages: vec![],
                }],
            },
            metadata: Metadata::default(),
            channel_index: vec![0, 1],
        }
    }

    fn transcript_delta(is_final: bool, second: i64) -> crate::LiveTranscriptDelta {
        let mut delta = crate::LiveTranscriptDelta {
            new_words: vec![],
            replaced_ids: vec![],
            partials: vec![],
        };
        if is_final {
            delta.new_words.push(anlg_transcript::FinalizedWord {
                id: second.to_string(),
                text: "hello".to_string(),
                start_ms: second * 1000,
                end_ms: second * 1000 + 500,
                channel: 0,
                speaker_index: None,
                state: anlg_transcript::WordState::Final,
            });
        } else {
            delta.partials.push(anlg_transcript::PartialWord {
                text: "hello".to_string(),
                start_ms: second * 1000,
                end_ms: second * 1000 + 500,
                channel: 0,
                speaker_index: None,
            });
        }
        delta
    }

    #[test]
    fn stalled_transcript_is_detected_while_audio_continues() {
        let now = Instant::now();
        let mut progress = StreamProgress::new(now);
        let audio = 1000_i16
            .to_le_bytes()
            .repeat(crate::actors::SAMPLE_RATE as usize);

        for second in 1..30 {
            assert!(!progress.observe_audio(
                active_audio_samples(&audio),
                now + Duration::from_secs(second),
            ));
        }
        assert!(
            progress.observe_audio(active_audio_samples(&audio), now + Duration::from_secs(30))
        );
    }

    #[test]
    fn silence_and_low_background_noise_do_not_trigger_reconnection() {
        let now = Instant::now();
        let mut progress = StreamProgress::new(now);
        for sample in [0_i16, 16, -16] {
            let audio = sample
                .to_le_bytes()
                .repeat(crate::actors::SAMPLE_RATE as usize);
            assert!(
                !progress
                    .observe_audio(active_audio_samples(&audio), now + Duration::from_secs(600))
            );
        }
        assert_eq!(active_audio_samples(&[]), 0);
        assert!(!progress.observe_audio(
            crate::actors::SAMPLE_RATE as usize,
            now + Duration::from_secs(601)
        ));
    }

    #[test]
    fn partial_transcripts_reset_the_stall_watchdog() {
        let now = Instant::now();
        let mut progress = StreamProgress::new(now);
        assert!(!progress.observe_audio(MIN_ACTIVE_AUDIO_SAMPLES, now + Duration::from_secs(29)));
        progress.observe_delta(&transcript_delta(false, 29), now + Duration::from_secs(29));
        assert!(!progress.observe_audio(MIN_ACTIVE_AUDIO_SAMPLES, now + Duration::from_secs(30)));
        assert!(progress.observe_audio(0, now + Duration::from_secs(59)));
    }

    #[test]
    fn stall_diagnostics_tell_a_silent_provider_from_a_wedged_stream() {
        let now = Instant::now();
        let sample_rate = crate::actors::SAMPLE_RATE as usize;

        // Empty provider messages keep the stream alive but are not transcript progress.
        let mut quiet = StreamProgress::new(now);
        quiet.observe_response(now + Duration::from_secs(29));
        assert!(quiet.observe_audio(MIN_ACTIVE_AUDIO_SAMPLES, now + Duration::from_secs(30)));
        let diagnostics = quiet.diagnostics(now + Duration::from_secs(30));
        assert_eq!(diagnostics.reason, StallReason::NoProgress);
        assert_eq!(diagnostics.secs_since_progress, 30.0);
        assert_eq!(diagnostics.secs_since_response, 1.0);
        assert_eq!(diagnostics.active_audio_secs, 5.0);

        // Recent partials rule out NoProgress, so the unfinalized cap is what tripped.
        let mut capped = StreamProgress::new(now);
        capped.observe_delta(&transcript_delta(false, 89), now + Duration::from_secs(89));
        assert!(capped.observe_audio(sample_rate * 90, now + Duration::from_secs(90)));
        let diagnostics = capped.diagnostics(now + Duration::from_secs(90));
        assert_eq!(diagnostics.reason, StallReason::UnfinalizedCap);
        assert_eq!(diagnostics.unfinalized_audio_secs, 90.0);
        assert_eq!(diagnostics.secs_since_response, 90.0);
    }

    #[test]
    fn empty_updates_do_not_hide_a_stalled_transcript() {
        let now = Instant::now();
        let mut progress = StreamProgress::new(now);
        assert!(!progress.observe_audio(MIN_ACTIVE_AUDIO_SAMPLES, now + Duration::from_secs(29)));
        progress.observe_delta(
            &crate::LiveTranscriptDelta {
                new_words: vec![],
                replaced_ids: vec![],
                partials: vec![],
            },
            now + Duration::from_secs(29),
        );
        assert!(progress.observe_audio(0, now + Duration::from_secs(30)));
    }

    #[test]
    fn partial_words_that_never_finalize_trigger_reconnection() {
        let now = Instant::now();
        let mut progress = StreamProgress::new(now);
        for second in 1..90 {
            let time = now + Duration::from_secs(second);
            progress.observe_delta(&transcript_delta(false, second as i64), time);
            assert!(!progress.observe_audio(crate::actors::SAMPLE_RATE as usize, time));
        }
        progress.observe_delta(&transcript_delta(false, 90), now + Duration::from_secs(90));
        assert!(progress.observe_audio(
            crate::actors::SAMPLE_RATE as usize,
            now + Duration::from_secs(90)
        ));
    }

    #[test]
    fn repeated_final_responses_do_not_hide_stalled_transcripts() {
        let now = Instant::now();
        let mut progress = StreamProgress::new(now);
        let mut engine = crate::LiveTranscriptEngine::new("anarlog", &[], None);
        let response = transcript_response(true);
        let initial = engine
            .process(&response)
            .expect("initial response should produce words");
        progress.observe_delta(&initial.transcript_delta, now);
        for second in 1..=30 {
            let time = now + Duration::from_secs(second);
            assert!(
                engine.process(&response).is_none(),
                "duplicate provider words must not count as app progress"
            );
            assert_eq!(
                progress.observe_audio(crate::actors::SAMPLE_RATE as usize, time),
                second == 30,
            );
        }
    }

    #[test]
    fn repeated_partials_do_not_hide_stalled_transcripts() {
        let now = Instant::now();
        let mut progress = StreamProgress::new(now);
        let delta = transcript_delta(false, 0);
        progress.observe_delta(&delta, now);
        for second in 1..=30 {
            let time = now + Duration::from_secs(second);
            progress.observe_delta(&delta, time);
            assert_eq!(
                progress.observe_audio(crate::actors::SAMPLE_RATE as usize, time),
                second == 30
            );
        }
    }

    #[test]
    fn advancing_single_word_finals_keep_an_active_stream_healthy() {
        let now = Instant::now();
        let mut progress = StreamProgress::new(now);
        let mut engine = crate::LiveTranscriptEngine::new("anarlog", &[], None);
        let mut response = transcript_response(true);
        if let StreamResponse::TranscriptResponse { channel, .. } = &mut response {
            channel.alternatives[0].words.truncate(1);
            channel.alternatives[0].transcript = "hello".to_string();
        }
        assert!(engine.process(&response).is_none());

        for second in 1..300 {
            let time = now + Duration::from_secs(second);
            if second % 20 == 0 {
                let mut next = response.clone();
                next.apply_offset(second as f64);
                let update = engine
                    .process(&next)
                    .expect("the next single-word final must release the held word");
                assert!(!update.transcript_delta.new_words.is_empty());
                progress.observe_delta(&update.transcript_delta, time);
            }
            assert!(!progress.observe_audio(crate::actors::SAMPLE_RATE as usize, time));
        }
    }

    #[test]
    fn finalized_words_keep_an_active_stream_healthy() {
        let now = Instant::now();
        let mut progress = StreamProgress::new(now);
        for second in 1..300 {
            let time = now + Duration::from_secs(second);
            progress.observe_delta(&transcript_delta(second % 20 == 0, second as i64), time);
            assert!(!progress.observe_audio(crate::actors::SAMPLE_RATE as usize, time));
        }
    }

    #[tokio::test]
    async fn provider_responses_wait_for_actor_acknowledgement() {
        let (response_tx, mut response_rx) = tokio::sync::mpsc::unbounded_channel();
        let (actor, actor_handle) = Actor::spawn(
            None,
            ResponseProbe {
                delay: Duration::from_millis(20),
                response_tx,
            },
            (),
        )
        .await
        .unwrap();
        let responses = futures_util::stream::iter(
            (0..5).map(|_| Ok::<StreamResponse, ()>(terminal_response())),
        );
        futures_util::pin_mut!(responses);
        let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let started_at = Instant::now();

        let final_responses = process_stream(
            responses,
            NoopFinalizeHandle,
            actor.clone(),
            shutdown_rx,
            0.0,
            extra(),
        )
        .await;

        assert!(final_responses.is_empty());
        assert!(started_at.elapsed() >= Duration::from_millis(100));
        for _ in 0..5 {
            response_rx.recv().await.unwrap();
        }

        actor.stop(None);
        let _ = actor_handle.await;
    }

    #[tokio::test]
    async fn provider_response_ack_timeout_terminates_stuck_actor() {
        let (actor, actor_handle) = Actor::spawn(None, StuckResponseProbe, ()).await.unwrap();
        let responses = futures_util::stream::iter([Ok::<StreamResponse, ()>(terminal_response())]);
        futures_util::pin_mut!(responses);
        let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        let final_responses = tokio::time::timeout(
            PROVIDER_RESPONSE_ACK_TIMEOUT + Duration::from_secs(1),
            process_stream(
                responses,
                NoopFinalizeHandle,
                actor,
                shutdown_rx,
                0.0,
                extra(),
            ),
        )
        .await
        .expect("stuck provider response should reach its acknowledgement timeout");

        assert!(final_responses.is_empty());
        tokio::time::timeout(Duration::from_secs(1), actor_handle)
            .await
            .expect("stuck listener should be terminated after the response timeout")
            .unwrap();
    }

    #[tokio::test]
    async fn finalize_responses_are_returned_without_reentering_stopped_actor() {
        let (probe_tx, mut probe_rx) = tokio::sync::mpsc::unbounded_channel();
        let (actor, actor_handle) = Actor::spawn(
            None,
            ResponseProbe {
                delay: Duration::ZERO,
                response_tx: probe_tx,
            },
            (),
        )
        .await
        .unwrap();
        let (response_tx, response_rx) = tokio::sync::mpsc::unbounded_channel();
        let responses = tokio_stream::wrappers::UnboundedReceiverStream::new(response_rx);
        futures_util::pin_mut!(responses);
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        shutdown_tx.send(()).unwrap();

        let final_responses = process_stream(
            responses,
            ResponseFinalizeHandle {
                response_tx: Mutex::new(Some(response_tx)),
            },
            actor.clone(),
            shutdown_rx,
            0.0,
            extra(),
        )
        .await;

        assert_eq!(final_responses, vec![terminal_response()]);
        assert!(
            tokio::time::timeout(Duration::from_millis(50), probe_rx.recv())
                .await
                .is_err()
        );

        actor.stop(None);
        let _ = actor_handle.await;
    }

    #[tokio::test]
    async fn hung_provider_finalize_has_a_deadline() {
        let responses = futures_util::stream::empty::<Result<StreamResponse, ()>>();
        futures_util::pin_mut!(responses);
        let started_at = Instant::now();

        let final_responses = tokio::time::timeout(
            FINALIZE_HANDLE_TIMEOUT + Duration::from_secs(1),
            collect_final_responses(responses, &StuckFinalizeHandle, 0.0, &extra(), None),
        )
        .await
        .expect("provider finalize should be cancelled at its deadline");

        assert!(final_responses.is_empty());
        assert!(started_at.elapsed() >= FINALIZE_HANDLE_TIMEOUT);
    }

    #[tokio::test]
    async fn final_response_collection_is_bounded() {
        let responses = futures_util::stream::iter(
            (0..MAX_FINAL_RESPONSES + 10).map(|_| Ok::<StreamResponse, ()>(terminal_response())),
        );
        futures_util::pin_mut!(responses);

        let final_responses =
            collect_final_responses(responses, &NoopFinalizeHandle, 0.0, &extra(), None).await;

        assert_eq!(final_responses.len(), MAX_FINAL_RESPONSES);
    }
}
