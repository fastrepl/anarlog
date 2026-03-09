use std::{
    collections::{HashMap, VecDeque},
    env,
    sync::Arc,
    time::{Duration, Instant},
};

use ractor::ActorRef;

use crate::{
    ListenerRuntime, SessionDataEvent,
    actors::{AudioChunk, ChannelMode, ListenerMsg, RecMsg},
};
use hypr_aec::AEC;
use hypr_audio_utils::f32_to_i16_bytes;
use hypr_vad_masking::VadMask;

use super::ListenerRouting;

const AUDIO_AMPLITUDE_THROTTLE: Duration = Duration::from_millis(100);
const MAX_BUFFER_CHUNKS: usize = 150;

type AudioPair = (Vec<f32>, Vec<f32>);
type BufferedAudio = (Arc<[f32]>, Arc<[f32]>, ChannelMode);

pub(in crate::actors) struct Pipeline {
    vad_mask: VadMask,
    aec: Option<AEC>,
    joiner: Joiner,
    amplitude: AmplitudeEmitter,
    audio_buffer: AudioBuffer,
    backlog_quota: f32,
}

impl Pipeline {
    const BACKLOG_QUOTA_INCREMENT: f32 = 0.25;
    const MAX_BACKLOG_QUOTA: f32 = 2.0;

    pub(super) fn new(runtime: Arc<dyn ListenerRuntime>, session_id: String) -> Self {
        Self {
            aec: if env::var("NO_AEC").as_deref() == Ok("1") {
                None
            } else {
                AEC::new()
                    .map_err(|e| tracing::warn!(error.message = ?e, "aec_init_failed"))
                    .ok()
            },
            joiner: Joiner::new(),
            amplitude: AmplitudeEmitter::new(runtime, session_id),
            audio_buffer: AudioBuffer::new(MAX_BUFFER_CHUNKS),
            backlog_quota: 0.0,
            vad_mask: VadMask::default(),
        }
    }

    pub(super) fn reset(&mut self) {
        self.joiner.reset();
        if let Some(aec) = &mut self.aec {
            aec.reset();
        }
        self.amplitude.reset();
        self.audio_buffer.clear();
        self.backlog_quota = 0.0;
        self.vad_mask = VadMask::default();
    }

    pub(super) fn ingest_mic(&mut self, chunk: AudioChunk) {
        self.joiner.push_mic(chunk.data);
    }

    pub(super) fn ingest_speaker(&mut self, chunk: AudioChunk) {
        self.joiner.push_spk(chunk.data);
    }

    pub(super) fn flush(
        &mut self,
        mode: ChannelMode,
        listener_routing: &ListenerRouting,
        recorder: Option<&ActorRef<RecMsg>>,
    ) {
        while let Some((mic, spk)) = self.joiner.pop_pair(mode) {
            self.dispatch(mic, spk, mode, listener_routing, recorder);
        }
    }

    pub(super) fn on_listener_routing_changed(&mut self, listener_routing: &ListenerRouting) {
        match listener_routing {
            ListenerRouting::Buffering => {}
            ListenerRouting::Attached(actor) => {
                if !self.audio_buffer.is_empty() && self.backlog_quota < 1.0 {
                    self.backlog_quota = 1.0;
                }
                self.flush_buffer_to_listener(actor);
            }
            ListenerRouting::Dropped => {
                self.audio_buffer.clear();
                self.backlog_quota = 0.0;
            }
        }
    }

    fn dispatch(
        &mut self,
        mic: Vec<f32>,
        spk: Vec<f32>,
        mode: ChannelMode,
        listener_routing: &ListenerRouting,
        recorder: Option<&ActorRef<RecMsg>>,
    ) {
        let mut processed_mic = if let Some(aec) = &mut self.aec {
            match aec.process_streaming(&mic, &spk) {
                Ok(processed) => processed,
                Err(e) => {
                    tracing::warn!(error.message = ?e, "aec_failed");
                    mic
                }
            }
        } else {
            mic
        };

        self.vad_mask.process(&mut processed_mic);
        let processed_mic = Arc::<[f32]>::from(processed_mic);
        let processed_spk = Arc::<[f32]>::from(spk);

        self.amplitude.observe_mic(&processed_mic);
        self.amplitude.observe_spk(&processed_spk);

        if let Some(actor) = recorder {
            let result = match mode {
                ChannelMode::MicOnly => actor.cast(RecMsg::AudioSingle(Arc::clone(&processed_mic))),
                ChannelMode::SpeakerOnly => {
                    actor.cast(RecMsg::AudioSingle(Arc::clone(&processed_spk)))
                }
                ChannelMode::MicAndSpeaker => actor.cast(RecMsg::AudioDual(
                    Arc::clone(&processed_mic),
                    Arc::clone(&processed_spk),
                )),
            };
            if let Err(e) = result {
                tracing::error!(error.message = ?e, "failed_to_send_audio_to_recorder");
            }
        }

        match listener_routing {
            ListenerRouting::Buffering => {
                self.audio_buffer.push(processed_mic, processed_spk, mode);
                tracing::debug!(
                    buffered = self.audio_buffer.len(),
                    "listener_unavailable_buffering"
                );
            }
            ListenerRouting::Attached(actor) => {
                self.flush_buffer_to_listener(actor);
                self.send_to_listener(actor, &processed_mic, &processed_spk, mode);
            }
            ListenerRouting::Dropped => {}
        }
    }

    fn flush_buffer_to_listener(&mut self, actor: &ActorRef<ListenerMsg>) {
        if !self.audio_buffer.is_empty() {
            self.backlog_quota =
                (self.backlog_quota + Self::BACKLOG_QUOTA_INCREMENT).min(Self::MAX_BACKLOG_QUOTA);

            while self.backlog_quota >= 1.0 {
                let Some((mic, spk, buffered_mode)) = self.audio_buffer.pop() else {
                    break;
                };

                self.send_to_listener(actor, &mic, &spk, buffered_mode);
                self.backlog_quota -= 1.0;
            }
        }
    }

    fn send_to_listener(
        &self,
        actor: &ActorRef<ListenerMsg>,
        mic: &Arc<[f32]>,
        spk: &Arc<[f32]>,
        mode: ChannelMode,
    ) {
        let result = match mode {
            ChannelMode::MicOnly => {
                let bytes = f32_to_i16_bytes(mic.iter().copied());
                actor.cast(ListenerMsg::AudioSingle(bytes))
            }
            ChannelMode::SpeakerOnly => {
                let bytes = f32_to_i16_bytes(spk.iter().copied());
                actor.cast(ListenerMsg::AudioSingle(bytes))
            }
            ChannelMode::MicAndSpeaker => {
                let mic_bytes = f32_to_i16_bytes(mic.iter().copied());
                let spk_bytes = f32_to_i16_bytes(spk.iter().copied());
                actor.cast(ListenerMsg::AudioDual(mic_bytes, spk_bytes))
            }
        };

        if result.is_err() {
            tracing::warn!("listener_cast_failed");
        }
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

    fn push(&mut self, mic: Arc<[f32]>, spk: Arc<[f32]>, mode: ChannelMode) {
        if self.buffer.len() >= self.max_size {
            self.buffer.pop_front();
            if !self.overflowing {
                self.overflowing = true;
                tracing::warn!("audio_buffer_overflow_listener_unavailable");
            }
        }
        self.buffer.push_back((mic, spk, mode));
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

struct Joiner {
    mic: VecDeque<Vec<f32>>,
    spk: VecDeque<Vec<f32>>,
    silence_cache: HashMap<usize, Arc<[f32]>>,
}

impl Joiner {
    const MAX_LAG: usize = 4;
    const MAX_QUEUE_SIZE: usize = 30;

    fn new() -> Self {
        Self {
            mic: VecDeque::new(),
            spk: VecDeque::new(),
            silence_cache: HashMap::new(),
        }
    }

    fn reset(&mut self) {
        self.mic.clear();
        self.spk.clear();
    }

    fn get_silence(&mut self, len: usize) -> Vec<f32> {
        self.silence_cache
            .entry(len)
            .or_insert_with(|| Arc::from(vec![0.0; len]))
            .to_vec()
    }

    fn push_mic(&mut self, data: Vec<f32>) {
        self.mic.push_back(data);
        if self.mic.len() > Self::MAX_QUEUE_SIZE {
            tracing::warn!("mic_queue_overflow");
            self.mic.pop_front();
        }
    }

    fn push_spk(&mut self, data: Vec<f32>) {
        self.spk.push_back(data);
        if self.spk.len() > Self::MAX_QUEUE_SIZE {
            tracing::warn!("spk_queue_overflow");
            self.spk.pop_front();
        }
    }

    fn pop_pair(&mut self, mode: ChannelMode) -> Option<AudioPair> {
        if self.mic.front().is_some() && self.spk.front().is_some() {
            return Some((self.mic.pop_front()?, self.spk.pop_front()?));
        }

        match mode {
            ChannelMode::MicOnly => {
                if let Some(mic) = self.mic.pop_front() {
                    let spk = self.get_silence(mic.len());
                    return Some((mic, spk));
                }
            }
            ChannelMode::SpeakerOnly => {
                if let Some(spk) = self.spk.pop_front() {
                    let mic = self.get_silence(spk.len());
                    return Some((mic, spk));
                }
            }
            ChannelMode::MicAndSpeaker => {
                if self.mic.front().is_some()
                    && self.spk.is_empty()
                    && self.mic.len() > Self::MAX_LAG
                {
                    let mic = self.mic.pop_front()?;
                    let spk = self.get_silence(mic.len());
                    return Some((mic, spk));
                }
                if self.spk.front().is_some()
                    && self.mic.is_empty()
                    && self.spk.len() > Self::MAX_LAG
                {
                    let spk = self.spk.pop_front()?;
                    let mic = self.get_silence(spk.len());
                    return Some((mic, spk));
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use ractor::{Actor, ActorProcessingErr, ActorRef};

    use super::*;
    use crate::{
        ListenerRuntime, SessionDataEvent, SessionErrorEvent, SessionLifecycleEvent,
        SessionProgressEvent,
    };

    struct TestRuntime;

    impl hypr_storage::StorageRuntime for TestRuntime {
        fn global_base(&self) -> Result<PathBuf, hypr_storage::Error> {
            Ok(std::env::temp_dir())
        }

        fn vault_base(&self) -> Result<PathBuf, hypr_storage::Error> {
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
        RecorderSingle,
        RecorderDual,
    }

    struct ListenerProbe(tokio::sync::mpsc::UnboundedSender<ProbeEvent>);

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
                ListenerMsg::AudioSingle(bytes) => {
                    let _ = bytes.len();
                    let _ = self.0.send(ProbeEvent::ListenerSingle);
                }
                ListenerMsg::AudioDual(mic, spk) => {
                    let _ = (mic.len(), spk.len());
                    let _ = self.0.send(ProbeEvent::ListenerDual);
                }
                _ => {}
            }
            Ok(())
        }
    }

    struct RecorderProbe(tokio::sync::mpsc::UnboundedSender<ProbeEvent>);

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
                RecMsg::AudioSingle(samples) => {
                    let _ = samples.len();
                    let _ = self.0.send(ProbeEvent::RecorderSingle);
                }
                RecMsg::AudioDual(mic, spk) => {
                    let _ = (mic.len(), spk.len());
                    let _ = self.0.send(ProbeEvent::RecorderDual);
                }
            }
            Ok(())
        }
    }

    fn test_pipeline() -> Pipeline {
        Pipeline::new(Arc::new(TestRuntime), "session".to_string())
    }

    fn stereo_chunk() -> AudioChunk {
        AudioChunk {
            data: vec![0.25, -0.25, 0.5, -0.5],
        }
    }

    #[tokio::test]
    async fn buffers_until_listener_attaches_then_flushes() {
        let mut pipeline = test_pipeline();

        pipeline.ingest_mic(stereo_chunk());
        pipeline.ingest_speaker(stereo_chunk());
        pipeline.flush(
            ChannelMode::MicAndSpeaker,
            &ListenerRouting::Buffering,
            None,
        );

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
    async fn dropped_listener_clears_backlog_and_stops_future_buffering() {
        let mut pipeline = test_pipeline();

        pipeline.ingest_mic(stereo_chunk());
        pipeline.ingest_speaker(stereo_chunk());
        pipeline.flush(
            ChannelMode::MicAndSpeaker,
            &ListenerRouting::Buffering,
            None,
        );
        assert_eq!(pipeline.audio_buffer.len(), 1);

        pipeline.on_listener_routing_changed(&ListenerRouting::Dropped);
        assert!(pipeline.audio_buffer.is_empty());

        let (probe_tx, mut probe_rx) = tokio::sync::mpsc::unbounded_channel();
        let (listener_ref, handle) = Actor::spawn(None, ListenerProbe(probe_tx), ())
            .await
            .unwrap();

        pipeline.on_listener_routing_changed(&ListenerRouting::Attached(listener_ref));

        pipeline.ingest_mic(stereo_chunk());
        pipeline.ingest_speaker(stereo_chunk());
        pipeline.flush(ChannelMode::MicAndSpeaker, &ListenerRouting::Dropped, None);

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

        pipeline.ingest_mic(stereo_chunk());
        pipeline.ingest_speaker(stereo_chunk());
        pipeline.flush(
            ChannelMode::MicAndSpeaker,
            &ListenerRouting::Dropped,
            Some(&recorder_ref),
        );

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), probe_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(event, ProbeEvent::RecorderDual));

        handle.abort();
    }
}
