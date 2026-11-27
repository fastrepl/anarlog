use std::collections::{BTreeMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anlg_audio::AudioProvider;
use anlg_listener_core::{
    DegradedError, ListenerRuntime, LiveTranscriptSegment, SessionDataEvent, SessionErrorEvent,
    SessionLifecycleEvent, SessionProgressEvent, TranscriptionMode,
    actors::{RootActor, RootArgs, RootMsg, SessionParams},
};
use anlg_storage::StorageRuntime;
use desktop_runtime::{Result, RuntimeHandle, ServiceError, SessionId};
use futures::future::BoxFuture;
use ractor::{Actor, ActorRef};
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};

use super::model::{Retention, failure};
use super::persistence::{Persistence, PersistenceGroup};
use super::recovery::{
    CaptureTarget, Recovery, RecoveryResolver, chunk_interval, delete_transcribed,
    ensure_transcript_ready, parse_target, release, remove_delete_on_stop, resolve_chunk_target,
    target_marker,
};
use super::retention::{Activities, Activity, session_directory};
use super::store::{TranscriptStore, statement, string};

type SegmentChanges = BTreeMap<String, Option<Arc<LiveTranscriptSegment>>>;

pub type StartResolver =
    Arc<dyn Fn(SessionId) -> BoxFuture<'static, Result<CaptureConfig>> + Send + Sync>;

pub struct CaptureConfig {
    pub params: SessionParams,
    pub provider: String,
    pub retention: Retention,
    pub memo: String,
    pub recovery: Option<RecoveryResolver>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Loading,
    Listening,
    Finalizing,
    Failed,
}

/// Which capture channels the recorder reports working (`None` until it says either way).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Health {
    pub mic: Option<bool>,
    pub speaker: Option<bool>,
    pub unavailable: bool,
}

pub struct CaptureUpdate {
    pub session: Option<SessionId>,
    pub phase: Phase,
    pub status: Arc<str>,
    pub error: Option<ServiceError>,
    pub amplitude: (u16, u16),
    pub changes: SegmentChanges,
    pub revision: u64,
    pub resync: bool,
    pub health: Health,
    /// A non-channel audio error interrupted capture.
    pub audio_interrupted: bool,
    pub requested_live: Option<bool>,
    pub live_active: Option<bool>,
    pub degraded: Option<DegradedError>,
    pub stalled: bool,
    pub provider: Arc<str>,
}

#[derive(Clone)]
struct RepairContext {
    recovery: Recovery,
    /// Without one, an incomplete transcript can't be repaired, but chunks are still released.
    resolver: Option<RecoveryResolver>,
    directory: PathBuf,
    started: u64,
    /// Older chunks below this belong to finished captures, not to a pending target.
    earliest: u64,
    /// Earlier pending captures in this note; their chunks route to their own transcripts.
    inherited: Vec<CaptureTarget>,
    job: Arc<tokio::sync::Mutex<()>>,
}

struct State {
    session: Option<SessionId>,
    transcript: Option<Arc<str>>,
    phase: Phase,
    status: Arc<str>,
    error: Option<ServiceError>,
    amplitude: (u16, u16),
    changes: SegmentChanges,
    history: VecDeque<(u64, Arc<SegmentChanges>)>,
    dropped_through: u64,
    revision: u64,
    persistence: Option<Persistence>,
    persistence_group: Option<PersistenceGroup>,
    last_activity: Instant,
    last_final: Instant,
    audible: Instant,
    incomplete: bool,
    repair: Option<RepairContext>,
    retention: Retention,
    capture_start: Instant,
    repair_through: u64,
    activity: Option<Activity>,
    batch: bool,
    health: Health,
    audio_interrupted: bool,
    requested_live: Option<bool>,
    live_active: Option<bool>,
    degraded: Option<DegradedError>,
    provider: Arc<str>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            session: None,
            transcript: None,
            phase: Phase::Idle,
            status: "Ready".into(),
            error: None,
            amplitude: (0, 0),
            changes: BTreeMap::new(),
            history: VecDeque::new(),
            dropped_through: 0,
            revision: 0,
            persistence: None,
            persistence_group: None,
            last_activity: Instant::now(),
            last_final: Instant::now(),
            audible: Instant::now(),
            incomplete: false,
            repair: None,
            retention: Retention::Forever,
            capture_start: Instant::now(),
            repair_through: 0,
            activity: None,
            batch: false,
            health: Health::default(),
            audio_interrupted: false,
            requested_live: None,
            live_active: None,
            degraded: None,
            provider: "".into(),
        }
    }
}

struct Adapter {
    storage: Arc<dyn StorageRuntime>,
    state: Arc<Mutex<State>>,
    events: mpsc::Sender<SessionLifecycleEvent>,
    overload: tokio::sync::Notify,
}

impl StorageRuntime for Adapter {
    fn global_base(&self) -> std::result::Result<PathBuf, anlg_storage::Error> {
        self.storage.global_base()
    }
    fn vault_base(&self) -> std::result::Result<PathBuf, anlg_storage::Error> {
        self.storage.vault_base()
    }
}

impl ListenerRuntime for Adapter {
    fn emit_lifecycle(&self, event: SessionLifecycleEvent) {
        if self.events.try_send(event).is_err() {
            self.fail(failure(
                "Capture lifecycle queue failed; recording is stopping for recovery.",
            ));
            self.overload.notify_one();
        }
    }

    fn emit_progress(&self, event: SessionProgressEvent) {
        let (session, status) = match event {
            SessionProgressEvent::AudioInitializing { session_id } => {
                (session_id, "Initializing audio")
            }
            SessionProgressEvent::AudioReady { session_id, .. } => (session_id, "Audio ready"),
            SessionProgressEvent::Connecting { session_id } => (session_id, "Connecting"),
            SessionProgressEvent::Connected { session_id, .. } => (session_id, "Listening…"),
        };
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if state
            .session
            .as_ref()
            .is_some_and(|id| id.0.as_ref() == session)
        {
            state.status = status.into();
            state.revision += 1;
        }
    }

    fn emit_error(&self, event: SessionErrorEvent) {
        let (session, error, audio) = match event {
            SessionErrorEvent::AudioError { error, .. } if error == "audio_deletion_completed" => {
                return;
            }
            SessionErrorEvent::AudioError {
                session_id, error, ..
            } => (session_id, error, true),
            SessionErrorEvent::ConnectionError { session_id, error } => (session_id, error, false),
        };
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut health = state.health;
        if audio && let Some(lost) = update_health(&mut health, &error) {
            if matches_session(&state, &session) {
                state.health = health;
                // A lost channel stays a recovery incident; only its display is a warning.
                if lost {
                    state.incomplete = true;
                    state.repair_through = state.capture_start.elapsed().as_millis() as u64;
                    if let Some(repair) = &state.repair {
                        repair.recovery.incident();
                    }
                }
                state.revision += 1;
            }
            return;
        }
        if state
            .session
            .as_ref()
            .is_none_or(|id| id.0.as_ref() == session)
        {
            state.audio_interrupted |= audio && !error.starts_with("audio_storage_");
            state.error = Some(failure(error));
            state.incomplete = true;
            state.repair_through = state.capture_start.elapsed().as_millis() as u64;
            if let Some(repair) = &state.repair {
                repair.recovery.incident();
            }
            state.revision += 1;
        }
    }

    fn emit_data(&self, event: SessionDataEvent) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        match event {
            SessionDataEvent::AudioAmplitude {
                session_id,
                mic,
                speaker,
            } if matches_session(&state, &session_id) => {
                state.amplitude = (mic, speaker);
                if mic > 100 || speaker > 100 {
                    state.audible = Instant::now();
                }
            }
            SessionDataEvent::TranscriptDelta { session_id, delta }
                if matches_session(&state, &session_id) =>
            {
                state.last_activity = Instant::now();
                if delta
                    .new_words
                    .iter()
                    .any(|word| word.state == anlg_transcript::WordState::Final)
                {
                    state.last_final = Instant::now();
                }
                let persistence = state.persistence.clone();
                drop(state);
                match persistence {
                    Some(persistence) => {
                        if let Err(error) = persistence.try_push(*delta) {
                            self.fail(failure(format!("Transcript admission failed ({error}); recording is stopping. Recovery remains pending.")));
                            self.overload.notify_one();
                        }
                    }
                    None => self.fail(failure("No persistence owner for capture.")),
                }
            }
            SessionDataEvent::TranscriptSegmentDelta { session_id, delta }
                if matches_session(&state, &session_id) =>
            {
                for id in delta.removed_ids {
                    state.changes.insert(id, None);
                }
                for segment in delta.upserts {
                    state
                        .changes
                        .insert(segment.id.clone(), Some(Arc::new(segment)));
                }
                if state.changes.len() > 10_000 {
                    state.error = Some(failure(
                        "Transcript display backlog full; durable data remains in the journal.",
                    ));
                    state.changes.clear();
                }
                state.revision += 1;
            }
            SessionDataEvent::MicDropouts { session_id, .. }
                if matches_session(&state, &session_id) =>
            {
                state.error = Some(failure(
                    "Microphone dropouts detected; transcript may be incomplete.",
                ));
                state.incomplete = true;
                state.repair_through = state.capture_start.elapsed().as_millis() as u64;
                if let Some(repair) = &state.repair {
                    repair.recovery.incident();
                }
                state.revision += 1;
            }
            _ => {}
        }
    }
}

/// The listener reports channel health on the audio error channel; shipping folds these into
/// `captureHealth` (`updateCaptureHealth`) instead of treating them as errors. Returns whether
/// the signal reports a lost channel, or `None` for anything else.
fn update_health(health: &mut Health, error: &str) -> Option<bool> {
    match error {
        "audio_capture_ready" => health.unavailable = false,
        "audio_capture_unavailable" => {
            health.unavailable = true;
            return Some(true);
        }
        "audio_mic_ready" => health.mic = Some(true),
        "audio_speaker_ready" => health.speaker = Some(true),
        _ if error.starts_with("audio_mic_unavailable:") => {
            health.mic = Some(false);
            return Some(true);
        }
        _ if error.starts_with("audio_speaker_unavailable:") => {
            health.speaker = Some(false);
            return Some(true);
        }
        _ => return None,
    }
    Some(false)
}

impl Adapter {
    fn fail(&self, error: ServiceError) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        state.error = Some(error);
        state.incomplete = true;
        state.repair_through = state.capture_start.elapsed().as_millis() as u64;
        if let Some(repair) = &state.repair {
            repair.recovery.incident();
        }
        state.revision += 1;
    }
}

fn matches_session(state: &State, id: &str) -> bool {
    state
        .session
        .as_ref()
        .is_some_and(|session| session.0.as_ref() == id)
}

#[cfg(test)]
mod callback_tests {
    use super::*;

    struct Storage;
    impl StorageRuntime for Storage {
        fn global_base(&self) -> std::result::Result<PathBuf, anlg_storage::Error> {
            Ok(PathBuf::new())
        }
        fn vault_base(&self) -> std::result::Result<PathBuf, anlg_storage::Error> {
            Ok(PathBuf::new())
        }
    }

    #[test]
    fn lifecycle_overload_never_waits_and_marks_recovery() {
        let (events, _receiver) = mpsc::channel(1);
        let state = Arc::new(Mutex::new(State::default()));
        let adapter = Adapter {
            storage: Arc::new(Storage),
            state: state.clone(),
            events,
            overload: tokio::sync::Notify::new(),
        };
        adapter.emit_lifecycle(SessionLifecycleEvent::Finalizing {
            session_id: "fixture".into(),
        });
        adapter.emit_lifecycle(SessionLifecycleEvent::Finalizing {
            session_id: "fixture".into(),
        });
        let state = state.lock().unwrap();
        assert!(state.incomplete);
        assert!(state.error.is_some());
    }

    #[tokio::test]
    async fn a_capture_that_fails_to_finish_frees_its_note() {
        let directory =
            std::env::temp_dir().join(format!("anarlog-capture-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
            database: directory.join("db.sqlite"),
        })
        .unwrap();
        ready.receive().await.unwrap();
        let activities = Activities::default();
        let session = SessionId("fixture".into());
        let state = Arc::new(Mutex::new(State {
            session: Some(session.clone()),
            phase: Phase::Listening,
            activity: Some(activities.acquire(session.clone()).unwrap()),
            ..State::default()
        }));
        // Stop handles this itself when the session ended before it drained the queue.
        let ended = SessionLifecycleEvent::Inactive {
            session_id: "fixture".into(),
            audio_path: None,
            error: Some("audio_stream_failed".into()),
        };
        handle_lifecycle(&runtime, &state, ended).await.unwrap_err();
        assert_eq!(state.lock().unwrap().phase, Phase::Failed);
        activities.acquire(session).unwrap();
        runtime.shutdown().await.unwrap();
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn lost_channels_are_health_warnings_that_still_mark_recovery() {
        let (events, _receiver) = mpsc::channel(1);
        let state = Arc::new(Mutex::new(State {
            session: Some(SessionId("fixture".into())),
            ..State::default()
        }));
        let adapter = Adapter {
            storage: Arc::new(Storage),
            state: state.clone(),
            events,
            overload: tokio::sync::Notify::new(),
        };
        let audio = |error: &str| SessionErrorEvent::AudioError {
            session_id: "fixture".into(),
            error: error.into(),
            device: None,
            is_fatal: false,
        };
        adapter.emit_error(audio("audio_speaker_ready"));
        adapter.emit_error(audio("audio_mic_unavailable: device removed"));
        {
            let state = state.lock().unwrap();
            assert_eq!(state.health.mic, Some(false));
            assert_eq!(state.health.speaker, Some(true));
            assert!(state.incomplete && state.error.is_none() && !state.audio_interrupted);
        }
        adapter.emit_error(audio("audio_stream_failed"));
        let state = state.lock().unwrap();
        assert!(state.audio_interrupted && state.error.is_some());
    }
}

enum Command {
    Start(SessionId, oneshot::Sender<Result<()>>),
    Stop(oneshot::Sender<Result<()>>),
    Devices(oneshot::Sender<Result<Devices>>),
    Microphone(Option<String>, oneshot::Sender<Result<()>>),
    AudioPath(SessionId, oneshot::Sender<Result<PathBuf>>),
}

pub struct Devices {
    pub default: String,
    pub microphones: Vec<String>,
}

#[derive(Clone)]
pub struct CaptureService {
    sender: mpsc::Sender<Command>,
    state: Arc<Mutex<State>>,
    pub activities: Activities,
}

impl CaptureService {
    pub fn spawn(
        runtime: RuntimeHandle,
        audio: Arc<dyn AudioProvider>,
        storage: Arc<dyn StorageRuntime>,
        resolver: StartResolver,
    ) -> Result<Self> {
        let (sender, mut commands) = mpsc::channel(8);
        let state = Arc::new(Mutex::new(State::default()));
        let shared = state.clone();
        let activities = Activities::default();
        let capture_activities = activities.clone();
        std::thread::Builder::new().name("meeting-capture".into()).spawn(move || {
            let executor = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build();
            let (events, mut lifecycle) = mpsc::channel(16);
            let adapter = Arc::new(Adapter { storage, state: shared.clone(), events, overload: tokio::sync::Notify::new() });
            let executor = match executor { Ok(executor) => executor, Err(error) => { adapter.fail(failure(error)); return; } };
            executor.block_on(async move {
                let group = match PersistenceGroup::install(&runtime).await {
                    Ok(group) => group,
                    Err(error) => { adapter.fail(error); return; }
                };
                shared.lock().unwrap_or_else(|poison| poison.into_inner()).persistence_group = Some(group);
                let (root, root_task) = match Actor::spawn(None, RootActor, RootArgs { runtime: adapter.clone(), audio: audio.clone() }).await {
                    Ok(root) => root,
                    Err(error) => { adapter.fail(failure(error)); return; }
                };
                let mut microphone: Option<Option<String>> = None;
                loop {
                    tokio::select! {
                        _ = adapter.overload.notified() => {
                            if let Err(error) = root.call(RootMsg::StopSession, Some(Duration::from_secs(30))).await {
                                adapter.fail(failure(error));
                            }
                        }
                        Some(event) = lifecycle.recv() => {
                            if let Err(error) = handle_lifecycle(&runtime, &shared, event).await {
                                adapter.fail(error);
                            }
                        }
                        command = commands.recv() => match command {
                            Some(Command::Start(session, reply)) => {
                                let resolver = resolver.clone();
                                let selected = microphone.clone();
                                let resolve: StartResolver = Arc::new(move |id| {
                                    let future = resolver(id);
                                    let selected = selected.clone();
                                    Box::pin(async move {
                                        let mut config = future.await?;
                                        if let Some(selected) = selected { config.params.mic_device = selected; }
                                        Ok(config)
                                    })
                                });
                                let result = start(&runtime, &root, &shared, &resolve, adapter.storage.clone(), &capture_activities, session).await;
                                if let Err(error) = &result {
                                    let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
                                    if state.phase == Phase::Loading {
                                        state.phase = Phase::Failed;
                                        state.status = "Recording could not start.".into();
                                    }
                                    state.error = Some(error.clone());
                                    state.revision += 1;
                                }
                                let _ = reply.send(result);
                            }
                            Some(Command::Stop(reply)) => {
                                let mut result = root.call(RootMsg::StopSession, Some(Duration::from_secs(30))).await
                                    .map_err(failure).and_then(|reply| reply.success_or_else(|| failure("Capture stop timed out; finalization is still pending.")));
                                while let Ok(event) = lifecycle.try_recv() {
                                    if let Err(error) = handle_lifecycle(&runtime, &shared, event).await {
                                        adapter.fail(error.clone());
                                        result = Err(error);
                                    }
                                }
                                let _ = reply.send(result);
                            }
                            Some(Command::Devices(reply)) => {
                                let audio = audio.clone();
                                let result = tokio::task::spawn_blocking(move || Devices {
                                    default: audio.default_device_name(), microphones: audio.list_mic_devices(),
                                }).await.map_err(failure);
                                let _ = reply.send(result);
                            }
                            Some(Command::Microphone(selection, reply)) => {
                                let audio = audio.clone();
                                let device = selection.clone();
                                let result = tokio::task::spawn_blocking(move || audio.probe_mic(device).map_err(failure)).await.map_err(failure).and_then(|result| result);
                                if result.is_ok() { microphone = Some(selection); }
                                let _ = reply.send(result);
                            }
                            Some(Command::AudioPath(session, reply)) => {
                                let directory = adapter.storage.vault_base().map_err(failure).and_then(|vault| session_directory(&vault, &session));
                                let result = match directory {
                                    Ok(directory) => tokio::task::spawn_blocking(move || {
                                        ["audio.mp3", "audio.wav", "audio.ogg", "audio.m4a"].into_iter()
                                            .map(|name| directory.join(name)).find(|path| path.is_file())
                                            .ok_or_else(|| failure("No saved audio for this meeting."))
                                    }).await.map_err(failure).and_then(|result| result),
                                    Err(error) => Err(error),
                                };
                                let _ = reply.send(result);
                            }
                            None => {
                                let _ = root.call(RootMsg::StopSession, Some(Duration::from_secs(30))).await;
                                root.stop(None);
                                let _ = root_task.await;
                                break;
                            }
                        }
                    }
                }
            });
        }).map_err(failure)?;
        Ok(Self {
            sender,
            state,
            activities,
        })
    }

    pub fn start(&self, session: SessionId) -> Result<oneshot::Receiver<Result<()>>> {
        let (reply, receive) = oneshot::channel();
        self.sender
            .try_send(Command::Start(session, reply))
            .map_err(|_| ServiceError::Busy)?;
        Ok(receive)
    }

    pub fn stop(&self) -> Result<oneshot::Receiver<Result<()>>> {
        let (reply, receive) = oneshot::channel();
        self.sender
            .try_send(Command::Stop(reply))
            .map_err(|_| ServiceError::Busy)?;
        Ok(receive)
    }

    pub fn devices(&self) -> Result<oneshot::Receiver<Result<Devices>>> {
        let (reply, receive) = oneshot::channel();
        self.sender
            .try_send(Command::Devices(reply))
            .map_err(|_| ServiceError::Busy)?;
        Ok(receive)
    }

    pub fn microphone(&self, device: Option<String>) -> Result<oneshot::Receiver<Result<()>>> {
        let (reply, receive) = oneshot::channel();
        self.sender
            .try_send(Command::Microphone(device, reply))
            .map_err(|_| ServiceError::Busy)?;
        Ok(receive)
    }

    pub fn audio_path(&self, session: SessionId) -> Result<oneshot::Receiver<Result<PathBuf>>> {
        let (reply, receive) = oneshot::channel();
        self.sender
            .try_send(Command::AudioPath(session, reply))
            .map_err(|_| ServiceError::Busy)?;
        Ok(receive)
    }

    pub fn active_probe(&self) -> Arc<dyn Fn() -> bool + Send + Sync> {
        let state = self.state.clone();
        Arc::new(move || {
            let Ok(state) = state.lock() else {
                // Treat a poisoned lock conservatively as active so destructive model actions stay blocked.
                return true;
            };
            matches!(
                state.phase,
                Phase::Loading | Phase::Listening | Phase::Finalizing
            )
        })
    }

    pub fn is_active(&self, session: &SessionId) -> bool {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        state.session.as_ref() == Some(session)
            && matches!(
                state.phase,
                Phase::Loading | Phase::Listening | Phase::Finalizing
            )
    }

    pub fn take_update(&self, after: u64) -> CaptureUpdate {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let persistence_error = state
            .persistence
            .as_ref()
            .and_then(|persistence| persistence.error.borrow().clone());
        let stalled = state.phase == Phase::Listening
            && state.audible.elapsed() < Duration::from_secs(45)
            && (state.last_activity.elapsed() > Duration::from_secs(45)
                || state.last_final.elapsed() > Duration::from_secs(90));
        if !state.changes.is_empty() {
            let changes = Arc::new(std::mem::take(&mut state.changes));
            let revision = state.revision;
            state.history.push_back((revision, changes));
            while state.history.len() > 16 {
                if let Some((revision, _)) = state.history.pop_front() {
                    state.dropped_through = revision;
                }
            }
        }
        let resync = after < state.dropped_through;
        let mut changes = BTreeMap::new();
        for (_, updates) in state
            .history
            .iter()
            .filter(|(revision, _)| *revision > after)
        {
            changes.extend(
                updates
                    .iter()
                    .map(|(id, segment)| (id.clone(), segment.clone())),
            );
        }
        CaptureUpdate {
            session: state.session.clone(),
            phase: state.phase,
            status: state.status.clone(),
            error: persistence_error
                .or_else(|| state.error.clone())
                .or_else(|| {
                    stalled
                        .then(|| failure("Transcription has stalled; audio recovery is required."))
                }),
            amplitude: state.amplitude,
            changes,
            revision: state.revision,
            resync,
            health: state.health,
            audio_interrupted: state.audio_interrupted,
            requested_live: state.requested_live,
            live_active: state.live_active,
            degraded: state.degraded.clone(),
            stalled,
            provider: state.provider.clone(),
        }
    }
}

async fn start(
    runtime: &RuntimeHandle,
    root: &ActorRef<RootMsg>,
    shared: &Arc<Mutex<State>>,
    resolver: &StartResolver,
    storage: Arc<dyn StorageRuntime>,
    activities: &Activities,
    session: SessionId,
) -> Result<()> {
    {
        let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
        if matches!(
            state.phase,
            Phase::Listening | Phase::Loading | Phase::Finalizing
        ) {
            return Err(ServiceError::Busy);
        }
        state.phase = Phase::Loading;
        state.status = "Preparing recording…".into();
        state.session = Some(session.clone());
        state.error = None;
        state.incomplete = false;
        state.health = Health::default();
        state.audio_interrupted = false;
        state.requested_live = None;
        state.live_active = None;
        state.degraded = None;
        state.amplitude = (0, 0);
        state.revision += 1;
    }
    let mut config = resolver(session.clone()).await?;
    if config.params.session_id != session.0.as_ref() {
        return Err(ServiceError::Conflict);
    }
    if config.retention == Retention::Never
        && config.params.transcription_mode == anlg_listener_core::TranscriptionMode::Batch
    {
        return Err(ServiceError::Unsupported("This provider needs saved audio for batch transcription. Choose a live provider for Don’t save.".into()));
    }
    config.params.retain_audio = Some(config.retention != Retention::Never);
    let transcript: Arc<str> = uuid::Uuid::new_v4().to_string().into();
    let activity = activities.acquire(session.clone())?;
    let directory = session_directory(&storage.vault_base().map_err(failure)?, &session)?;
    let audio_directory = directory.clone();
    let audio_offset = tokio::task::spawn_blocking(move || {
        for extension in ["mp3", "wav", "ogg"] {
            let path = audio_directory.join(format!("audio.{extension}"));
            if path.try_exists().map_err(failure)? {
                return super::playback::decode_waveform(&path)
                    .map(|(_, duration)| duration.as_millis() as u64);
            }
        }
        Ok(0)
    })
    .await
    .map_err(failure)??;
    let (started, inherited) =
        prepare(runtime, &session, transcript.clone(), &config, audio_offset).await?;
    // The recorder stamps recovery chunks with the marker's start time so recovery can match a
    // chunk to its attempt even when the wall clock moves between attempts.
    config.params.capture_started_at_ms = Some(started);
    let earliest = inherited
        .iter()
        .map(|target| target.started.saturating_sub(5_000).max(0) as u64)
        .min()
        .unwrap_or(started)
        .min(started);
    let group = shared
        .lock()
        .map_err(failure)?
        .persistence_group
        .clone()
        .ok_or(ServiceError::Closed)?;
    let persistence = group.transcript(runtime, transcript.clone()).await?;
    {
        let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
        state.transcript = Some(transcript.clone());
        state.persistence = Some(persistence);
        state.retention = config.retention;
        state.provider = config.provider.as_str().into();
        state.capture_start = Instant::now();
        state.activity = Some(activity);
        state.repair = Some(RepairContext {
            recovery: Recovery::default(),
            resolver: config.recovery,
            directory,
            started,
            earliest,
            inherited,
            job: Arc::new(tokio::sync::Mutex::new(())),
        });
        state.changes.clear();
        state.history.clear();
        state.last_activity = Instant::now();
        state.last_final = Instant::now();
    }
    let background_state = shared.clone();
    let background_runtime = runtime.clone();
    let capture = transcript.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            {
                let state = background_state
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner());
                if state.transcript.as_ref() != Some(&capture)
                    || matches!(state.phase, Phase::Idle | Phase::Failed)
                {
                    break;
                }
            }
            let result = match repair_once(&background_runtime, &background_state).await {
                Ok(()) => release_chunks(&background_state, false).await,
                error => error,
            };
            if let Err(error) = result
                && !matches!(error, ServiceError::Cancelled)
            {
                let mut state = background_state
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner());
                state.error = Some(error);
                state.revision += 1;
            }
        }
    });
    let response = root
        .call(
            |reply| RootMsg::StartSession(config.params, reply),
            Some(Duration::from_secs(30)),
        )
        .await
        .map_err(failure)?;
    let result = response
        .success_or_else(|| failure("Capture start timed out; its recovery marker is retained."))?;
    result.map_err(|error| {
        failure(format!(
            "Capture failed: {error:?}; recovery marker retained."
        ))
    })
}

pub(super) async fn prepare(
    runtime: &RuntimeHandle,
    session: &SessionId,
    transcript: Arc<str>,
    config: &CaptureConfig,
    audio_offset: u64,
) -> Result<(u64, Vec<CaptureTarget>)> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(failure)?
        .as_millis() as i64;
    let session = session.clone();
    let provider = config.provider.clone();
    let model = config.params.model.clone();
    let memo = config.memo.clone();
    let retained = config.retention != Retention::Never;
    runtime.submit(move |services| async move {
        let rows = services.executor.execute("SELECT owner_user_id, created_at FROM sessions WHERE id = ? AND deleted_at IS NULL AND (locked = 0 OR id IN (SELECT value FROM json_each(?)))".into(), vec![json!(session), super::access::param()]).await.map_err(failure)?;
        let row = rows.first().ok_or(ServiceError::Conflict)?;
        let owner = string(row, "owner_user_id")?;
        let pending = services.executor.execute(
            "SELECT value_json FROM app_settings WHERE id = ?".into(),
            vec![json!(format!("capture_lifecycle_pending:{}", session.0))],
        ).await.map_err(failure)?;
        let mut inherited_entries = Vec::new();
        if let Some(pending) = pending.first() {
            let raw = string(pending, "value_json")?;
            let previous: Value = serde_json::from_str(raw).map_err(|_| {
                failure("An earlier recording's recovery marker is unreadable; recover or discard it before recording again.")
            })?;
            let mut candidates = vec![previous.clone()];
            if let Some(inherited) = previous["inheritedCaptures"].as_array() {
                candidates.extend(inherited.iter().cloned());
            }
            let mut seen = std::collections::HashSet::new();
            for candidate in candidates {
                let mut target = parse_target(&candidate).ok_or_else(|| {
                    failure("An earlier recording's recovery marker is unreadable; recover or discard it before recording again.")
                })?;
                if target.transcript == transcript {
                    continue;
                }
                if !seen.insert(target.transcript.clone()) {
                    continue;
                }
                if target.owner_user_id.is_empty() {
                    target.owner_user_id = owner.to_owned();
                }
                inherited_entries.push((target_marker(&target), target));
            }
            inherited_entries.sort_by_key(|(_, target)| target.started);
        }
        let (inherited_markers, inherited): (Vec<_>, Vec<_>) =
            inherited_entries.into_iter().unzip();
        let mut marker = json!({
            "version": 1, "chunkedAudio": true, "retainAudio": retained, "phase": "capturing",
            "sessionId": session, "transcriptId": transcript, "startedAt": now,
            "createdAt": string(row, "created_at")?, "audioOffsetMs": audio_offset,
            "preserveExistingTranscript": true, "ownerUserId": owner,
            "memo": memo, "provider": provider, "model": model,
        });
        if !inherited_markers.is_empty() {
            marker["inheritedCaptures"] = json!(inherited_markers);
        }
        services.executor.execute_transaction(vec![
            statement("INSERT INTO transcripts (id, session_id, owner_user_id, workspace_id, source, provider, model, started_at_ms, memo) SELECT ?, id, owner_user_id, workspace_id, 'live_capture', ?, ?, ?, ? FROM sessions WHERE id = ? AND deleted_at IS NULL AND (locked = 0 OR id IN (SELECT value FROM json_each(?)))",
                vec![json!(transcript), json!(provider), json!(model), json!(now), json!(memo), json!(session), super::access::param()], Some(1)),
            statement("INSERT INTO app_settings (id, value_json, updated_at) VALUES (?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
                vec![json!(format!("capture_lifecycle_pending:{}", session.0)), json!(marker.to_string())], Some(1)),
        ]).await.map_err(failure)?;
        Ok((now as u64, inherited))
    })?.receive().await
}

async fn handle_lifecycle(
    runtime: &RuntimeHandle,
    shared: &Arc<Mutex<State>>,
    event: SessionLifecycleEvent,
) -> Result<()> {
    match event {
        SessionLifecycleEvent::Active {
            session_id,
            error,
            requested_transcription_mode,
            current_transcription_mode,
        } => {
            let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
            if matches_session(&state, &session_id) {
                state.phase = Phase::Listening;
                state.status = "Listening…".into();
                state.requested_live =
                    Some(requested_transcription_mode == TranscriptionMode::Live);
                state.live_active = Some(current_transcription_mode == TranscriptionMode::Live);
                state.degraded = error.clone();
                state.batch = current_transcription_mode == TranscriptionMode::Batch;
                state.incomplete |= state.batch;
                if let Some(error) = error {
                    state.error = Some(failure(format!("{error:?}")));
                    state.incomplete = true;
                }
                state.revision += 1;
            }
        }
        SessionLifecycleEvent::Finalizing { session_id } => {
            let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
            if matches_session(&state, &session_id) {
                state.phase = Phase::Finalizing;
                state.status = "Finalizing transcript…".into();
                state.revision += 1;
            }
        }
        SessionLifecycleEvent::Inactive {
            session_id, error, ..
        } => {
            let result = finish(runtime, shared, session_id, error).await;
            // A capture that fails to finish must not stay busy, wherever its event was handled.
            if result.is_err() {
                let repair = {
                    let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
                    state.phase = Phase::Failed;
                    state.activity.take();
                    state.repair.clone()
                };
                if let Some(repair) = repair {
                    repair.recovery.stop().await;
                }
            }
            return result;
        }
    }
    Ok(())
}

async fn finish(
    runtime: &RuntimeHandle,
    shared: &Arc<Mutex<State>>,
    session_id: String,
    error: Option<String>,
) -> Result<()> {
    let (persistence, transcript, retained) = {
        let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
        if !matches_session(&state, &session_id) {
            if let Some(error) = error {
                state.error = Some(failure(error));
                state.revision += 1;
            }
            return Ok(());
        }
        state.phase = Phase::Finalizing;
        (
            state.persistence.clone(),
            state.transcript.clone(),
            state.retention != Retention::Never,
        )
    };
    if let Some(persistence) = persistence {
        persistence.retire().await?;
    }
    if let Some(error) = error {
        return Err(failure(error));
    }
    repair_once(runtime, shared).await?;
    let (incomplete, recovery) = {
        let state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
        (state.incomplete, state.repair.clone())
    };
    let unresolved = match (recovery.as_ref(), transcript.as_ref()) {
        (Some(repair), Some(transcript)) => inherited_chunks_remain(repair, transcript).await?,
        _ => false,
    };
    let inherited: Vec<Arc<str>> = recovery
        .as_ref()
        .map(|repair| {
            repair
                .inherited
                .iter()
                .map(|target| target.transcript.clone())
                .collect()
        })
        .unwrap_or_default();
    // An earlier retained attempt owns the shared archive; a "Don't save" retry must not delete
    // it. Only the retry's own recovery chunks go (released above).
    let inherited_retained = recovery
        .as_ref()
        .is_some_and(|repair| repair.inherited.iter().any(|target| target.retained));
    if let Some(repair) = &recovery {
        repair.recovery.stop().await;
    }
    if incomplete {
        return Err(failure(
            "Recording stopped with an incomplete transcript; its audio is kept for recovery.",
        ));
    }
    release_chunks(shared, true).await?;
    // "Don't save" counts as saved only once its audio is gone. Earlier pending audio keeps the
    // shared archive until startup recovery finishes it.
    if !retained
        && !unresolved
        && !inherited_retained
        && let Some(repair) = recovery.as_ref()
        && !delete_transcribed(repair.directory.clone()).await?
    {
        return Err(failure(
            "Don’t save audio is still waiting for transcription; it is kept for recovery.",
        ));
    }
    // A "Don't save" retry over a retained archive must not leave the recorder's deletion marker
    // behind, or startup cleanup would delete the earlier recording on the next launch.
    if inherited_retained
        && let Some(repair) = recovery.as_ref()
        && remove_delete_on_stop(repair.directory.clone()).await?
    {
        // The marker is gone; the retained archive survives startup cleanup.
    }
    if let Some(transcript) = transcript {
        runtime.submit(move |services| async move {
            let delete_target = transcript.clone();
            let mut statements = vec![statement(
                "UPDATE transcripts SET ended_at_ms = CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER) WHERE id = ?",
                vec![json!(transcript)],
                Some(1),
            )];
            if !unresolved {
                statements.push(statement(
                    "DELETE FROM app_settings WHERE id = ? AND json_valid(value_json) AND json_extract(value_json, '$.transcriptId') = ?",
                    vec![json!(format!("capture_lifecycle_pending:{session_id}")), json!(delete_target)],
                    Some(1),
                ));
                for inherited in inherited {
                    statements.push(statement(
                        "UPDATE transcripts SET ended_at_ms = COALESCE(ended_at_ms, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)) WHERE id = ? AND session_id = ? AND deleted_at IS NULL",
                        vec![json!(inherited), json!(&session_id)],
                        None,
                    ));
                }
            }
            services.executor.execute_transaction(statements).await.map_err(failure)?;
            Ok(())
        })?.receive().await?;
    }
    let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
    state.phase = Phase::Idle;
    state.activity = None;
    state.status = if unresolved {
        "Recording saved; earlier audio is still waiting for recovery".into()
    } else {
        "Recording saved".into()
    };
    state.persistence = None;
    state.revision += 1;
    Ok(())
}

async fn inherited_chunks_remain(repair: &RepairContext, transcript: &Arc<str>) -> Result<bool> {
    if repair.inherited.is_empty() {
        return Ok(false);
    }
    let primary = CaptureTarget {
        transcript: transcript.clone(),
        started: repair.started as i64,
        provider: String::new(),
        model: String::new(),
        memo: String::new(),
        created_at: None,
        owner_user_id: String::new(),
        retained: true,
    };
    let chunks = Recovery::chunks(repair.directory.clone()).await?;
    Ok(chunks.iter().any(|chunk| {
        chunk.capture_started_at >= repair.earliest
            && resolve_chunk_target(chunk, &primary, &repair.inherited)
                .is_some_and(|target| target.transcript.as_ref() != transcript.as_ref())
    }))
}

async fn repair_once(runtime: &RuntimeHandle, shared: &Arc<Mutex<State>>) -> Result<()> {
    let (repair, resolver, session, transcript, persistence, through) = {
        let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
        let stalled = state.phase == Phase::Listening
            && state.audible.elapsed() < Duration::from_secs(45)
            && (state.last_activity.elapsed() > Duration::from_secs(45)
                || state.last_final.elapsed() > Duration::from_secs(90));
        if stalled && !state.incomplete {
            state.incomplete = true;
            state.repair_through = state.capture_start.elapsed().as_millis() as u64;
            if let Some(repair) = &state.repair {
                repair.recovery.incident();
            }
        }
        let inherited = state
            .repair
            .as_ref()
            .is_some_and(|repair| !repair.inherited.is_empty());
        if !state.incomplete && !state.batch && !inherited {
            return Ok(());
        }
        let (Some(repair), Some(session), Some(transcript), Some(persistence)) = (
            state.repair.clone(),
            state.session.clone(),
            state.transcript.clone(),
            state.persistence.clone(),
        ) else {
            return Err(failure(
                "Transcript is incomplete and this provider has no batch recovery adapter.",
            ));
        };
        let Some(resolver) = repair.resolver.clone() else {
            return Err(if state.incomplete || state.batch {
                failure("Transcript is incomplete and this provider has no batch recovery adapter.")
            } else {
                failure(
                    "An earlier interrupted recording needs batch recovery, but this provider has no batch adapter.",
                )
            });
        };
        (
            repair,
            resolver,
            session,
            transcript,
            persistence,
            state.repair_through,
        )
    };
    let _job = repair.job.lock().await;
    let (incomplete, batch) = {
        let state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
        (state.incomplete, state.batch)
    };
    if !incomplete && !batch && repair.inherited.is_empty() {
        return Ok(());
    }
    let generation = repair.recovery.generation();
    let store = TranscriptStore(runtime.clone());
    let primary = CaptureTarget {
        transcript: transcript.clone(),
        started: repair.started as i64,
        provider: String::new(),
        model: String::new(),
        memo: String::new(),
        created_at: None,
        owner_user_id: String::new(),
        retained: true,
    };
    let mut recovered_through = 0;
    let mut failed = HashSet::new();
    let mut cursor: Option<(u64, u64)> = None;
    loop {
        // Advance a cursor past every examined chunk so a full page of failed inherited chunks
        // cannot block the current capture's later chunks. Failed files stay unacknowledged.
        let chunks = Recovery::chunks_after(repair.directory.clone(), cursor).await?;
        if chunks.is_empty() {
            break;
        }
        let mut progressed = false;
        for chunk in chunks {
            cursor = Some((chunk.capture_started_at, chunk.start_ms));
            if failed.contains(&chunk.id) {
                continue;
            }
            let Some(target) = resolve_chunk_target(&chunk, &primary, &repair.inherited) else {
                failed.insert(chunk.id);
                continue;
            };
            let current = target.transcript == transcript;
            if current && !incomplete && !batch {
                continue;
            }
            if current {
                persistence.flush().await?;
            } else if ensure_transcript_ready(runtime, &store, &session, target)
                .await
                .is_err()
            {
                failed.insert(chunk.id);
                continue;
            }
            let chunk_id = chunk.id.clone();
            let gap = chunk_interval(&chunk, target.started);
            let end = gap.end.max(0) as u64;
            match repair
                .recovery
                .repair(
                    &store,
                    session.clone(),
                    target.transcript.clone(),
                    chunk,
                    vec![gap],
                    &resolver,
                )
                .await
            {
                Ok(()) => {
                    if current {
                        recovered_through = recovered_through.max(end);
                    }
                    progressed = true;
                }
                Err(error) if current => return Err(error),
                Err(_) => {
                    failed.insert(chunk_id);
                }
            }
        }
        if !progressed && failed.len() >= 128 {
            // Every chunk on this page failed; the cursor already advanced past them, so the next
            // iteration reads the following page instead of looping on the same one.
            continue;
        }
        if !progressed {
            break;
        }
    }
    let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
    if recovered_through > through && repair.recovery.generation() == generation {
        state.incomplete = false;
        state.error = None;
        state.revision += 1;
    }
    Ok(())
}

/// Shipping's `createCaptureAudioRecovery`: a complete capture releases each recovery chunk once
/// its speech is persisted, and the rest after its final flush (`settled`). Incomplete captures
/// keep theirs for `repair_once`.
async fn release_chunks(shared: &Arc<Mutex<State>>, settled: bool) -> Result<()> {
    let (repair, persistence, transcript) = {
        let state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
        if state.incomplete || state.batch {
            return Ok(());
        }
        let (Some(repair), Some(persistence), Some(transcript)) = (
            state.repair.clone(),
            state.persistence.clone(),
            state.transcript.clone(),
        ) else {
            return Ok(());
        };
        (repair, persistence, transcript)
    };
    let _job = repair.job.lock().await;
    let primary = CaptureTarget {
        transcript,
        started: repair.started as i64,
        provider: String::new(),
        model: String::new(),
        memo: String::new(),
        created_at: None,
        owner_user_id: String::new(),
        retained: true,
    };
    release_attempt_chunks(
        &repair.directory,
        &primary,
        &repair.inherited,
        persistence.persisted_through(),
        settled,
    )
    .await
}

pub(super) async fn release_attempt_chunks(
    directory: &std::path::Path,
    primary: &CaptureTarget,
    inherited: &[CaptureTarget],
    through: i64,
    settled: bool,
) -> Result<()> {
    let earliest = inherited
        .iter()
        .map(|target| target.started.saturating_sub(5_000).max(0) as u64)
        .min()
        .unwrap_or(primary.started.max(0) as u64)
        .min(primary.started.max(0) as u64);
    let mut cursor: Option<(u64, u64)> = None;
    loop {
        // Page with a cursor so a full page of inherited-owned chunks cannot hide the current
        // capture's chunks on later pages. Examined chunks are never re-read.
        let chunks = Recovery::chunks_after(directory.to_path_buf(), cursor).await?;
        if chunks.is_empty() {
            return Ok(());
        }
        let page_len = chunks.len();
        let mut released = false;
        for chunk in chunks {
            cursor = Some((chunk.capture_started_at, chunk.start_ms));
            // Classify by the owning target, not by start time alone: a backward clock
            // adjustment can make an inherited chunk's start land after the current one.
            match resolve_chunk_target(&chunk, primary, inherited) {
                Some(target) if target.transcript == primary.transcript => {
                    if !settled && chunk_interval(&chunk, primary.started).end > through {
                        continue;
                    }
                    release(directory, chunk.id).await?;
                    released = true;
                }
                Some(_) => {
                    // An inherited target still owns this chunk; keep it for its own transcript.
                }
                None if chunk.capture_started_at < earliest => {
                    // Older than every known pending capture: a leftover from a finished one.
                    release(directory, chunk.id).await?;
                    released = true;
                }
                None => {}
            }
        }
        if !released && page_len < 128 {
            // A short page with nothing releasable means the rest are inherited-owned too.
            return Ok(());
        }
    }
}
