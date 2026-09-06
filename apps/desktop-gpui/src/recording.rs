//! The capture engine: Tauri's `transcription` plugin is a thin shell around
//! `listener-core`'s `RootActor`, so the GPUI app drives the very same actor
//! tree (source → recorder → listener) and forwards its events to the
//! workspace the way `TauriRuntime` forwards them to the webview.

use std::path::PathBuf;
use std::sync::Arc;

use anlg_audio::AudioProvider;
use anlg_listener_core::actors::{RootActor, RootArgs, RootMsg, SessionParams};
use anlg_listener_core::{
    ListenerRuntime, SessionDataEvent, SessionErrorEvent, SessionLifecycleEvent,
    SessionProgressEvent, StartSessionError,
};
use ractor::{Actor, ActorRef};

pub enum Event {
    Lifecycle(SessionLifecycleEvent),
    Progress(SessionProgressEvent),
    Error(SessionErrorEvent),
    Data(SessionDataEvent),
}

/// `TauriRuntime`: the storage roots plus the event bridge.
struct Runtime {
    base: PathBuf,
    events: tokio::sync::mpsc::UnboundedSender<Event>,
}

impl anlg_storage::StorageRuntime for Runtime {
    fn global_base(&self) -> Result<PathBuf, anlg_storage::Error> {
        Ok(self.base.clone())
    }

    fn vault_base(&self) -> Result<PathBuf, anlg_storage::Error> {
        Ok(self.base.clone())
    }
}

impl ListenerRuntime for Runtime {
    fn emit_lifecycle(&self, event: SessionLifecycleEvent) {
        let _ = self.events.send(Event::Lifecycle(event));
    }

    fn emit_progress(&self, event: SessionProgressEvent) {
        let _ = self.events.send(Event::Progress(event));
    }

    fn emit_error(&self, event: SessionErrorEvent) {
        let _ = self.events.send(Event::Error(event));
    }

    fn emit_data(&self, event: SessionDataEvent) {
        let _ = self.events.send(Event::Data(event));
    }
}

pub struct Recorder {
    runtime: tokio::runtime::Handle,
    root: ActorRef<RootMsg>,
}

impl Recorder {
    /// Spawns the root actor on the tokio runtime; `base` is the folder
    /// holding `app.db` (the vault and global storage root).
    pub async fn spawn(
        runtime: tokio::runtime::Handle,
        base: PathBuf,
        audio: Arc<dyn AudioProvider>,
    ) -> anyhow::Result<(Self, tokio::sync::mpsc::UnboundedReceiver<Event>)> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let listener_runtime: Arc<dyn ListenerRuntime> = Arc::new(Runtime { base, events: tx });
        let root = runtime
            .spawn(async move {
                Actor::spawn(
                    Some(RootActor::name()),
                    RootActor,
                    RootArgs {
                        runtime: listener_runtime,
                        audio,
                    },
                )
                .await
                .map(|(actor, _handle)| actor)
            })
            .await??;
        Ok((Self { runtime, root }, rx))
    }

    /// `start_capture`
    pub fn start(
        &self,
        params: SessionParams,
    ) -> tokio::task::JoinHandle<Result<(), StartSessionError>> {
        let root = self.root.clone();
        self.runtime.spawn(async move {
            match ractor::call!(root, RootMsg::StartSession, params) {
                Ok(result) => result,
                Err(_) => Err(StartSessionError::FailedToStartSession),
            }
        })
    }

    /// `stop_capture`
    pub fn stop(&self) -> tokio::task::JoinHandle<()> {
        let root = self.root.clone();
        self.runtime.spawn(async move {
            let _ = ractor::call!(root, RootMsg::StopSession);
        })
    }
}
