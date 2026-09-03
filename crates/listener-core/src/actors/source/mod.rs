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
use anlg_audio::{AudioProvider, CaptureFrame};

use pipeline::{DispatchFrameResult, Pipeline};
use stream::start_source_loop;

use anlg_device_monitor::{DeviceMonitorHandle, DeviceSwitch, DeviceSwitchMonitor};

pub enum SourceMsg {
    SetMicMute(bool),
    GetMicMute(RpcReplyPort<bool>),
    GetMicDevice(RpcReplyPort<Option<String>>),
    PrepareListenerRefresh(RpcReplyPort<ListenerRefreshReplay>),
    SetListenerRouting(ListenerRouting),
    SetRecorder(Option<ActorRef<RecMsg>>),
    CaptureFramesReady,
    StreamFailed(String),
}

pub struct SourceFrame {
    pub capture: CaptureFrame,
    pub mic_muted: bool,
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
    pub(super) mic_device: Option<String>,
    pub(super) onboarding: bool,
    pub(super) mic_muted: Arc<AtomicBool>,
    pub(super) run_task: Option<tokio::task::JoinHandle<()>>,
    pub(super) stream_cancel_token: Option<CancellationToken>,
    pub(super) capture_frames: Option<tokio::sync::mpsc::Receiver<SourceFrame>>,
    pub(super) capture_wake_pending: Arc<AtomicBool>,
    pub(super) current_mode: ChannelMode,
    pub(super) pipeline: Pipeline,
    pub(super) listener_routing: ListenerRouting,
    pub(super) recorder: Option<ActorRef<RecMsg>>,
    _device_watcher: Option<DeviceChangeWatcher>,
    _silence_stream_tx: Option<std::sync::mpsc::Sender<()>>,
}

pub struct SourceActor;

const MAX_CAPTURE_FRAMES_PER_TICK: usize = 4;
const RECORDER_BACKPRESSURE_RETRY_DELAY: Duration = Duration::from_millis(10);
const OUTPUT_ROUTING_POLL_INTERVAL: Duration = Duration::from_secs(2);
// Only the macOS backend reports which outputs are running; elsewhere the verdict can only move
// with the default output, which already restarts the source.
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
                Ok(DeviceSwitch::DefaultInputChanged) => {
                    tracing::info!("default_input_changed_restarting_source");
                    actor.stop(Some("device_change".to_string()));
                }
                Ok(DeviceSwitch::DefaultOutputChanged { .. }) => {
                    tracing::info!("default_output_changed_restarting_source");
                    actor.stop(Some("device_change".to_string()));
                }
                Ok(DeviceSwitch::DeviceListChanged) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let Some(routing) = routing.as_mut() else {
                        continue;
                    };
                    let observed = headphone_only_output();
                    if routing.observe(observed) {
                        tracing::info!(
                            headphone_output = observed,
                            "output_routing_changed_restarting_source"
                        );
                        actor.stop(Some("device_change".to_string()));
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

// A meeting app can start playing through speakers after capture began, flipping the AEC and
// mic-isolation verdict the streams were opened with. Requiring two consecutive polls keeps a
// one-off system sound from bouncing the source.
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

impl SourceActor {
    pub fn name() -> ActorName {
        "source".into()
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
                onboarding: args.onboarding,
                mic_muted: Arc::new(AtomicBool::new(false)),
                run_task: None,
                stream_cancel_token: None,
                capture_frames: None,
                capture_wake_pending: Arc::new(AtomicBool::new(false)),
                _device_watcher: None,
                _silence_stream_tx: silence_stream_tx,
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
                capture.mic_isolated,
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
                        let _ = reply.send(st.mic_device.clone());
                    }
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
                SourceMsg::CaptureFramesReady => {
                    st.capture_wake_pending.store(false, Ordering::Release);

                    let pending_result = st
                        .pipeline
                        .retry_pending_recorder(&st.listener_routing, st.recorder.as_ref())
                        .await;
                    let mut recorder_backpressured = match pending_result {
                        Ok(DispatchFrameResult::Complete) => false,
                        Ok(DispatchFrameResult::RecorderBackpressured) => true,
                        Err(reason) => return recorder_failure(st, reason),
                    };

                    if !recorder_backpressured {
                        for _ in 0..MAX_CAPTURE_FRAMES_PER_TICK {
                            let frame = st
                                .capture_frames
                                .as_mut()
                                .and_then(|frames| frames.try_recv().ok());
                            let Some(frame) = frame else {
                                break;
                            };

                            match st
                                .pipeline
                                .dispatch_frame(
                                    frame,
                                    st.current_mode,
                                    &st.listener_routing,
                                    st.recorder.as_ref(),
                                )
                                .await
                            {
                                Ok(DispatchFrameResult::Complete) => {}
                                Ok(DispatchFrameResult::RecorderBackpressured) => {
                                    recorder_backpressured = true;
                                    break;
                                }
                                Err(reason) => return recorder_failure(st, reason),
                            }
                        }
                    }

                    let has_queued_frames = st
                        .capture_frames
                        .as_ref()
                        .is_some_and(|frames| !frames.is_empty());
                    if recorder_backpressured || st.pipeline.has_pending_recorder_item() {
                        if !st.capture_wake_pending.swap(true, Ordering::AcqRel) {
                            myself.send_after(RECORDER_BACKPRESSURE_RETRY_DELAY, || {
                                SourceMsg::CaptureFramesReady
                            });
                        }
                    } else if has_queued_frames
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
                        device: st.mic_device.clone(),
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
        if let Some(cancel_token) = st.stream_cancel_token.take() {
            cancel_token.cancel();
        }
        st.capture_frames.take();
        if let Some(task) = st.run_task.take() {
            task.abort();
        }

        Ok(())
    }
}

fn recorder_failure(state: &SourceState, reason: String) -> Result<(), ActorProcessingErr> {
    tracing::error!(%reason, "recorder_audio_write_failed");
    state.runtime.emit_error(SessionErrorEvent::AudioError {
        session_id: state.session_id.clone(),
        error: reason.clone(),
        device: state.mic_device.clone(),
        is_fatal: true,
    });
    Err(std::io::Error::other(reason).into())
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
    use anlg_audio::{CaptureConfig, CaptureFrame, CaptureStream, Error};

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
    }

    impl AudioProvider for TestAudio {
        fn open_capture(&self, config: CaptureConfig) -> Result<CaptureStream, Error> {
            let _ = self.capture_tx.send(config.mic_device);
            if self.end_immediately {
                Ok(CaptureStream::new(stream::empty()))
            } else if self.emit_frame {
                let frame = CaptureFrame {
                    raw_mic: Arc::from(vec![0.0; config.chunk_size]),
                    raw_speaker: Arc::from(vec![0.0; config.chunk_size]),
                    aec_mic: None,
                };
                Ok(CaptureStream::new(
                    stream::iter([Ok(frame)]).chain(stream::pending()),
                ))
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
            emit_frame: false,
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

    #[test]
    fn output_routing_restart_needs_two_consecutive_flipped_polls() {
        let mut tracker = OutputRoutingTracker::new(true);

        assert!(!tracker.observe(true));
        assert!(!tracker.observe(false));
        assert!(tracker.observe(false));
    }

    #[test]
    fn output_routing_blip_does_not_restart() {
        let mut tracker = OutputRoutingTracker::new(true);

        assert!(!tracker.observe(false));
        assert!(!tracker.observe(true));
        assert!(!tracker.observe(false));
    }

    #[test]
    fn output_routing_tracks_the_new_verdict_after_firing() {
        let mut tracker = OutputRoutingTracker::new(true);

        assert!(!tracker.observe(false));
        assert!(tracker.observe(false));
        assert!(!tracker.observe(false));
        assert!(!tracker.observe(true));
        assert!(tracker.observe(true));
    }

    #[tokio::test]
    async fn unspecified_mic_uses_capture_provider_default() {
        assert_source_uses_mic_device(None).await;
    }

    #[tokio::test]
    async fn explicit_mic_selection_is_preserved() {
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
