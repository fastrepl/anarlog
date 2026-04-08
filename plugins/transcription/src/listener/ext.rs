use ractor::{ActorRef, call_t, registry};

use crate::{CaptureParams, CaptureState};
use hypr_transcription_core::listener::{
    StartSessionError,
    actors::{RootActor, RootMsg, SessionParams, SourceActor, SourceMsg},
};

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
            .state::<std::sync::Arc<dyn hypr_audio::AudioProvider>>();
        Ok(audio.list_mic_devices())
    }

    #[tracing::instrument(skip_all)]
    pub async fn get_current_microphone_device(&self) -> Result<Option<String>, crate::Error> {
        if let Some(cell) = registry::where_is(SourceActor::name()) {
            let actor: ActorRef<SourceMsg> = cell.into();
            match call_t!(actor, SourceMsg::GetMicDevice, 500) {
                Ok(device_name) => Ok(device_name),
                Err(_) => Ok(None),
            }
        } else {
            Err(crate::Error::ActorNotFound(SourceActor::name()))
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
    pub async fn get_mic_muted(&self) -> bool {
        if let Some(cell) = registry::where_is(SourceActor::name()) {
            let actor: ActorRef<SourceMsg> = cell.into();
            call_t!(actor, SourceMsg::GetMicMute, 100).unwrap_or_default()
        } else {
            false
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn set_mic_muted(&self, muted: bool) {
        if let Some(cell) = registry::where_is(SourceActor::name()) {
            let actor: ActorRef<SourceMsg> = cell.into();
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
