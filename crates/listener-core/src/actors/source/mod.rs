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
    actors::{AudioChunk, ChannelMode},
};
use hypr_audio::AudioInput;

use pipeline::Pipeline;
use stream::start_source_loop;

use hypr_device_monitor::{DeviceMonitorHandle, DeviceSwitch, DeviceSwitchMonitor};

pub enum SourceMsg {
    SetMicMute(bool),
    GetMicMute(RpcReplyPort<bool>),
    GetMicDevice(RpcReplyPort<Option<String>>),
    MicChunk(AudioChunk),
    SpeakerChunk(AudioChunk),
    StreamFailed(String),
}

pub struct SourceArgs {
    pub mic_device: Option<String>,
    pub onboarding: bool,
    pub runtime: Arc<dyn ListenerRuntime>,
    pub session_id: String,
}

pub struct SourceState {
    pub(super) runtime: Arc<dyn ListenerRuntime>,
    pub(super) session_id: String,
    pub(super) mic_device: Option<String>,
    pub(super) onboarding: bool,
    pub(super) mic_muted: Arc<AtomicBool>,
    pub(super) run_task: Option<tokio::task::JoinHandle<()>>,
    pub(super) stream_cancel_token: Option<CancellationToken>,
    pub(super) current_mode: ChannelMode,
    pub(super) pipeline: Pipeline,
    _device_watcher: Option<DeviceChangeWatcher>,
    _silence_stream_tx: Option<std::sync::mpsc::Sender<()>>,
}

pub struct SourceActor;

struct DeviceChangeWatcher {
    _handle: DeviceMonitorHandle,
    _thread: std::thread::JoinHandle<()>,
}

impl DeviceChangeWatcher {
    fn spawn(actor: ActorRef<SourceMsg>) -> Self {
        let (event_tx, event_rx) = mpsc::channel();
        let handle = DeviceSwitchMonitor::spawn_debounced(event_tx);
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

            let silence_stream_tx = Some(hypr_audio::AudioOutput::silence());
            let mic_device = args
                .mic_device
                .or_else(|| Some(AudioInput::get_default_device_name()));
            tracing::info!(mic_device = ?mic_device);

            let pipeline = Pipeline::new(args.runtime.clone(), args.session_id.clone());

            let mut st = SourceState {
                runtime: args.runtime,
                session_id: args.session_id,
                mic_device,
                onboarding: args.onboarding,
                mic_muted: Arc::new(AtomicBool::new(false)),
                run_task: None,
                stream_cancel_token: None,
                _device_watcher: Some(device_watcher),
                _silence_stream_tx: silence_stream_tx,
                current_mode: ChannelMode::MicAndSpeaker,
                pipeline,
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
            SourceMsg::MicChunk(chunk) => {
                st.pipeline.ingest_mic(chunk);
                st.pipeline.flush(st.current_mode);
            }
            SourceMsg::SpeakerChunk(chunk) => {
                st.pipeline.ingest_speaker(chunk);
                st.pipeline.flush(st.current_mode);
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
        if let Some(task) = st.run_task.take() {
            task.abort();
        }

        Ok(())
    }
}
