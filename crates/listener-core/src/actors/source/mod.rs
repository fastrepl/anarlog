mod pipeline;
mod stream;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
};
use std::time::Duration;

use ractor::{Actor, ActorName, ActorProcessingErr, ActorRef, RpcReplyPort};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use crate::{
    ListenerRuntime, SessionErrorEvent, SessionProgressEvent,
    actors::session::session_span,
    actors::{ChannelMode, ListenerMsg, RecMsg},
};
use anlg_audio::{AudioProvider, CaptureChannel, CaptureFrame};

use pipeline::Pipeline;
use stream::start_source_loop;

use anlg_device_monitor::{DeviceMonitorHandle, DeviceSwitch, DeviceSwitchMonitor};

pub enum SourceMsg {
    SetMicMute(bool),
    GetMicMute(RpcReplyPort<bool>),
    GetMicDevice(RpcReplyPort<Option<String>>),
    GetMicIsolated(RpcReplyPort<bool>),
    GetCaptureHealth(RpcReplyPort<CaptureHealth>),
    PrepareListenerRefresh(RpcReplyPort<ListenerRefreshReplay>),
    SetListenerRouting(ListenerRouting),
    SetRecorder(Option<ActorRef<RecMsg>>),
    CaptureFramesReady,
    CaptureReady,
    CaptureUnavailable,
    ChannelReady(CaptureChannel, Option<String>),
    ChannelFailed(CaptureChannel, String),
    RetryCapture(Option<String>),
    InputChanged,
    OutputChanged,
    StreamFailed(String),
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CaptureHealth {
    pub mic: Option<bool>,
    pub speaker: Option<bool>,
    pub unavailable: bool,
}

pub struct SourceFrame {
    pub capture: CaptureFrame,
    pub mic_muted: bool,
    pub captured: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ListenerRefreshReplay {
    pub duration_secs: f64,
}

#[derive(Clone)]
pub enum ListenerRouting {
    Buffering,
    Attached(ActorRef<ListenerMsg>),
    Dropped,
}

pub struct SourceArgs {
    pub mic_device: Option<String>,
    pub onboarding: bool,
    pub runtime: Arc<dyn ListenerRuntime>,
    pub audio: Arc<dyn AudioProvider>,
    pub session_id: String,
    pub listener_routing: ListenerRouting,
    pub recorder: Option<ActorRef<RecMsg>>,
}

pub struct SourceState {
    pub(super) runtime: Arc<dyn ListenerRuntime>,
    pub(super) audio: Arc<dyn AudioProvider>,
    pub(super) session_id: String,
    /// The user's explicit choice; `None` defers to the system default.
    pub(super) mic_device: Option<String>,
    /// The device the current stream actually opened. Differs from `mic_device` when a Bluetooth
    /// default input was swapped for a wired one.
    pub(super) active_mic_device: Option<String>,
    pub(super) onboarding: bool,
    pub(super) capture_mic_isolated: bool,
    pub(super) mic_muted: Arc<AtomicBool>,
    pub(super) run_task: Option<tokio::task::JoinHandle<()>>,
    pub(super) stream_cancel_token: Option<CancellationToken>,
    pub(super) capture_frames: Option<tokio::sync::mpsc::Receiver<SourceFrame>>,
    pub(super) capture_wake_pending: Arc<AtomicBool>,
    pub(super) capture_retry: Option<tokio::sync::mpsc::UnboundedSender<(Option<String>, bool)>>,
    pub(super) capture_output: Option<tokio::sync::mpsc::UnboundedSender<bool>>,
    capture_health: CaptureHealth,
    healthy_since: Option<std::time::Instant>,
    last_frame: Option<std::time::Instant>,
    pub(super) current_mode: ChannelMode,
    pub(super) pipeline: Pipeline,
    pub(super) listener_routing: ListenerRouting,
    pub(super) recorder: Option<ActorRef<RecMsg>>,
    _device_watcher: Option<DeviceChangeWatcher>,
    _silence_stream_tx: Option<std::sync::mpsc::Sender<()>>,
}

pub struct SourceActor;

const MAX_CAPTURE_FRAMES_PER_TICK: usize = 4;
const OUTPUT_ROUTING_POLL_INTERVAL: Duration = Duration::from_secs(2);
// Only the macOS backend reports which outputs are running; elsewhere the verdict can only move
// with the default output.
const POLLS_OUTPUT_ROUTING: bool = cfg!(target_os = "macos");

struct DeviceChangeWatcher {
    _handle: DeviceMonitorHandle,
    _thread: std::thread::JoinHandle<()>,
}

impl DeviceChangeWatcher {
    fn spawn(actor: ActorRef<SourceMsg>, headphone_output: bool) -> Self {
        let (event_tx, event_rx) = mpsc::sync_channel(1);
        let handle = DeviceSwitchMonitor::spawn_debounced_bounded(event_tx);
        let routing = POLLS_OUTPUT_ROUTING.then(|| OutputRoutingTracker::new(headphone_output));
        let thread = std::thread::spawn(move || Self::event_loop(event_rx, actor, routing));

        Self {
            _handle: handle,
            _thread: thread,
        }
    }

    fn event_loop(
        event_rx: Receiver<DeviceSwitch>,
        actor: ActorRef<SourceMsg>,
        mut routing: Option<OutputRoutingTracker>,
    ) {
        loop {
            let event = match routing {
                Some(_) => event_rx.recv_timeout(OUTPUT_ROUTING_POLL_INTERVAL),
                None => event_rx
                    .recv()
                    .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
            };
            match event {
                Ok(DeviceSwitch::DeviceListChanged) => {}
                Ok(event)
                    if !device_switch_updates_capture(
                        &event,
                        anlg_audio_device::bluetooth_input_owns_system_defaults(),
                    ) =>
                {
                    tracing::info!(?event, "device_switch_ignored_bluetooth_handoff");
                }
                Ok(DeviceSwitch::DefaultInputChanged) => {
                    tracing::info!("default_input_changed_retrying_microphone");
                    let _ = actor.cast(SourceMsg::InputChanged);
                }
                Ok(DeviceSwitch::DefaultOutputChanged { .. }) => {
                    tracing::info!("default_output_changed_retrying_speaker");
                    let _ = actor.cast(SourceMsg::OutputChanged);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let Some(routing) = routing.as_mut() else {
                        continue;
                    };
                    let observed = headphone_only_output();
                    if routing.observe(observed) {
                        if anlg_audio_device::bluetooth_input_owns_system_defaults() {
                            tracing::info!(
                                headphone_output = observed,
                                "output_routing_ignored_bluetooth_handoff"
                            );
                            continue;
                        }
                        tracing::info!(
                            headphone_output = observed,
                            "output_routing_changed_retrying_speaker"
                        );
                        let _ = actor.cast(SourceMsg::OutputChanged);
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    }
}

fn headphone_only_output() -> bool {
    anlg_audio_device::headphone_only_output().is_some()
}

// The same mic-isolation verdict `start_source_loop` emits as `MicIsolated`, recomputed
// for callers that need it without a running source (e.g. listener spawn).
pub(crate) fn mic_isolated(mic_device: &Option<String>, audio: &dyn AudioProvider) -> bool {
    let mic_swapped =
        mic_device.is_none() && stream::active_mic_device(mic_device.clone(), audio).is_some();
    headphone_only_output() && !mic_swapped
}

// Holding a Bluetooth headset in HFP/SCO sets the system default input and can also move
// the default output. Those Core Audio events must not bounce the source we just opened.
fn device_switch_updates_capture(event: &DeviceSwitch, bluetooth_owns_defaults: bool) -> bool {
    match event {
        DeviceSwitch::DeviceListChanged => false,
        DeviceSwitch::DefaultInputChanged | DeviceSwitch::DefaultOutputChanged { .. } => {
            !bluetooth_owns_defaults
        }
    }
}

// A meeting app can start playing through speakers after capture began, flipping the AEC and
// mic-isolation verdict the streams were opened with. Requiring two consecutive polls keeps a
// one-off system sound from interrupting capture.
struct OutputRoutingTracker {
    expected: bool,
    pending: Option<bool>,
}

impl OutputRoutingTracker {
    fn new(expected: bool) -> Self {
        Self {
            expected,
            pending: None,
        }
    }

    fn observe(&mut self, observed: bool) -> bool {
        if observed == self.expected {
            self.pending = None;
            return false;
        }
        if self.pending == Some(observed) {
            self.expected = observed;
            self.pending = None;
            return true;
        }
        self.pending = Some(observed);
        false
    }
}

fn capturing_mic_device(st: &SourceState) -> Option<String> {
    if st.current_mode == ChannelMode::SpeakerOnly {
        return None;
    }
    match st.capture_health.mic {
        Some(false) => None,
        None if st.capture_health.speaker.is_some() => None,
        _ => st.active_mic_device.clone(),
    }
}

fn emit_mic_isolation(st: &SourceState) {
    st.runtime.emit_data(crate::SessionDataEvent::MicIsolated {
        session_id: st.session_id.clone(),
        value: st.capture_mic_isolated,
    });
    if let Some(session) =
        ractor::registry::where_is(crate::actors::session_supervisor_name(&st.session_id))
    {
        let session: ActorRef<crate::actors::SessionMsg> = session.into();
        let _ = session.cast(crate::actors::SessionMsg::SourceRoutingChanged);
    }
}

fn emit_channel_status(st: &SourceState, error: String) {
    st.runtime.emit_error(SessionErrorEvent::AudioError {
        session_id: st.session_id.clone(),
        error,
        device: st.active_mic_device.clone(),
        is_fatal: false,
    });
}

impl SourceActor {
    pub fn name(session_id: &str) -> ActorName {
        format!("source_{session_id}")
    }
}

#[ractor::async_trait]
impl Actor for SourceActor {
    type Msg = SourceMsg;
    type State = SourceState;
    type Arguments = SourceArgs;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let session_id = args.session_id.clone();
        let span = session_span(&session_id);

        async {
            args.runtime
                .emit_progress(SessionProgressEvent::AudioInitializing {
                    session_id: session_id.clone(),
                });

            let silence_stream_tx = Some(args.audio.play_silence());
            let mic_device = args.mic_device;
            tracing::info!(mic_device = ?mic_device);

            let pipeline = Pipeline::new(args.runtime.clone(), args.session_id.clone());

            let mut st = SourceState {
                runtime: args.runtime,
                audio: args.audio,
                session_id: args.session_id,
                mic_device,
                active_mic_device: None,
                onboarding: args.onboarding,
                capture_mic_isolated: false,
                mic_muted: Arc::new(AtomicBool::new(false)),
                run_task: None,
                stream_cancel_token: None,
                capture_frames: None,
                capture_wake_pending: Arc::new(AtomicBool::new(false)),
                _device_watcher: None,
                _silence_stream_tx: silence_stream_tx,
                capture_retry: None,
                capture_output: None,
                capture_health: CaptureHealth::default(),
                healthy_since: None,
                last_frame: None,
                current_mode: ChannelMode::MicAndSpeaker,
                pipeline,
                listener_routing: args.listener_routing,
                recorder: args.recorder,
            };

            // The watcher's baseline is the verdict the streams opened with, sampled after the
            // silence stream started, so it cannot read startup skew as a routing change.
            let capture = start_source_loop(&myself, &mut st).await?;
            st._device_watcher = Some(DeviceChangeWatcher::spawn(
                myself.clone(),
                capture.headphone_output,
            ));
            Ok(st)
        }
        .instrument(span)
        .await
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        st: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        let span = session_span(&st.session_id);
        async {
            match msg {
                SourceMsg::SetMicMute(muted) => {
                    st.mic_muted.store(muted, Ordering::Relaxed);
                }
                SourceMsg::GetMicMute(reply) => {
                    if !reply.is_closed() {
                        let _ = reply.send(st.mic_muted.load(Ordering::Relaxed));
                    }
                }
                SourceMsg::GetMicDevice(reply) => {
                    if !reply.is_closed() {
                        let _ = reply.send(capturing_mic_device(st));
                    }
                }
                SourceMsg::GetMicIsolated(reply) => {
                    let _ = reply.send(st.capture_mic_isolated);
                }
                SourceMsg::GetCaptureHealth(reply) => {
                    let _ = reply.send(st.capture_health);
                }
                SourceMsg::PrepareListenerRefresh(reply) => {
                    st.listener_routing = ListenerRouting::Buffering;
                    let replay = st.pipeline.prepare_listener_refresh();
                    if !reply.is_closed() {
                        let _ = reply.send(replay);
                    }
                }
                SourceMsg::SetListenerRouting(routing) => {
                    st.listener_routing = routing;
                    st.pipeline
                        .on_listener_routing_changed(&st.listener_routing);
                }
                SourceMsg::SetRecorder(recorder) => {
                    st.recorder = recorder;
                }
                SourceMsg::OutputChanged => {
                    if st.current_mode == ChannelMode::SpeakerOnly {
                        myself.stop(Some("device_change".into()));
                    } else if st.current_mode == ChannelMode::MicAndSpeaker {
                        let swapped = st.mic_device.as_ref().map_or(
                            stream::active_mic_device(None, st.audio.as_ref()).is_some(),
                            |requested| st.active_mic_device.as_ref() != Some(requested),
                        );
                        let capture = stream::capture_settings(swapped);
                        st.capture_mic_isolated = capture.mic_isolated;
                        if let Some(output) = &st.capture_output {
                            let _ = output.send(capture.enable_aec);
                        }
                        emit_mic_isolation(st);
                    }
                }
                SourceMsg::InputChanged => {
                    if st.mic_device.is_some() {
                        return Ok(());
                    }
                    st.capture_mic_isolated = mic_isolated(&st.mic_device, st.audio.as_ref());
                    if let Some(retry) = &st.capture_retry {
                        let _ = retry.send((
                            stream::active_mic_device(st.mic_device.clone(), st.audio.as_ref()),
                            true,
                        ));
                    }
                }
                SourceMsg::RetryCapture(device) => {
                    st.mic_device = device.clone();
                    st.capture_mic_isolated = mic_isolated(&st.mic_device, st.audio.as_ref());
                    if let Some(retry) = &st.capture_retry {
                        let _ = retry
                            .send((stream::active_mic_device(device, st.audio.as_ref()), false));
                    }
                }
                SourceMsg::CaptureReady => {
                    st.capture_health.unavailable = false;
                    st.runtime.emit_progress(SessionProgressEvent::AudioReady {
                        session_id: st.session_id.clone(),
                        device: capturing_mic_device(st),
                    });
                    emit_channel_status(st, "audio_capture_ready".into());
                }
                SourceMsg::CaptureUnavailable => {
                    st.healthy_since = None;
                    st.capture_health.unavailable = true;
                    emit_channel_status(st, "audio_capture_unavailable".into());
                }
                SourceMsg::ChannelReady(channel, device) => {
                    match channel {
                        CaptureChannel::Mic => st.capture_health.mic = Some(true),
                        CaptureChannel::Speaker => st.capture_health.speaker = Some(true),
                    }
                    if channel == CaptureChannel::Mic {
                        st.active_mic_device = device;
                        let swapped = st.mic_device.as_ref().map_or(
                            stream::active_mic_device(None, st.audio.as_ref()).is_some(),
                            |requested| st.active_mic_device.as_ref() != Some(requested),
                        );
                        st.capture_mic_isolated = stream::capture_settings(swapped).mic_isolated;
                        emit_mic_isolation(st);
                    }
                    if channel == CaptureChannel::Mic {
                        st.runtime.emit_progress(SessionProgressEvent::AudioReady {
                            session_id: st.session_id.clone(),
                            device: st.active_mic_device.clone(),
                        });
                    }
                    emit_channel_status(
                        st,
                        match channel {
                            CaptureChannel::Mic => "audio_mic_ready",
                            CaptureChannel::Speaker => "audio_speaker_ready",
                        }
                        .into(),
                    );
                }
                SourceMsg::ChannelFailed(channel, reason) => {
                    match channel {
                        CaptureChannel::Mic => st.capture_health.mic = Some(false),
                        CaptureChannel::Speaker => st.capture_health.speaker = Some(false),
                    }
                    tracing::warn!(?channel, %reason, "capture_channel_unavailable");
                    emit_channel_status(
                        st,
                        format!(
                            "{}: {reason}",
                            match channel {
                                CaptureChannel::Mic => "audio_mic_unavailable",
                                CaptureChannel::Speaker => "audio_speaker_unavailable",
                            }
                        ),
                    );
                }
                SourceMsg::CaptureFramesReady => {
                    st.capture_wake_pending.store(false, Ordering::Release);

                    for _ in 0..MAX_CAPTURE_FRAMES_PER_TICK {
                        let frame = st
                            .capture_frames
                            .as_mut()
                            .and_then(|frames| frames.try_recv().ok());
                        let Some(frame) = frame else {
                            break;
                        };
                        let now = std::time::Instant::now();
                        if !frame.captured {
                            st.healthy_since = None;
                            st.last_frame = None;
                        } else {
                            if st.last_frame.is_none_or(|last| {
                                now.duration_since(last) > Duration::from_secs(5)
                            }) {
                                st.healthy_since = Some(now);
                            }
                            st.last_frame = Some(now);
                            if st.healthy_since.is_some_and(|started| {
                                now.duration_since(started) >= Duration::from_secs(30)
                            }) {
                                if let Some(cell) = ractor::registry::where_is(
                                    crate::actors::session_supervisor_name(&st.session_id),
                                ) {
                                    let supervisor: ActorRef<crate::actors::SessionMsg> =
                                        cell.into();
                                    let _ =
                                        supervisor.cast(crate::actors::SessionMsg::SourceHealthy);
                                }
                                st.healthy_since = Some(now);
                            }
                        }
                        st.pipeline
                            .dispatch_frame(
                                frame,
                                st.current_mode,
                                &st.listener_routing,
                                st.recorder.as_ref(),
                            )
                            .await
                            .map_err(std::io::Error::other)?;
                    }

                    let has_queued_frames = st
                        .capture_frames
                        .as_ref()
                        .is_some_and(|frames| !frames.is_empty());
                    if has_queued_frames
                        && !st.capture_wake_pending.swap(true, Ordering::AcqRel)
                        && myself.cast(SourceMsg::CaptureFramesReady).is_err()
                    {
                        return Err(std::io::Error::other(
                            "failed to schedule queued capture frames",
                        )
                        .into());
                    }
                }
                SourceMsg::StreamFailed(reason) => {
                    tracing::warn!(%reason, "source_stream_failed_stopping");
                    st.runtime.emit_error(SessionErrorEvent::AudioError {
                        session_id: st.session_id.clone(),
                        error: reason.clone(),
                        device: st.active_mic_device.clone(),
                        is_fatal: true,
                    });
                    myself.stop(Some(reason));
                }
            }

            Ok(())
        }
        .instrument(span)
        .await
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        st: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        // Drop the watcher before the capture stream so restoring the previous default
        // input cannot be observed as a device_change restart of this source.
        st._device_watcher.take();
        if let Some(cancel_token) = st.stream_cancel_token.take() {
            cancel_token.cancel();
        }
        st.capture_frames.take();
        st.capture_retry.take();
        st.capture_output.take();
        st.pipeline.flush_recorder().await;
        if let Some(task) = st.run_task.take() {
            task.abort();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use futures_util::{StreamExt, stream};
    use ractor::Actor;
    use tokio::sync::{mpsc, oneshot};

    use super::*;
    use crate::{
        SessionDataEvent, SessionLifecycleEvent, SessionProgressEvent,
        actors::source::ListenerRouting,
        actors::{RecMsg, RecorderEnqueueResult},
    };
    use anlg_audio::{CaptureConfig, CaptureEvent, CaptureFrame, CaptureStream, Error};

    struct TestRuntime {
        progress_tx: mpsc::UnboundedSender<SessionProgressEvent>,
        error_tx: Option<mpsc::UnboundedSender<SessionErrorEvent>>,
    }

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

        fn emit_progress(&self, event: SessionProgressEvent) {
            let _ = self.progress_tx.send(event);
        }

        fn emit_error(&self, event: SessionErrorEvent) {
            if let Some(error_tx) = &self.error_tx {
                let _ = error_tx.send(event);
            }
        }

        fn emit_data(&self, _event: SessionDataEvent) {}
    }

    struct TestAudio {
        capture_tx: mpsc::UnboundedSender<Option<String>>,
        default_device_name_calls: AtomicUsize,
        end_immediately: bool,
        emit_frame: bool,
        partial_audio: bool,
        gap_only: bool,
        output_tx: Option<mpsc::UnboundedSender<bool>>,
    }

    impl AudioProvider for TestAudio {
        fn open_capture(&self, config: CaptureConfig) -> Result<CaptureStream, Error> {
            let _ = self.capture_tx.send(config.mic_device);
            if self.gap_only {
                let silence = Arc::from(vec![0.0; config.chunk_size]);
                return Ok(CaptureStream::with_events(
                    stream::iter([Ok(CaptureEvent::Gap(CaptureFrame {
                        raw_mic: Arc::clone(&silence),
                        raw_speaker: silence,
                        aec_mic: None,
                    }))])
                    .chain(stream::pending()),
                ));
            }
            if self.partial_audio {
                let samples = (0..16_000)
                    .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / 16_000.0).sin() * 0.25)
                    .collect::<Vec<_>>();
                return Ok(CaptureStream::with_events(
                    stream::iter([
                        Ok(CaptureEvent::ChannelFailed {
                            channel: CaptureChannel::Mic,
                            error: Error::MicOpenFailed,
                        }),
                        Ok(CaptureEvent::ChannelReady {
                            channel: CaptureChannel::Speaker,
                            device: None,
                        }),
                        Ok(CaptureEvent::Frame(CaptureFrame {
                            raw_mic: Arc::from(vec![0.0; samples.len()]),
                            raw_speaker: Arc::from(samples),
                            aec_mic: None,
                        })),
                    ])
                    .chain(stream::pending()),
                ));
            }
            if self.end_immediately {
                Ok(CaptureStream::new(stream::empty()))
            } else if self.emit_frame {
                let frame = CaptureFrame {
                    raw_mic: Arc::from(vec![0.0; config.chunk_size]),
                    raw_speaker: Arc::from(vec![0.0; config.chunk_size]),
                    aec_mic: None,
                };
                let output_tx = self.output_tx.clone();
                Ok(
                    CaptureStream::new(stream::iter([Ok(frame)]).chain(stream::pending()))
                        .with_output_change(move |enabled| {
                            if let Some(output_tx) = &output_tx {
                                let _ = output_tx.send(enabled);
                            }
                        }),
                )
            } else {
                Ok(CaptureStream::new(stream::pending()))
            }
        }

        fn open_speaker_capture(
            &self,
            _sample_rate: u32,
            _chunk_size: usize,
        ) -> Result<CaptureStream, Error> {
            unreachable!()
        }

        fn open_mic_capture(
            &self,
            _device: Option<String>,
            _sample_rate: u32,
            _chunk_size: usize,
        ) -> Result<CaptureStream, Error> {
            unreachable!()
        }

        fn default_device_name(&self) -> String {
            self.default_device_name_calls
                .fetch_add(1, Ordering::Relaxed);
            "system-default".to_string()
        }

        fn list_mic_devices(&self) -> Vec<String> {
            vec![]
        }

        fn play_silence(&self) -> std::sync::mpsc::Sender<()> {
            let (tx, _rx) = std::sync::mpsc::channel();
            tx
        }

        fn play_bytes(&self, _bytes: &'static [u8]) -> std::sync::mpsc::Sender<()> {
            let (tx, _rx) = std::sync::mpsc::channel();
            tx
        }

        fn probe_mic(&self, _device: Option<String>) -> Result<(), Error> {
            Ok(())
        }

        fn probe_speaker(&self) -> Result<(), Error> {
            Ok(())
        }
    }

    async fn assert_source_uses_mic_device(mic_device: Option<&str>) {
        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
        let (capture_tx, mut capture_rx) = mpsc::unbounded_channel();
        let audio = Arc::new(TestAudio {
            capture_tx,
            default_device_name_calls: AtomicUsize::new(0),
            end_immediately: false,
            emit_frame: true,
            partial_audio: false,
            gap_only: false,
            output_tx: None,
        });
        let expected = mic_device.map(str::to_string);
        let (actor, handle) = Actor::spawn(
            None,
            SourceActor,
            SourceArgs {
                mic_device: expected.clone(),
                onboarding: false,
                runtime: Arc::new(TestRuntime {
                    progress_tx,
                    error_tx: None,
                }),
                audio: audio.clone(),
                session_id: "test-session".to_string(),
                listener_routing: ListenerRouting::Dropped,
                recorder: None,
            },
        )
        .await
        .unwrap();

        let captured = tokio::time::timeout(std::time::Duration::from_secs(1), capture_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(captured, expected);

        let ready_device = loop {
            let event = tokio::time::timeout(std::time::Duration::from_secs(1), progress_rx.recv())
                .await
                .unwrap()
                .unwrap();
            if let SessionProgressEvent::AudioReady { device, .. } = event {
                break device;
            }
        };
        assert_eq!(ready_device, expected);
        assert_eq!(audio.default_device_name_calls.load(Ordering::Relaxed), 0);

        actor.stop(None);
        let _ = handle.await;
    }

    #[tokio::test]
    async fn opening_without_frames_never_reports_ready_and_surfaces_capture_failure() {
        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
        let (error_tx, mut error_rx) = mpsc::unbounded_channel();
        let (capture_tx, _capture_rx) = mpsc::unbounded_channel();
        let (actor, handle) = Actor::spawn(
            None,
            SourceActor,
            SourceArgs {
                mic_device: None,
                onboarding: false,
                runtime: Arc::new(TestRuntime {
                    progress_tx,
                    error_tx: Some(error_tx),
                }),
                audio: Arc::new(TestAudio {
                    capture_tx,
                    default_device_name_calls: AtomicUsize::new(0),
                    end_immediately: false,
                    emit_frame: false,
                    partial_audio: false,
                    gap_only: true,
                    output_tx: None,
                }),
                session_id: "no-frames".into(),
                listener_routing: ListenerRouting::Dropped,
                recorder: None,
            },
        )
        .await
        .unwrap();
        assert!(matches!(
            progress_rx.recv().await,
            Some(SessionProgressEvent::AudioInitializing { .. })
        ));
        assert!(
            matches!(tokio::time::timeout(Duration::from_secs(6), error_rx.recv()).await.unwrap(), Some(SessionErrorEvent::AudioError { error, is_fatal: false, .. }) if error == "audio_capture_unavailable")
        );
        assert!(progress_rx.try_recv().is_err());
        actor.stop(None);
        handle.await.unwrap();
    }

    #[test]
    fn bluetooth_handoff_device_switches_do_not_interrupt_capture() {
        assert!(device_switch_updates_capture(
            &DeviceSwitch::DefaultInputChanged,
            false
        ));
        assert!(device_switch_updates_capture(
            &DeviceSwitch::DefaultOutputChanged {
                headphone: Some(true)
            },
            false
        ));
        assert!(!device_switch_updates_capture(
            &DeviceSwitch::DefaultInputChanged,
            true
        ));
        assert!(!device_switch_updates_capture(
            &DeviceSwitch::DefaultOutputChanged {
                headphone: Some(true)
            },
            true
        ));
        assert!(!device_switch_updates_capture(
            &DeviceSwitch::DeviceListChanged,
            false
        ));
    }

    #[test]
    fn output_routing_changes_are_debounced_across_two_polls() {
        let mut tracker = OutputRoutingTracker::new(true);

        assert!(!tracker.observe(true));
        assert!(!tracker.observe(false));
        assert!(tracker.observe(false));

        let mut tracker = OutputRoutingTracker::new(true);
        assert!(!tracker.observe(false));
        assert!(!tracker.observe(true));
        assert!(!tracker.observe(false));

        let mut tracker = OutputRoutingTracker::new(true);
        assert!(!tracker.observe(false));
        assert!(tracker.observe(false));
        assert!(!tracker.observe(false));
        assert!(!tracker.observe(true));
        assert!(tracker.observe(true));
    }

    #[tokio::test]
    async fn output_change_updates_capture_without_reopening_the_microphone() {
        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
        let (capture_tx, mut capture_rx) = mpsc::unbounded_channel();
        let (output_tx, mut output_rx) = mpsc::unbounded_channel();
        let (actor, handle) = Actor::spawn(
            None,
            SourceActor,
            SourceArgs {
                mic_device: Some("working mic".into()),
                onboarding: false,
                runtime: Arc::new(TestRuntime {
                    progress_tx,
                    error_tx: None,
                }),
                audio: Arc::new(TestAudio {
                    capture_tx,
                    default_device_name_calls: AtomicUsize::new(0),
                    end_immediately: false,
                    emit_frame: true,
                    partial_audio: false,
                    gap_only: false,
                    output_tx: Some(output_tx),
                }),
                session_id: "output-change".into(),
                listener_routing: ListenerRouting::Dropped,
                recorder: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(capture_rx.recv().await.unwrap(), Some("working mic".into()));
        while !matches!(
            progress_rx.recv().await,
            Some(SessionProgressEvent::AudioReady { .. })
        ) {}
        actor.cast(SourceMsg::OutputChanged).unwrap();
        tokio::time::timeout(Duration::from_secs(1), output_rx.recv())
            .await
            .unwrap()
            .unwrap();
        let device = actor
            .call(SourceMsg::GetMicDevice, Some(Duration::from_secs(1)))
            .await
            .unwrap();
        assert!(
            matches!(device, ractor::rpc::CallResult::Success(Some(device)) if device == "working mic")
        );
        assert!(capture_rx.try_recv().is_err());
        actor.stop(None);
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn source_opens_requested_or_default_mic() {
        assert_source_uses_mic_device(None).await;
        assert_source_uses_mic_device(Some("external-mic")).await;
    }

    #[tokio::test]
    async fn capture_stream_eof_reports_a_restartable_failure() {
        let (progress_tx, _progress_rx) = mpsc::unbounded_channel();
        let (error_tx, mut error_rx) = mpsc::unbounded_channel();
        let (capture_tx, _capture_rx) = mpsc::unbounded_channel();
        let audio = Arc::new(TestAudio {
            capture_tx,
            default_device_name_calls: AtomicUsize::new(0),
            end_immediately: true,
            emit_frame: false,
            partial_audio: false,
            gap_only: false,
            output_tx: None,
        });
        let (_actor, handle) = Actor::spawn(
            None,
            SourceActor,
            SourceArgs {
                mic_device: None,
                onboarding: false,
                runtime: Arc::new(TestRuntime {
                    progress_tx,
                    error_tx: Some(error_tx),
                }),
                audio,
                session_id: "finite-stream".to_string(),
                listener_routing: ListenerRouting::Dropped,
                recorder: None,
            },
        )
        .await
        .unwrap();

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), error_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            event,
            SessionErrorEvent::AudioError {
                error,
                is_fatal: true,
                ..
            } if error == "capture stream ended unexpectedly"
        ));

        tokio::time::timeout(std::time::Duration::from_secs(1), handle)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn microphone_failure_still_saves_and_transcribes_system_audio() {
        let dir = tempfile::tempdir().unwrap();
        let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
        let runtime: Arc<dyn ListenerRuntime> = Arc::new(TestRuntime {
            progress_tx,
            error_tx: None,
        });
        // Recording tests share the production writer limit; wait for a fixture slot.
        let (recorder, recorder_handle) = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                match Actor::spawn(
                    None,
                    crate::actors::RecorderActor,
                    crate::actors::RecArgs {
                        runtime: runtime.clone(),
                        app_dir: dir.path().into(),
                        retain_audio: true,
                        capture_started_at: 1,
                        offset_ms: 0,
                        session_id: "partial-audio".into(),
                    },
                )
                .await
                {
                    Ok(recorder) => break recorder,
                    Err(error) if error.to_string().contains("too many recorder writer jobs") => {
                        tokio::time::sleep(Duration::from_millis(25)).await;
                    }
                    Err(error) => panic!("{error}"),
                }
            }
        })
        .await
        .unwrap();
        let (live_tx, mut live_rx) = mpsc::unbounded_channel();
        let (listener, listener_handle) = Actor::spawn(None, AudioListenerProbe(live_tx), ())
            .await
            .unwrap();
        let (capture_tx, _capture_rx) = mpsc::unbounded_channel();
        let (source, source_handle) = Actor::spawn(
            None,
            SourceActor,
            SourceArgs {
                mic_device: Some("unavailable mic".into()),
                onboarding: false,
                runtime,
                audio: Arc::new(TestAudio {
                    capture_tx,
                    default_device_name_calls: AtomicUsize::new(0),
                    end_immediately: false,
                    emit_frame: false,
                    partial_audio: true,
                    gap_only: false,
                    output_tx: None,
                }),
                session_id: "partial-audio".into(),
                listener_routing: ListenerRouting::Attached(listener.clone()),
                recorder: Some(recorder.clone()),
            },
        )
        .await
        .unwrap();
        let (live_mic, live_speaker) = tokio::time::timeout(Duration::from_secs(3), live_rx.recv())
            .await
            .unwrap()
            .unwrap();
        let ready_device = loop {
            if let Some(SessionProgressEvent::AudioReady { device, .. }) = progress_rx.recv().await
            {
                break device;
            }
        };
        assert_eq!(ready_device, None);
        assert_eq!(live_mic.len(), 32_000);
        assert!(live_mic.iter().all(|byte| *byte == 0));
        let expected_speaker = (0..16_000)
            .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / 16_000.0).sin() * 0.25)
            .collect::<Vec<_>>();
        assert_eq!(
            live_speaker,
            anlg_audio_utils::f32_to_i16_bytes(expected_speaker.into_iter())
        );
        let health = source
            .call(SourceMsg::GetCaptureHealth, Some(Duration::from_secs(1)))
            .await
            .unwrap();
        assert!(matches!(
            health,
            ractor::rpc::CallResult::Success(CaptureHealth {
                mic: Some(false),
                speaker: Some(true),
                unavailable: false,
            })
        ));
        source.stop(None);
        tokio::time::timeout(Duration::from_secs(3), source_handle)
            .await
            .unwrap()
            .unwrap();
        listener.stop(None);
        listener_handle.await.unwrap();
        recorder.stop(None);
        tokio::time::timeout(Duration::from_secs(3), recorder_handle)
            .await
            .unwrap()
            .unwrap();
        let wav = dir.path().join("decoded.wav");
        anlg_mp3::decode_to_wav(&dir.path().join("partial-audio/audio.mp3"), &wav).unwrap();
        let mut reader = hound::WavReader::open(wav).unwrap();
        assert_eq!(reader.spec().channels, 2);
        assert!(reader.duration() >= 16_000);
        let samples = reader
            .samples::<f32>()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        let mic_energy = samples
            .iter()
            .step_by(2)
            .map(|sample| (*sample as f64).powi(2))
            .sum::<f64>();
        let speaker_energy = samples
            .iter()
            .skip(1)
            .step_by(2)
            .map(|sample| (*sample as f64).powi(2))
            .sum::<f64>();
        assert!(speaker_energy > 100.0);
        assert!(mic_energy < speaker_energy * 0.01);
    }

    struct AudioListenerProbe(mpsc::UnboundedSender<(bytes::Bytes, bytes::Bytes)>);

    #[ractor::async_trait]
    impl Actor for AudioListenerProbe {
        type Msg = ListenerMsg;
        type State = ();
        type Arguments = ();

        async fn pre_start(
            &self,
            _myself: ActorRef<Self::Msg>,
            _args: (),
        ) -> Result<(), ActorProcessingErr> {
            Ok(())
        }

        async fn handle(
            &self,
            _myself: ActorRef<Self::Msg>,
            msg: Self::Msg,
            _state: &mut (),
        ) -> Result<(), ActorProcessingErr> {
            if let ListenerMsg::AudioDual(mic, speaker, reply) = msg {
                self.0.send((mic, speaker)).unwrap();
                let _ = reply.send(crate::actors::ListenerAudioResult::Accepted);
            }
            Ok(())
        }
    }

    struct BlockingRecorder;

    struct BlockingRecorderState {
        started_tx: Option<oneshot::Sender<()>>,
        release_rx: Option<oneshot::Receiver<()>>,
    }

    #[ractor::async_trait]
    impl Actor for BlockingRecorder {
        type Msg = RecMsg;
        type State = BlockingRecorderState;
        type Arguments = (oneshot::Sender<()>, oneshot::Receiver<()>);

        async fn pre_start(
            &self,
            _myself: ActorRef<Self::Msg>,
            (started_tx, release_rx): Self::Arguments,
        ) -> Result<Self::State, ActorProcessingErr> {
            Ok(BlockingRecorderState {
                started_tx: Some(started_tx),
                release_rx: Some(release_rx),
            })
        }

        async fn handle(
            &self,
            _myself: ActorRef<Self::Msg>,
            message: Self::Msg,
            state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            let reply = match message {
                RecMsg::AudioSingle(_, reply) | RecMsg::AudioDual(_, _, reply) => reply,
                RecMsg::WriterFailed(_) => return Ok(()),
            };

            if let Some(started_tx) = state.started_tx.take() {
                let _ = started_tx.send(());
            }
            if let Some(release_rx) = state.release_rx.take() {
                let _ = release_rx.await;
            }
            let _ = reply.send(RecorderEnqueueResult::Accepted);
            Ok(())
        }
    }

    #[test]
    fn source_dispatch_exits_session_span_while_awaiting_recorder() {
        tracing::subscriber::with_default(tracing_subscriber::registry(), || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (started_tx, started_rx) = oneshot::channel();
                    let (release_tx, release_rx) = oneshot::channel();
                    let (recorder, recorder_handle) =
                        Actor::spawn(None, BlockingRecorder, (started_tx, release_rx))
                            .await
                            .unwrap();
                    let (progress_tx, _progress_rx) = mpsc::unbounded_channel();
                    let (capture_tx, _capture_rx) = mpsc::unbounded_channel();
                    let audio = Arc::new(TestAudio {
                        capture_tx,
                        default_device_name_calls: AtomicUsize::new(0),
                        end_immediately: false,
                        emit_frame: true,
                        partial_audio: false,
                        gap_only: false,
                        output_tx: None,
                    });
                    let (source, source_handle) = Actor::spawn(
                        None,
                        SourceActor,
                        SourceArgs {
                            mic_device: None,
                            onboarding: false,
                            runtime: Arc::new(TestRuntime {
                                progress_tx,
                                error_tx: None,
                            }),
                            audio,
                            session_id: "span-test".to_string(),
                            listener_routing: ListenerRouting::Dropped,
                            recorder: Some(recorder.clone()),
                        },
                    )
                    .await
                    .unwrap();

                    tokio::time::timeout(std::time::Duration::from_secs(1), started_rx)
                        .await
                        .unwrap()
                        .unwrap();
                    assert!(tracing::Span::current().is_none());

                    let _ = release_tx.send(());
                    source.stop(None);
                    recorder.stop(None);
                    tokio::time::timeout(std::time::Duration::from_secs(1), source_handle)
                        .await
                        .unwrap()
                        .unwrap();
                    tokio::time::timeout(std::time::Duration::from_secs(1), recorder_handle)
                        .await
                        .unwrap()
                        .unwrap();
                });
        });
    }
}
