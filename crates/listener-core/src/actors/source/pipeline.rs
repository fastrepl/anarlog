use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use ractor::{ActorRef, rpc::CallResult};

use crate::{
    ListenerRuntime, SessionDataEvent,
    actors::{
        ChannelMode, ListenerAudioResult, ListenerMsg, RecMsg, RecorderEnqueueResult, SAMPLE_RATE,
    },
};
use anlg_audio_utils::f32_to_i16_bytes;
use anlg_vad_masking::VadMask;

use super::{ListenerRefreshReplay, ListenerRouting, SourceFrame};

const AUDIO_AMPLITUDE_THROTTLE: Duration = Duration::from_millis(100);
const MAX_BUFFER_CHUNKS: usize = 150;
const REPLAY_HISTORY_SECS: usize = 5;
const LISTENER_DISPATCH_CAPACITY: usize = 32;
const LISTENER_AUDIO_ACK_TIMEOUT: Duration = Duration::from_secs(1);
const LISTENER_BACKPRESSURE_TIMEOUT: Duration = Duration::from_secs(1);
const LISTENER_BACKPRESSURE_RETRY_DELAY: Duration = Duration::from_millis(10);
const MAX_BACKLOG_DISPATCH_PER_FRAME: usize = 2;
const RECORDER_BACKPRESSURE_TIMEOUT: Duration = Duration::from_secs(2);
const RECORDER_RPC_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Clone)]
struct BufferedAudio {
    mic: Arc<[f32]>,
    spk: Arc<[f32]>,
    mode: ChannelMode,
}

impl BufferedAudio {
    fn new(mic: Arc<[f32]>, spk: Arc<[f32]>, mode: ChannelMode) -> Self {
        Self { mic, spk, mode }
    }

    fn sample_count(&self) -> usize {
        self.mic.len().max(self.spk.len())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DispatchFrameResult {
    Complete,
    RecorderBackpressured,
}

pub(in crate::actors) struct Pipeline {
    vad_mask: VadMask,
    amplitude: AmplitudeEmitter,
    audio_buffer: AudioBuffer,
    replay_history: ReplayHistory,
    listener_dispatcher: ListenerDispatcher,
    pending_recorder_item: Option<BufferedAudio>,
    recorder_backpressure_started_at: Option<Instant>,
}

impl Pipeline {
    pub(super) fn new(runtime: Arc<dyn ListenerRuntime>, session_id: String) -> Self {
        Self {
            amplitude: AmplitudeEmitter::new(runtime, session_id),
            audio_buffer: AudioBuffer::new(MAX_BUFFER_CHUNKS),
            replay_history: ReplayHistory::new(SAMPLE_RATE as usize * REPLAY_HISTORY_SECS),
            listener_dispatcher: ListenerDispatcher::new(),
            pending_recorder_item: None,
            recorder_backpressure_started_at: None,
            vad_mask: VadMask::default(),
        }
    }

    pub(super) fn reset(&mut self) {
        self.amplitude.reset();
        self.audio_buffer.clear();
        self.replay_history.clear();
        self.listener_dispatcher.reset();
        self.pending_recorder_item = None;
        self.recorder_backpressure_started_at = None;
        self.vad_mask = VadMask::default();
    }

    pub(super) async fn dispatch_frame(
        &mut self,
        frame: SourceFrame,
        mode: ChannelMode,
        listener_routing: &ListenerRouting,
        recorder: Option<&ActorRef<RecMsg>>,
    ) -> Result<DispatchFrameResult, String> {
        self.dispatch(frame, mode, listener_routing, recorder).await
    }

    pub(super) fn has_pending_recorder_item(&self) -> bool {
        self.pending_recorder_item.is_some()
    }

    pub(super) async fn retry_pending_recorder(
        &mut self,
        listener_routing: &ListenerRouting,
        recorder: Option<&ActorRef<RecMsg>>,
    ) -> Result<DispatchFrameResult, String> {
        let Some(item) = self.pending_recorder_item.clone() else {
            return Ok(DispatchFrameResult::Complete);
        };
        let Some(recorder) = recorder else {
            return Ok(DispatchFrameResult::RecorderBackpressured);
        };

        match Self::write_to_recorder(recorder, &item).await? {
            RecorderEnqueueResult::Accepted => {
                self.pending_recorder_item = None;
                self.recorder_backpressure_started_at = None;
                self.complete_item(item, listener_routing);
                Ok(DispatchFrameResult::Complete)
            }
            RecorderEnqueueResult::Backpressured => {
                if self
                    .recorder_backpressure_started_at
                    .is_some_and(|started_at| started_at.elapsed() >= RECORDER_BACKPRESSURE_TIMEOUT)
                {
                    return Err("recorder queue remained full".into());
                }
                Ok(DispatchFrameResult::RecorderBackpressured)
            }
            RecorderEnqueueResult::Closed => Err("recorder writer is unavailable".into()),
        }
    }

    pub(super) fn on_listener_routing_changed(&mut self, listener_routing: &ListenerRouting) {
        match listener_routing {
            ListenerRouting::Buffering => {}
            ListenerRouting::Attached(actor) => {
                self.flush_buffer_to_listener(actor, usize::MAX);
            }
            ListenerRouting::Dropped => {
                self.listener_dispatcher.reset();
                self.audio_buffer.clear();
            }
        }
    }

    pub(super) fn prepare_listener_refresh(&mut self) -> ListenerRefreshReplay {
        self.listener_dispatcher.reset();
        self.audio_buffer.clear();

        for item in self.replay_history.items() {
            self.audio_buffer.push_item(item);
        }

        ListenerRefreshReplay {
            duration_secs: self.replay_history.duration_secs(),
        }
    }

    async fn dispatch(
        &mut self,
        frame: SourceFrame,
        mode: ChannelMode,
        listener_routing: &ListenerRouting,
        recorder: Option<&ActorRef<RecMsg>>,
    ) -> Result<DispatchFrameResult, String> {
        if self.pending_recorder_item.is_some() {
            return Err("recorder audio must be retried before dispatching another frame".into());
        }

        let (mut processed_mic, processed_spk) = Self::select_tracks(frame, mode);
        self.vad_mask.process(&mut processed_mic);
        let processed_mic = Arc::<[f32]>::from(processed_mic);
        let item = BufferedAudio::new(processed_mic, processed_spk, mode);

        if let Some(actor) = recorder {
            match Self::write_to_recorder(actor, &item).await? {
                RecorderEnqueueResult::Accepted => {}
                RecorderEnqueueResult::Backpressured => {
                    self.pending_recorder_item = Some(item);
                    self.recorder_backpressure_started_at = Some(Instant::now());
                    return Ok(DispatchFrameResult::RecorderBackpressured);
                }
                RecorderEnqueueResult::Closed => {
                    return Err("recorder writer is unavailable".into());
                }
            }
        }

        self.complete_item(item, listener_routing);
        Ok(DispatchFrameResult::Complete)
    }

    fn complete_item(&mut self, item: BufferedAudio, listener_routing: &ListenerRouting) {
        self.replay_history.push(item.clone());
        self.amplitude.observe_mic(&item.mic);
        self.amplitude.observe_spk(&item.spk);

        match listener_routing {
            ListenerRouting::Buffering => {
                self.audio_buffer.push_item(item);
                tracing::debug!(
                    buffered = self.audio_buffer.len(),
                    "listener_unavailable_buffering"
                );
            }
            ListenerRouting::Attached(actor) => {
                if self.audio_buffer.is_empty() {
                    if !self.send_to_listener(actor, &item.mic, &item.spk, item.mode) {
                        self.audio_buffer.push_item(item);
                    }
                } else {
                    self.audio_buffer.push_item(item);
                    self.flush_buffer_to_listener(actor, MAX_BACKLOG_DISPATCH_PER_FRAME);
                }
            }
            ListenerRouting::Dropped => {}
        }
    }

    async fn write_to_recorder(
        actor: &ActorRef<RecMsg>,
        item: &BufferedAudio,
    ) -> Result<RecorderEnqueueResult, String> {
        let result = match item.mode {
            ChannelMode::MicOnly => {
                let audio = Arc::clone(&item.mic);
                actor
                    .call(
                        move |reply| RecMsg::AudioSingle(audio, reply),
                        Some(RECORDER_RPC_TIMEOUT),
                    )
                    .await
            }
            ChannelMode::SpeakerOnly => {
                let audio = Arc::clone(&item.spk);
                actor
                    .call(
                        move |reply| RecMsg::AudioSingle(audio, reply),
                        Some(RECORDER_RPC_TIMEOUT),
                    )
                    .await
            }
            ChannelMode::MicAndSpeaker => {
                let mic = Arc::clone(&item.mic);
                let spk = Arc::clone(&item.spk);
                actor
                    .call(
                        move |reply| RecMsg::AudioDual(mic, spk, reply),
                        Some(RECORDER_RPC_TIMEOUT),
                    )
                    .await
            }
        };

        match result {
            Ok(CallResult::Success(result)) => Ok(result),
            Ok(CallResult::SenderError) => {
                Err("recorder stopped before acknowledging audio".into())
            }
            Ok(CallResult::Timeout) => Err("recorder enqueue acknowledgement timed out".into()),
            Err(error) => Err(format!("failed to send audio to recorder: {error}")),
        }
    }

    fn flush_buffer_to_listener(&mut self, actor: &ActorRef<ListenerMsg>, max_items: usize) {
        for _ in 0..max_items {
            let Some(item) = self.audio_buffer.pop() else {
                break;
            };

            if !self.send_to_listener(actor, &item.mic, &item.spk, item.mode) {
                self.audio_buffer.push_front(item);
                break;
            }
        }
    }

    fn send_to_listener(
        &self,
        actor: &ActorRef<ListenerMsg>,
        mic: &Arc<[f32]>,
        spk: &Arc<[f32]>,
        mode: ChannelMode,
    ) -> bool {
        self.listener_dispatcher
            .try_send(actor.clone(), mic, spk, mode)
    }

    fn select_tracks(frame: SourceFrame, mode: ChannelMode) -> (Vec<f32>, Arc<[f32]>) {
        let raw_speaker = Arc::clone(&frame.capture.raw_speaker);

        let mic_source = match mode {
            ChannelMode::SpeakerOnly => Arc::<[f32]>::from(vec![0.0; raw_speaker.len()]),
            ChannelMode::MicOnly | ChannelMode::MicAndSpeaker => frame.capture.preferred_mic(),
        };

        let mic = if frame.mic_muted {
            vec![0.0; mic_source.len()]
        } else {
            mic_source.to_vec()
        };

        (mic, raw_speaker)
    }
}

struct ListenerDispatcher {
    tx: tokio::sync::mpsc::Sender<ListenerDispatch>,
    task: tokio::task::JoinHandle<()>,
    generation: Arc<AtomicU64>,
}

struct ListenerDispatch {
    actor: ActorRef<ListenerMsg>,
    audio: ListenerAudio,
    generation: u64,
}

#[derive(Clone)]
enum ListenerAudio {
    Single(bytes::Bytes),
    Dual(bytes::Bytes, bytes::Bytes),
}

impl ListenerDispatcher {
    fn new() -> Self {
        let (tx, mut rx) =
            tokio::sync::mpsc::channel::<ListenerDispatch>(LISTENER_DISPATCH_CAPACITY);
        let generation = Arc::new(AtomicU64::new(0));
        let task_generation = generation.clone();
        let task = tokio::spawn(async move {
            while let Some(dispatch) = rx.recv().await {
                let started_at = Instant::now();
                loop {
                    if dispatch.generation != task_generation.load(Ordering::Acquire) {
                        break;
                    }

                    let actor = dispatch.actor.clone();
                    let result = match &dispatch.audio {
                        ListenerAudio::Single(audio) => {
                            let audio = audio.clone();
                            actor
                                .call(
                                    move |reply| ListenerMsg::AudioSingle(audio, reply),
                                    Some(LISTENER_AUDIO_ACK_TIMEOUT),
                                )
                                .await
                        }
                        ListenerAudio::Dual(mic, spk) => {
                            let mic = mic.clone();
                            let spk = spk.clone();
                            actor
                                .call(
                                    move |reply| ListenerMsg::AudioDual(mic, spk, reply),
                                    Some(LISTENER_AUDIO_ACK_TIMEOUT),
                                )
                                .await
                        }
                    };

                    match result {
                        Ok(CallResult::Success(ListenerAudioResult::Accepted)) => break,
                        Ok(CallResult::Success(ListenerAudioResult::Backpressured)) => {
                            if started_at.elapsed() >= LISTENER_BACKPRESSURE_TIMEOUT {
                                stop_listener(&actor, "listener_audio_backpressure_timed_out");
                                break;
                            }
                            tokio::time::sleep(LISTENER_BACKPRESSURE_RETRY_DELAY).await;
                        }
                        Ok(CallResult::Success(ListenerAudioResult::Closed)) => {
                            stop_listener(&actor, "listener_audio_channel_closed");
                            break;
                        }
                        Ok(CallResult::Success(ListenerAudioResult::ModeMismatch)) => {
                            stop_listener(&actor, "listener_audio_mode_mismatch");
                            break;
                        }
                        Ok(CallResult::SenderError) => {
                            tracing::warn!("listener_stopped_before_acknowledging_audio");
                            break;
                        }
                        Ok(CallResult::Timeout) => {
                            kill_listener(&actor, "listener_audio_ack_timed_out");
                            break;
                        }
                        Err(error) => {
                            tracing::warn!(error.message = ?error, "listener_audio_send_failed");
                            actor.kill();
                            break;
                        }
                    }
                }
            }
        });

        Self {
            tx,
            task,
            generation,
        }
    }

    fn reset(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    fn try_send(
        &self,
        actor: ActorRef<ListenerMsg>,
        mic: &Arc<[f32]>,
        spk: &Arc<[f32]>,
        mode: ChannelMode,
    ) -> bool {
        let Ok(permit) = self.tx.try_reserve() else {
            return false;
        };

        let audio = match mode {
            ChannelMode::MicOnly => ListenerAudio::Single(f32_to_i16_bytes(mic.iter().copied())),
            ChannelMode::SpeakerOnly => {
                ListenerAudio::Single(f32_to_i16_bytes(spk.iter().copied()))
            }
            ChannelMode::MicAndSpeaker => ListenerAudio::Dual(
                f32_to_i16_bytes(mic.iter().copied()),
                f32_to_i16_bytes(spk.iter().copied()),
            ),
        };

        permit.send(ListenerDispatch {
            actor,
            audio,
            generation: self.generation.load(Ordering::Acquire),
        });
        true
    }
}

fn stop_listener(actor: &ActorRef<ListenerMsg>, reason: &'static str) {
    tracing::warn!(reason, "listener_audio_delivery_failed");
    actor.stop(Some(reason.to_string()));
}

fn kill_listener(actor: &ActorRef<ListenerMsg>, reason: &'static str) {
    tracing::warn!(reason, "listener_audio_delivery_failed");
    actor.kill();
}

impl Drop for ListenerDispatcher {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct AudioBuffer {
    buffer: VecDeque<BufferedAudio>,
    max_size: usize,
    overflowing: bool,
}

impl AudioBuffer {
    fn new(max_size: usize) -> Self {
        Self {
            buffer: VecDeque::new(),
            max_size,
            overflowing: false,
        }
    }

    fn push_item(&mut self, item: BufferedAudio) {
        if self.buffer.len() >= self.max_size {
            self.buffer.pop_front();
            if !self.overflowing {
                self.overflowing = true;
                tracing::warn!("audio_buffer_overflow_listener_unavailable");
            }
        }
        self.buffer.push_back(item);
    }

    fn push_front(&mut self, item: BufferedAudio) {
        self.buffer.push_front(item);
    }

    fn pop(&mut self) -> Option<BufferedAudio> {
        let item = self.buffer.pop_front();
        if self.overflowing && self.buffer.len() < self.max_size {
            self.overflowing = false;
        }
        item
    }

    fn len(&self) -> usize {
        self.buffer.len()
    }

    fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    fn clear(&mut self) {
        self.buffer.clear();
        self.overflowing = false;
    }
}

struct ReplayHistory {
    buffer: VecDeque<BufferedAudio>,
    max_samples: usize,
    samples: usize,
}

impl ReplayHistory {
    fn new(max_samples: usize) -> Self {
        Self {
            buffer: VecDeque::new(),
            max_samples,
            samples: 0,
        }
    }

    fn push(&mut self, item: BufferedAudio) {
        self.samples += item.sample_count();
        self.buffer.push_back(item);

        while self.samples > self.max_samples {
            let Some(item) = self.buffer.pop_front() else {
                self.samples = 0;
                return;
            };
            self.samples = self.samples.saturating_sub(item.sample_count());
        }
    }

    fn items(&self) -> impl Iterator<Item = BufferedAudio> + '_ {
        self.buffer.iter().cloned()
    }

    fn duration_secs(&self) -> f64 {
        self.samples as f64 / SAMPLE_RATE as f64
    }

    fn clear(&mut self) {
        self.buffer.clear();
        self.samples = 0;
    }
}

struct AmplitudeEmitter {
    runtime: Arc<dyn ListenerRuntime>,
    session_id: String,
    mic_smoothed: f32,
    spk_smoothed: f32,
    last_emit: Instant,
}

impl AmplitudeEmitter {
    const SMOOTHING_ALPHA: f32 = 0.7;
    const MIN_DB: f32 = -60.0;
    const MAX_DB: f32 = 0.0;

    fn new(runtime: Arc<dyn ListenerRuntime>, session_id: String) -> Self {
        Self {
            runtime,
            session_id,
            mic_smoothed: 0.0,
            spk_smoothed: 0.0,
            last_emit: Instant::now() - AUDIO_AMPLITUDE_THROTTLE,
        }
    }

    fn reset(&mut self) {
        self.mic_smoothed = 0.0;
        self.spk_smoothed = 0.0;
        self.last_emit = Instant::now() - AUDIO_AMPLITUDE_THROTTLE;
    }

    fn observe_mic(&mut self, data: &[f32]) {
        let amplitude = Self::amplitude_from_chunk(data);
        self.mic_smoothed =
            (1.0 - Self::SMOOTHING_ALPHA) * self.mic_smoothed + Self::SMOOTHING_ALPHA * amplitude;
        self.emit_if_ready();
    }

    fn observe_spk(&mut self, data: &[f32]) {
        let amplitude = Self::amplitude_from_chunk(data);
        self.spk_smoothed =
            (1.0 - Self::SMOOTHING_ALPHA) * self.spk_smoothed + Self::SMOOTHING_ALPHA * amplitude;
        self.emit_if_ready();
    }

    fn emit_if_ready(&mut self) {
        if self.last_emit.elapsed() < AUDIO_AMPLITUDE_THROTTLE {
            return;
        }

        let mic_level = (self.mic_smoothed * 1000.0) as u16;
        let spk_level = (self.spk_smoothed * 1000.0) as u16;

        self.runtime.emit_data(SessionDataEvent::AudioAmplitude {
            session_id: self.session_id.clone(),
            mic: mic_level,
            speaker: spk_level,
        });

        self.last_emit = Instant::now();
    }

    fn amplitude_from_chunk(chunk: &[f32]) -> f32 {
        if chunk.is_empty() {
            return 0.0;
        }

        let sum_squares: f32 = chunk.iter().filter(|x| x.is_finite()).map(|&x| x * x).sum();
        let count = chunk.iter().filter(|x| x.is_finite()).count();
        if count == 0 {
            return 0.0;
        }
        let rms = (sum_squares / count as f32).sqrt();

        let db = if rms > 0.0 {
            20.0 * rms.log10()
        } else {
            Self::MIN_DB
        };

        ((db - Self::MIN_DB) / (Self::MAX_DB - Self::MIN_DB)).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use ractor::{Actor, ActorProcessingErr, ActorRef};

    use anlg_audio::CaptureFrame;

    use super::*;
    use crate::{
        ListenerRuntime, SessionDataEvent, SessionErrorEvent, SessionLifecycleEvent,
        SessionProgressEvent,
    };

    struct TestRuntime;

    impl anlg_storage::StorageRuntime for TestRuntime {
        fn global_base(&self) -> Result<PathBuf, anlg_storage::Error> {
            Ok(std::env::temp_dir())
        }

        fn vault_base(&self) -> Result<PathBuf, anlg_storage::Error> {
            Ok(std::env::temp_dir())
        }
    }

    impl ListenerRuntime for TestRuntime {
        fn emit_lifecycle(&self, _event: SessionLifecycleEvent) {}

        fn emit_progress(&self, _event: SessionProgressEvent) {}

        fn emit_error(&self, _event: SessionErrorEvent) {}

        fn emit_data(&self, _event: SessionDataEvent) {}
    }

    enum ProbeEvent {
        ListenerSingle,
        ListenerDual,
        ListenerControl,
        ListenerStopped,
        RecorderSingle,
        RecorderDual,
    }

    struct ListenerProbe(tokio::sync::mpsc::UnboundedSender<ProbeEvent>);

    struct OrderingListenerProbe(tokio::sync::mpsc::UnboundedSender<i16>);

    #[ractor::async_trait]
    impl Actor for ListenerProbe {
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
                ListenerMsg::AudioSingle(bytes, reply) => {
                    let _ = bytes.len();
                    let _ = self.0.send(ProbeEvent::ListenerSingle);
                    let _ = reply.send(ListenerAudioResult::Accepted);
                }
                ListenerMsg::AudioDual(mic, spk, reply) => {
                    let _ = (mic.len(), spk.len());
                    let _ = self.0.send(ProbeEvent::ListenerDual);
                    let _ = reply.send(ListenerAudioResult::Accepted);
                }
                _ => {}
            }
            Ok(())
        }
    }

    #[ractor::async_trait]
    impl Actor for OrderingListenerProbe {
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
            if let ListenerMsg::AudioSingle(bytes, reply) = message {
                let value = i16::from_le_bytes([bytes[0], bytes[1]]);
                let _ = self.0.send(value);
                let _ = reply.send(ListenerAudioResult::Accepted);
            }
            Ok(())
        }
    }

    struct RecorderProbe(tokio::sync::mpsc::UnboundedSender<ProbeEvent>);

    struct BackpressuredRecorderProbe;

    struct RecoveringRecorderProbe(tokio::sync::mpsc::UnboundedSender<ProbeEvent>);

    struct BackpressuredListenerProbe(tokio::sync::mpsc::UnboundedSender<ProbeEvent>);

    struct StuckListenerProbe;

    #[ractor::async_trait]
    impl Actor for BackpressuredListenerProbe {
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
                ListenerMsg::AudioSingle(_, reply) | ListenerMsg::AudioDual(_, _, reply) => {
                    let _ = reply.send(ListenerAudioResult::Backpressured);
                }
                ListenerMsg::StreamEnded => {
                    let _ = self.0.send(ProbeEvent::ListenerControl);
                }
                _ => {}
            }
            Ok(())
        }

        async fn post_stop(
            &self,
            _myself: ActorRef<Self::Msg>,
            _state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            let _ = self.0.send(ProbeEvent::ListenerStopped);
            Ok(())
        }
    }

    #[ractor::async_trait]
    impl Actor for StuckListenerProbe {
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
                ListenerMsg::AudioSingle(_, reply) | ListenerMsg::AudioDual(_, _, reply) => {
                    let reply = reply;
                    std::future::pending::<()>().await;
                    drop(reply);
                }
                _ => {}
            }
            Ok(())
        }
    }

    #[ractor::async_trait]
    impl Actor for RecorderProbe {
        type Msg = RecMsg;
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
                RecMsg::AudioSingle(samples, reply) => {
                    let _ = samples.len();
                    let _ = self.0.send(ProbeEvent::RecorderSingle);
                    let _ = reply.send(RecorderEnqueueResult::Accepted);
                }
                RecMsg::AudioDual(mic, spk, reply) => {
                    let _ = (mic.len(), spk.len());
                    let _ = self.0.send(ProbeEvent::RecorderDual);
                    let _ = reply.send(RecorderEnqueueResult::Accepted);
                }
                RecMsg::WriterFailed(_) => {}
            }
            Ok(())
        }
    }

    #[ractor::async_trait]
    impl Actor for BackpressuredRecorderProbe {
        type Msg = RecMsg;
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
                RecMsg::AudioSingle(_, reply) | RecMsg::AudioDual(_, _, reply) => {
                    let _ = reply.send(RecorderEnqueueResult::Backpressured);
                }
                RecMsg::WriterFailed(_) => {}
            }
            Ok(())
        }
    }

    #[ractor::async_trait]
    impl Actor for RecoveringRecorderProbe {
        type Msg = RecMsg;
        type State = bool;
        type Arguments = ();

        async fn pre_start(
            &self,
            _myself: ActorRef<Self::Msg>,
            _args: Self::Arguments,
        ) -> Result<Self::State, ActorProcessingErr> {
            Ok(false)
        }

        async fn handle(
            &self,
            _myself: ActorRef<Self::Msg>,
            message: Self::Msg,
            backpressured_once: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            match message {
                RecMsg::AudioSingle(_, reply) | RecMsg::AudioDual(_, _, reply) => {
                    if *backpressured_once {
                        let _ = self.0.send(ProbeEvent::RecorderDual);
                        let _ = reply.send(RecorderEnqueueResult::Accepted);
                    } else {
                        *backpressured_once = true;
                        let _ = reply.send(RecorderEnqueueResult::Backpressured);
                    }
                }
                RecMsg::WriterFailed(_) => {}
            }
            Ok(())
        }
    }

    fn test_pipeline() -> Pipeline {
        Pipeline::new(Arc::new(TestRuntime), "session".to_string())
    }

    fn capture_frame() -> CaptureFrame {
        CaptureFrame {
            raw_mic: Arc::from([0.25_f32, -0.25, 0.5, -0.5]),
            raw_speaker: Arc::from([0.75_f32, -0.75, 1.0, -1.0]),
            aec_mic: Some(Arc::from([0.1_f32, -0.1, 0.2, -0.2])),
        }
    }

    fn source_frame(mic_muted: bool) -> SourceFrame {
        SourceFrame {
            capture: capture_frame(),
            mic_muted,
        }
    }

    fn source_frame_with_speaker_value(value: f32) -> SourceFrame {
        SourceFrame {
            capture: CaptureFrame {
                raw_mic: Arc::from([0.0_f32; 4]),
                raw_speaker: Arc::from([value; 4]),
                aec_mic: None,
            },
            mic_muted: false,
        }
    }

    #[tokio::test]
    async fn buffers_until_listener_attaches_then_flushes() {
        let mut pipeline = test_pipeline();

        pipeline
            .dispatch_frame(
                source_frame(false),
                ChannelMode::MicAndSpeaker,
                &ListenerRouting::Buffering,
                None,
            )
            .await
            .unwrap();

        assert_eq!(pipeline.audio_buffer.len(), 1);

        let (probe_tx, mut probe_rx) = tokio::sync::mpsc::unbounded_channel();
        let (listener_ref, handle) = Actor::spawn(None, ListenerProbe(probe_tx), ())
            .await
            .unwrap();

        pipeline.on_listener_routing_changed(&ListenerRouting::Attached(listener_ref));

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), probe_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(event, ProbeEvent::ListenerDual));
        assert!(pipeline.audio_buffer.is_empty());

        handle.abort();
    }

    #[tokio::test]
    async fn listener_refresh_replays_recent_audio_history() {
        let mut pipeline = test_pipeline();

        let (old_probe_tx, mut old_probe_rx) = tokio::sync::mpsc::unbounded_channel();
        let (old_listener_ref, old_handle) = Actor::spawn(None, ListenerProbe(old_probe_tx), ())
            .await
            .unwrap();

        for _ in 0..3 {
            pipeline
                .dispatch_frame(
                    source_frame(false),
                    ChannelMode::MicAndSpeaker,
                    &ListenerRouting::Attached(old_listener_ref.clone()),
                    None,
                )
                .await
                .unwrap();
        }

        for _ in 0..3 {
            tokio::time::timeout(std::time::Duration::from_secs(1), old_probe_rx.recv())
                .await
                .unwrap()
                .unwrap();
        }

        let replay = pipeline.prepare_listener_refresh();

        assert_eq!(pipeline.audio_buffer.len(), 3);
        assert!(replay.duration_secs > 0.0);

        let (new_probe_tx, mut new_probe_rx) = tokio::sync::mpsc::unbounded_channel();
        let (new_listener_ref, new_handle) = Actor::spawn(None, ListenerProbe(new_probe_tx), ())
            .await
            .unwrap();

        pipeline.on_listener_routing_changed(&ListenerRouting::Attached(new_listener_ref));

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), new_probe_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(event, ProbeEvent::ListenerDual));

        old_handle.abort();
        new_handle.abort();
    }

    #[tokio::test]
    async fn buffered_audio_is_delivered_before_new_audio() {
        let mut pipeline = test_pipeline();

        for value in [0.1, 0.2] {
            pipeline
                .dispatch_frame(
                    source_frame_with_speaker_value(value),
                    ChannelMode::SpeakerOnly,
                    &ListenerRouting::Buffering,
                    None,
                )
                .await
                .unwrap();
        }

        let (probe_tx, mut probe_rx) = tokio::sync::mpsc::unbounded_channel();
        let (listener_ref, handle) = Actor::spawn(None, OrderingListenerProbe(probe_tx), ())
            .await
            .unwrap();
        let routing = ListenerRouting::Attached(listener_ref);
        pipeline.on_listener_routing_changed(&routing);
        pipeline
            .dispatch_frame(
                source_frame_with_speaker_value(0.3),
                ChannelMode::SpeakerOnly,
                &routing,
                None,
            )
            .await
            .unwrap();

        let mut values = Vec::new();
        for _ in 0..3 {
            values.push(
                tokio::time::timeout(std::time::Duration::from_secs(1), probe_rx.recv())
                    .await
                    .unwrap()
                    .unwrap(),
            );
        }
        assert!(values[0] < values[1] && values[1] < values[2]);

        handle.abort();
    }

    #[tokio::test]
    async fn listener_reconnect_replays_old_audio_before_new_audio() {
        let mut pipeline = test_pipeline();
        let (old_probe_tx, _old_probe_rx) = tokio::sync::mpsc::unbounded_channel();
        let (old_listener, old_handle) =
            Actor::spawn(None, BackpressuredListenerProbe(old_probe_tx), ())
                .await
                .unwrap();
        let old_routing = ListenerRouting::Attached(old_listener);

        for value in [0.1, 0.2] {
            pipeline
                .dispatch_frame(
                    source_frame_with_speaker_value(value),
                    ChannelMode::SpeakerOnly,
                    &old_routing,
                    None,
                )
                .await
                .unwrap();
        }
        tokio::task::yield_now().await;

        let replay = pipeline.prepare_listener_refresh();
        assert!(replay.duration_secs > 0.0);

        let (new_probe_tx, mut new_probe_rx) = tokio::sync::mpsc::unbounded_channel();
        let (new_listener, new_handle) =
            Actor::spawn(None, OrderingListenerProbe(new_probe_tx), ())
                .await
                .unwrap();
        let new_routing = ListenerRouting::Attached(new_listener);
        pipeline.on_listener_routing_changed(&new_routing);
        pipeline
            .dispatch_frame(
                source_frame_with_speaker_value(0.3),
                ChannelMode::SpeakerOnly,
                &new_routing,
                None,
            )
            .await
            .unwrap();

        let mut values = Vec::new();
        for _ in 0..3 {
            values.push(
                tokio::time::timeout(std::time::Duration::from_secs(1), new_probe_rx.recv())
                    .await
                    .unwrap()
                    .unwrap(),
            );
        }
        assert!(values[0] < values[1] && values[1] < values[2]);

        old_handle.abort();
        new_handle.abort();
    }

    #[tokio::test]
    async fn listener_backpressure_stays_bounded_without_blocking_control_messages() {
        let mut pipeline = test_pipeline();
        let (probe_tx, mut probe_rx) = tokio::sync::mpsc::unbounded_channel();
        let (listener_ref, handle) = Actor::spawn(None, BackpressuredListenerProbe(probe_tx), ())
            .await
            .unwrap();
        let routing = ListenerRouting::Attached(listener_ref.clone());

        for _ in 0..(LISTENER_DISPATCH_CAPACITY + MAX_BUFFER_CHUNKS + 100) {
            pipeline
                .dispatch_frame(
                    source_frame(false),
                    ChannelMode::MicAndSpeaker,
                    &routing,
                    None,
                )
                .await
                .unwrap();
        }

        assert_eq!(pipeline.audio_buffer.len(), MAX_BUFFER_CHUNKS);

        listener_ref.cast(ListenerMsg::StreamEnded).unwrap();
        let event = tokio::time::timeout(std::time::Duration::from_secs(1), probe_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(event, ProbeEvent::ListenerControl));

        handle.abort();
    }

    #[tokio::test]
    async fn listener_ack_timeout_terminates_stuck_actor() {
        let mut pipeline = test_pipeline();
        let (listener_ref, handle) = Actor::spawn(None, StuckListenerProbe, ()).await.unwrap();
        let routing = ListenerRouting::Attached(listener_ref);

        pipeline
            .dispatch_frame(
                source_frame(false),
                ChannelMode::MicAndSpeaker,
                &routing,
                None,
            )
            .await
            .unwrap();

        tokio::time::timeout(LISTENER_AUDIO_ACK_TIMEOUT + Duration::from_secs(1), handle)
            .await
            .expect("stuck listener should be terminated after its acknowledgement timeout")
            .unwrap();
    }

    #[tokio::test]
    async fn repeated_listener_backpressure_terminates_actor_and_preserves_replay() {
        let mut pipeline = test_pipeline();
        let (probe_tx, mut probe_rx) = tokio::sync::mpsc::unbounded_channel();
        let (listener_ref, handle) = Actor::spawn(None, BackpressuredListenerProbe(probe_tx), ())
            .await
            .unwrap();
        let routing = ListenerRouting::Attached(listener_ref);

        pipeline
            .dispatch_frame(
                source_frame(false),
                ChannelMode::MicAndSpeaker,
                &routing,
                None,
            )
            .await
            .unwrap();

        tokio::time::timeout(
            LISTENER_BACKPRESSURE_TIMEOUT + Duration::from_secs(1),
            handle,
        )
        .await
        .expect("listener should be terminated after sustained backpressure")
        .unwrap();

        let event = tokio::time::timeout(Duration::from_secs(1), probe_rx.recv())
            .await
            .expect("listener post_stop should complete")
            .expect("listener post_stop event");
        assert!(matches!(event, ProbeEvent::ListenerStopped));

        let replay = pipeline.prepare_listener_refresh();
        assert!(replay.duration_secs > 0.0);
        assert_eq!(pipeline.audio_buffer.len(), 1);
    }

    #[tokio::test]
    async fn dropped_listener_clears_backlog_and_stops_future_buffering() {
        let mut pipeline = test_pipeline();

        pipeline
            .dispatch_frame(
                source_frame(false),
                ChannelMode::MicAndSpeaker,
                &ListenerRouting::Buffering,
                None,
            )
            .await
            .unwrap();
        assert_eq!(pipeline.audio_buffer.len(), 1);

        pipeline.on_listener_routing_changed(&ListenerRouting::Dropped);
        assert!(pipeline.audio_buffer.is_empty());

        let (probe_tx, mut probe_rx) = tokio::sync::mpsc::unbounded_channel();
        let (listener_ref, handle) = Actor::spawn(None, ListenerProbe(probe_tx), ())
            .await
            .unwrap();

        pipeline.on_listener_routing_changed(&ListenerRouting::Attached(listener_ref));

        pipeline
            .dispatch_frame(
                source_frame(false),
                ChannelMode::MicAndSpeaker,
                &ListenerRouting::Dropped,
                None,
            )
            .await
            .unwrap();

        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(200), probe_rx.recv())
                .await
                .is_err()
        );

        handle.abort();
    }

    #[tokio::test]
    async fn recorder_receives_audio_from_explicit_sink() {
        let mut pipeline = test_pipeline();

        let (probe_tx, mut probe_rx) = tokio::sync::mpsc::unbounded_channel();
        let (recorder_ref, handle) = Actor::spawn(None, RecorderProbe(probe_tx), ())
            .await
            .unwrap();

        pipeline
            .dispatch_frame(
                source_frame(false),
                ChannelMode::MicAndSpeaker,
                &ListenerRouting::Dropped,
                Some(&recorder_ref),
            )
            .await
            .unwrap();

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), probe_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(event, ProbeEvent::RecorderDual));

        handle.abort();
    }

    #[tokio::test]
    async fn recorder_backpressure_yields_before_a_finite_deadline() {
        let mut pipeline = test_pipeline();
        let (recorder_ref, handle) = Actor::spawn(None, BackpressuredRecorderProbe, ())
            .await
            .unwrap();
        let started_at = Instant::now();

        let result = pipeline
            .dispatch_frame(
                source_frame(false),
                ChannelMode::MicAndSpeaker,
                &ListenerRouting::Dropped,
                Some(&recorder_ref),
            )
            .await
            .unwrap();

        assert_eq!(result, DispatchFrameResult::RecorderBackpressured);
        assert!(pipeline.has_pending_recorder_item());
        assert!(started_at.elapsed() < RECORDER_BACKPRESSURE_TIMEOUT);

        pipeline.recorder_backpressure_started_at =
            Some(Instant::now() - RECORDER_BACKPRESSURE_TIMEOUT);
        let error = pipeline
            .retry_pending_recorder(&ListenerRouting::Dropped, Some(&recorder_ref))
            .await
            .unwrap_err();

        assert_eq!(error, "recorder queue remained full");

        handle.abort();
    }

    #[tokio::test]
    async fn recorder_backpressure_retries_the_same_frame_without_dropping_it() {
        let mut pipeline = test_pipeline();
        let (probe_tx, mut probe_rx) = tokio::sync::mpsc::unbounded_channel();
        let (recorder_ref, handle) = Actor::spawn(None, RecoveringRecorderProbe(probe_tx), ())
            .await
            .unwrap();

        let first_result = pipeline
            .dispatch_frame(
                source_frame(false),
                ChannelMode::MicAndSpeaker,
                &ListenerRouting::Dropped,
                Some(&recorder_ref),
            )
            .await
            .unwrap();
        assert_eq!(first_result, DispatchFrameResult::RecorderBackpressured);

        let retry_result = pipeline
            .retry_pending_recorder(&ListenerRouting::Dropped, Some(&recorder_ref))
            .await
            .unwrap();
        assert_eq!(retry_result, DispatchFrameResult::Complete);
        assert!(!pipeline.has_pending_recorder_item());
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), probe_rx.recv())
                .await
                .unwrap(),
            Some(ProbeEvent::RecorderDual)
        ));

        handle.abort();
    }

    #[tokio::test]
    async fn recorder_backpressure_survives_recorder_restart() {
        let mut pipeline = test_pipeline();
        let (backpressured_ref, backpressured_handle) =
            Actor::spawn(None, BackpressuredRecorderProbe, ())
                .await
                .unwrap();

        let first_result = pipeline
            .dispatch_frame(
                source_frame(false),
                ChannelMode::MicAndSpeaker,
                &ListenerRouting::Dropped,
                Some(&backpressured_ref),
            )
            .await
            .unwrap();
        assert_eq!(first_result, DispatchFrameResult::RecorderBackpressured);

        let missing_result = pipeline
            .retry_pending_recorder(&ListenerRouting::Dropped, None)
            .await
            .unwrap();
        assert_eq!(missing_result, DispatchFrameResult::RecorderBackpressured);
        assert!(pipeline.has_pending_recorder_item());

        let (probe_tx, mut probe_rx) = tokio::sync::mpsc::unbounded_channel();
        let (replacement_ref, replacement_handle) = Actor::spawn(None, RecorderProbe(probe_tx), ())
            .await
            .unwrap();
        let retry_result = pipeline
            .retry_pending_recorder(&ListenerRouting::Dropped, Some(&replacement_ref))
            .await
            .unwrap();

        assert_eq!(retry_result, DispatchFrameResult::Complete);
        assert!(!pipeline.has_pending_recorder_item());
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(1), probe_rx.recv())
                .await
                .unwrap(),
            Some(ProbeEvent::RecorderDual)
        ));

        backpressured_handle.abort();
        replacement_handle.abort();
    }

    #[test]
    fn select_tracks_prefers_aec_mic() {
        let (mic, speaker) =
            Pipeline::select_tracks(source_frame(false), ChannelMode::MicAndSpeaker);
        assert_eq!(mic, vec![0.1, -0.1, 0.2, -0.2]);
        assert_eq!(&*speaker, &[0.75, -0.75, 1.0, -1.0]);
    }

    #[test]
    fn select_tracks_falls_back_to_raw_mic() {
        let mut frame = source_frame(false);
        frame.capture.aec_mic = None;

        let (mic, speaker) = Pipeline::select_tracks(frame, ChannelMode::MicAndSpeaker);
        assert_eq!(mic, vec![0.25, -0.25, 0.5, -0.5]);
        assert_eq!(&*speaker, &[0.75, -0.75, 1.0, -1.0]);
    }

    #[test]
    fn select_tracks_zeroes_muted_mic() {
        let (mic, speaker) =
            Pipeline::select_tracks(source_frame(true), ChannelMode::MicAndSpeaker);
        assert_eq!(mic, vec![0.0, 0.0, 0.0, 0.0]);
        assert_eq!(&*speaker, &[0.75, -0.75, 1.0, -1.0]);
    }

    #[test]
    fn select_tracks_zeroes_mic_for_speaker_only() {
        let (mic, speaker) = Pipeline::select_tracks(source_frame(false), ChannelMode::SpeakerOnly);
        assert_eq!(mic, vec![0.0, 0.0, 0.0, 0.0]);
        assert_eq!(&*speaker, &[0.75, -0.75, 1.0, -1.0]);
    }
}
