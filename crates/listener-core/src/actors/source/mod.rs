mod pipeline;
mod stream;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
};

use ractor::{Actor, ActorName, ActorProcessingErr, ActorRef, RpcReplyPort};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use crate::{
    ListenerRuntime, SessionErrorEvent, SessionProgressEvent,
    actors::session::session_span,
    actors::{ChannelMode, ListenerMsg, RecMsg},
};
use anlg_audio::{AudioProvider, CaptureFrame};

use pipeline::Pipeline;
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

struct DeviceChangeWatcher {
    _handle: DeviceMonitorHandle,
    _thread: std::thread::JoinHandle<()>,
}

impl DeviceChangeWatcher {
    fn spawn(actor: ActorRef<SourceMsg>) -> Self {
        let (event_tx, event_rx) = mpsc::sync_channel(1);
        let handle = DeviceSwitchMonitor::spawn_debounced_bounded(event_tx);
        let thread = std::thread::spawn(move || Self::event_loop(event_rx, actor));

        Self {
            _handle: handle,
            _thread: thread,
        }
    }

    fn event_loop(event_rx: Receiver<DeviceSwitch>, actor: ActorRef<SourceMsg>) {
        loop {
            match event_rx.recv() {
                Ok(DeviceSwitch::DefaultInputChanged) => {
                    tracing::info!("default_input_changed_restarting_source");
                    actor.stop(Some("device_change".to_string()));
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
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

            let device_watcher = DeviceChangeWatcher::spawn(myself.clone());

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
                _device_watcher: Some(device_watcher),
                _silence_stream_tx: silence_stream_tx,
                current_mode: ChannelMode::MicAndSpeaker,
                pipeline,
                listener_routing: args.listener_routing,
                recorder: args.recorder,
            };

            start_source_loop(&myself, &mut st).await?;
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
        let _guard = span.enter();

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

                for _ in 0..MAX_CAPTURE_FRAMES_PER_TICK {
                    let frame = st
                        .capture_frames
                        .as_mut()
                        .and_then(|frames| frames.try_recv().ok());
                    let Some(frame) = frame else {
                        break;
                    };

                    if let Err(reason) = st
                        .pipeline
                        .dispatch_frame(
                            frame,
                            st.current_mode,
                            &st.listener_routing,
                            st.recorder.as_ref(),
                        )
                        .await
                    {
                        tracing::error!(%reason, "recorder_audio_write_failed");
                        st.runtime.emit_error(SessionErrorEvent::AudioError {
                            session_id: st.session_id.clone(),
                            error: reason.clone(),
                            device: st.mic_device.clone(),
                            is_fatal: true,
                        });
                        return Err(std::io::Error::other(reason).into());
                    }
                }

                let has_more = st
                    .capture_frames
                    .as_ref()
                    .is_some_and(|frames| !frames.is_empty());
                if has_more
                    && !st.capture_wake_pending.swap(true, Ordering::AcqRel)
                    && myself.cast(SourceMsg::CaptureFramesReady).is_err()
                {
                    return Err(
                        std::io::Error::other("failed to schedule queued capture frames").into(),
                    );
                }
            }
            SourceMsg::StreamFailed(reason) => {
                tracing::error!(%reason, "source_stream_failed_stopping");
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

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use futures_util::stream;
    use ractor::Actor;
    use tokio::sync::mpsc;

    use super::*;
    use crate::{
        SessionDataEvent, SessionLifecycleEvent, SessionProgressEvent,
        actors::source::ListenerRouting,
    };
    use anlg_audio::{CaptureConfig, CaptureStream, Error};

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
    }

    impl AudioProvider for TestAudio {
        fn open_capture(&self, config: CaptureConfig) -> Result<CaptureStream, Error> {
            let _ = self.capture_tx.send(config.mic_device);
            if self.end_immediately {
                Ok(CaptureStream::new(stream::empty()))
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
}
