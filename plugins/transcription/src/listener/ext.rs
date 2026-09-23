use ractor::{ActorRef, call_t, registry};

use crate::{CaptureConfigUpdate, CaptureParams, CaptureSnapshot, CaptureState, SessionStateCache};
use anlg_transcription_core::listener::{
    StartSessionError,
    actors::{RootActor, RootMsg, SessionParams, SourceActor, SourceMsg},
};

fn capture_snapshot_from_result<E: std::fmt::Debug>(
    result: Result<anlg_transcription_core::listener::Snapshot, E>,
) -> crate::Result<CaptureSnapshot> {
    result.map(CaptureSnapshot::from).map_err(|error| {
        tracing::warn!(?error, "capture_snapshot_unavailable");
        crate::Error::CaptureSnapshotUnavailable
    })
}

async fn active_source() -> Option<ActorRef<SourceMsg>> {
    let root: ActorRef<RootMsg> = registry::where_is(RootActor::name())?.into();
    let snapshot = call_t!(root, RootMsg::GetSnapshot, 100).ok()?;
    let session_id = snapshot.active_session_id?;
    registry::where_is(SourceActor::name(&session_id)).map(Into::into)
}

fn hydrate_session_state(
    snapshot: &mut CaptureSnapshot,
    session_id: String,
    cached: Option<(
        bool,
        bool,
        Vec<anlg_transcription_core::listener::LiveTranscriptSegment>,
    )>,
) {
    snapshot.live_segments_session_id = Some(session_id);
    if let Some((requested, active, segments)) = cached {
        snapshot.requested_live_transcription = Some(requested);
        snapshot.live_transcription_active = Some(active);
        snapshot.live_segments = Some(segments);
    } else {
        snapshot.live_segments = Some(Vec::new());
    }
}

pub struct Listener<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    #[allow(unused)]
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Listener<'a, R, M> {
    #[tracing::instrument(skip_all)]
    pub async fn list_microphone_devices(&self) -> Result<Vec<String>, crate::Error> {
        let audio = self
            .manager
            .state::<std::sync::Arc<dyn anlg_audio::AudioProvider>>();
        Ok(audio.list_mic_devices())
    }

    #[tracing::instrument(skip_all)]
    pub async fn get_current_microphone_device(&self) -> Result<Option<String>, crate::Error> {
        if let Some(actor) = active_source().await {
            match call_t!(actor, SourceMsg::GetMicDevice, 500) {
                Ok(device_name) => Ok(device_name),
                Err(_) => Ok(None),
            }
        } else {
            Err(crate::Error::ActorNotFound("active source".to_string()))
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn get_capture_state(&self) -> CaptureState {
        if let Some(cell) = registry::where_is(RootActor::name()) {
            let actor: ActorRef<RootMsg> = cell.into();
            match call_t!(actor, RootMsg::GetState, 100) {
                Ok(fsm_state) => CaptureState::from(fsm_state),
                Err(_) => CaptureState::Inactive,
            }
        } else {
            CaptureState::Inactive
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn get_capture_snapshot(&self) -> Result<CaptureSnapshot, crate::Error> {
        let cell = registry::where_is(RootActor::name())
            .ok_or_else(|| crate::Error::ActorNotFound(RootActor::name().to_string()))?;
        let actor: ActorRef<RootMsg> = cell.into();
        let mut snapshot = capture_snapshot_from_result(call_t!(actor, RootMsg::GetSnapshot, 100))?;

        let session_id = snapshot
            .active_session_id
            .as_ref()
            .or_else(|| snapshot.finalizing_session_ids.first())
            .cloned();
        if let Some(session_id) = session_id {
            let cached = self
                .manager
                .try_state::<SessionStateCache>()
                .and_then(|cache| {
                    let cache = cache.lock().ok()?;
                    let state = cache.get(&session_id)?;
                    Some((
                        state.requested_live_transcription,
                        state.live_transcription_active,
                        state.live_segments.clone(),
                    ))
                });
            hydrate_session_state(&mut snapshot, session_id, cached);
        }

        Ok(snapshot)
    }

    #[tracing::instrument(skip_all)]
    pub async fn get_mic_muted(&self) -> bool {
        if let Some(actor) = active_source().await {
            call_t!(actor, SourceMsg::GetMicMute, 100).unwrap_or_default()
        } else {
            false
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn set_mic_muted(&self, muted: bool) {
        if let Some(actor) = active_source().await {
            let _ = actor.cast(SourceMsg::SetMicMute(muted));
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn start_capture(&self, params: CaptureParams) -> Result<(), crate::Error> {
        let params: SessionParams = params.into();
        if let Some(cell) = registry::where_is(RootActor::name()) {
            let actor: ActorRef<RootMsg> = cell.into();
            match ractor::call!(actor, RootMsg::StartSession, params) {
                Ok(Ok(())) => Ok(()),
                Ok(Err(StartSessionError::SessionAlreadyRunning)) => {
                    Err(crate::Error::SessionAlreadyRunning)
                }
                Ok(Err(_)) => Err(crate::Error::StartSessionFailed),
                Err(_) => Err(crate::Error::StartSessionFailed),
            }
        } else {
            Err(crate::Error::ActorNotFound(RootActor::name().to_string()))
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn stop_capture(&self) {
        if let Some(cell) = registry::where_is(RootActor::name()) {
            let actor: ActorRef<RootMsg> = cell.into();
            let _ = ractor::call!(actor, RootMsg::StopSession);
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn delete_capture_audio(&self, session_id: String) -> Result<(), String> {
        let cell = registry::where_is(RootActor::name()).ok_or_else(|| {
            crate::Error::ActorNotFound(RootActor::name().to_string()).to_string()
        })?;
        let actor: ActorRef<RootMsg> = cell.into();
        match ractor::call!(actor, RootMsg::DeleteSessionAudio, session_id) {
            Ok(result) => result,
            Err(error) => Err(error.to_string()),
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn update_capture_config(&self, update: CaptureConfigUpdate) {
        if let Some(cell) = registry::where_is(RootActor::name()) {
            let actor: ActorRef<RootMsg> = cell.into();
            let update = update.into();
            let _ = ractor::call!(actor, RootMsg::UpdateSessionConfig, update);
        }
    }
}

pub trait ListenerPluginExt<R: tauri::Runtime> {
    fn listener(&self) -> Listener<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> ListenerPluginExt<R> for T {
    fn listener(&self) -> Listener<'_, R, Self>
    where
        Self: Sized,
    {
        Listener {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SnapshotProbe;

    #[ractor::async_trait]
    impl ractor::Actor for SnapshotProbe {
        type Msg = RootMsg;
        type State = Option<String>;
        type Arguments = Self::State;

        async fn pre_start(
            &self,
            _myself: ActorRef<RootMsg>,
            args: Self::Arguments,
        ) -> Result<Self::State, ractor::ActorProcessingErr> {
            Ok(args)
        }

        async fn handle(
            &self,
            _myself: ActorRef<RootMsg>,
            message: RootMsg,
            state: &mut Self::State,
        ) -> Result<(), ractor::ActorProcessingErr> {
            match message {
                RootMsg::GetSnapshot(reply) => {
                    let _ = reply.send(anlg_transcription_core::listener::Snapshot {
                        state: if state.is_some() {
                            anlg_transcription_core::listener::State::Active
                        } else {
                            anlg_transcription_core::listener::State::Finalizing
                        },
                        active_session_id: state.clone(),
                        finalizing_session_ids: vec!["previous".to_string()],
                    });
                }
                RootMsg::StopSession(reply) => {
                    *state = None;
                    let _ = reply.send(());
                }
                _ => {}
            }
            Ok(())
        }
    }

    struct MicProbe;

    #[ractor::async_trait]
    impl ractor::Actor for MicProbe {
        type Msg = SourceMsg;
        type State = bool;
        type Arguments = ();

        async fn pre_start(
            &self,
            _myself: ActorRef<SourceMsg>,
            _args: (),
        ) -> Result<bool, ractor::ActorProcessingErr> {
            Ok(false)
        }

        async fn handle(
            &self,
            _myself: ActorRef<SourceMsg>,
            message: SourceMsg,
            muted: &mut bool,
        ) -> Result<(), ractor::ActorProcessingErr> {
            match message {
                SourceMsg::SetMicMute(value) => *muted = value,
                SourceMsg::GetMicMute(reply) => {
                    let _ = reply.send(*muted);
                }
                _ => {}
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn microphone_controls_target_active_session_and_ignore_finalizing_sessions() {
        use ractor::Actor;

        let (previous, previous_task) =
            Actor::spawn(Some(SourceActor::name("previous")), MicProbe, ())
                .await
                .unwrap();
        let next_id = uuid::Uuid::new_v4().to_string();
        let (next, next_task) = Actor::spawn(Some(SourceActor::name(&next_id)), MicProbe, ())
            .await
            .unwrap();
        let (root, root_task) = Actor::spawn(Some(RootActor::name()), SnapshotProbe, Some(next_id))
            .await
            .unwrap();

        let active = active_source().await.unwrap();
        assert_eq!(active.get_id(), next.get_id());
        active.cast(SourceMsg::SetMicMute(true)).unwrap();
        assert!(call_t!(next, SourceMsg::GetMicMute, 1000).unwrap());
        assert!(!call_t!(previous, SourceMsg::GetMicMute, 1000).unwrap());
        call_t!(root, RootMsg::StopSession, 1000).unwrap();
        assert!(active_source().await.is_none());

        root.stop(None);
        previous.stop(None);
        next.stop(None);
        root_task.await.unwrap();
        previous_task.await.unwrap();
        next_task.await.unwrap();
    }

    #[test]
    fn capture_snapshot_failure_is_not_reported_as_inactive() {
        let error = capture_snapshot_from_result::<&str>(Err("timed out"))
            .expect_err("snapshot failure must remain retryable");

        assert!(matches!(error, crate::Error::CaptureSnapshotUnavailable));
    }

    #[test]
    fn hydrated_segments_are_labeled_with_their_session() {
        let mut snapshot = CaptureSnapshot {
            state: CaptureState::Finalizing,
            active_session_id: None,
            finalizing_session_ids: vec!["session-a".to_string()],
            requested_live_transcription: None,
            live_transcription_active: None,
            live_segments_session_id: None,
            live_segments: None,
        };

        hydrate_session_state(&mut snapshot, "session-a".to_string(), None);

        assert_eq!(
            snapshot.live_segments_session_id.as_deref(),
            Some("session-a")
        );
        assert_eq!(snapshot.live_segments, Some(Vec::new()));
    }
}
