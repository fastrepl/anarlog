use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anlg_meeting_capture::{
    BotState, CaptureProviderKind, CaptureWorkerCheckpoint, MeetingPlatform, MeetingReference,
    MeetingSdkBridgeCommand, MeetingSdkBridgeEvent, MeetingSdkBridgeNormalizer,
    MeetingSdkBridgeStart,
};
use tauri::Manager;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_sidecar2::Sidecar2PluginExt;
use tauri_specta::Event;

use crate::{
    Error, Result, SIDECAR_NAME, ZoomClientEvent, ZoomClientState, ZoomMeetingStatus,
    ZoomParticipant,
};

const STOP_GRACE: std::time::Duration = std::time::Duration::from_secs(5);

pub fn is_enabled() -> bool {
    cfg!(debug_assertions) || option_env!("ANARLOG_ZOOM_CLIENT").is_some()
}

/// Zoom Meeting SDK credentials never live in source; the sidecar mints its own JWT
/// from these when present, otherwise it runs in stub mode.
pub const SDK_KEY_ENV: &str = "ANARLOG_ZOOM_SDK_KEY";
pub const SDK_SECRET_ENV: &str = "ANARLOG_ZOOM_SDK_SECRET";

fn sdk_credential_env() -> HashMap<String, String> {
    [SDK_KEY_ENV, SDK_SECRET_ENV]
        .into_iter()
        .filter_map(|name| {
            std::env::var(name)
                .ok()
                .map(|value| (name.to_string(), value))
        })
        .collect()
}

#[derive(Default)]
pub struct ManagedState {
    active: Mutex<Option<ActiveSession>>,
}

struct ActiveSession {
    session_id: String,
    job_id: String,
    child: Option<CommandChild>,
    status: Arc<Mutex<ZoomMeetingStatus>>,
}

pub struct ZoomClientPlugin<'a, R: tauri::Runtime, M: Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

pub trait ZoomClientPluginExt<R: tauri::Runtime>: Manager<R> + Sized {
    fn zoom_client(&self) -> ZoomClientPlugin<'_, R, Self> {
        ZoomClientPlugin {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}

impl<R: tauri::Runtime, M: Manager<R>> ZoomClientPluginExt<R> for M {}

impl<R: tauri::Runtime, M: Manager<R>> ZoomClientPlugin<'_, R, M> {
    pub fn status(&self) -> ZoomMeetingStatus {
        let state = self.manager.state::<ManagedState>();
        let active = state.active.lock().unwrap();
        match active.as_ref() {
            Some(session) => session.status.lock().unwrap().clone(),
            None => idle_status(),
        }
    }

    pub fn join(&self, meeting_url: String, display_name: String) -> Result<String> {
        if !is_enabled() {
            return Err(Error::Disabled);
        }
        if display_name.trim().is_empty() || display_name.chars().count() > 80 {
            return Err(Error::InvalidDisplayName);
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let checkpoint = CaptureWorkerCheckpoint {
            job_id: format!("zoom-{session_id}"),
            bot_id: format!("zoom-self-{session_id}"),
            provider: CaptureProviderKind::ZoomMeetingSdk,
            meeting: MeetingReference {
                platform: MeetingPlatform::Zoom,
                url: meeting_url,
                external_id: None,
                calendar_event_id: None,
            },
            state: BotState::Queued,
            next_sequence: 0,
        };
        let start = MeetingSdkBridgeStart::new(checkpoint.clone(), display_name.trim())?;
        let normalizer = MeetingSdkBridgeNormalizer::new(&checkpoint)?;

        let state = self.manager.state::<ManagedState>();
        let mut active = state.active.lock().unwrap();
        if active
            .as_ref()
            .is_some_and(|session| session.status.lock().unwrap().state != ZoomClientState::Ended)
        {
            return Err(Error::AlreadyActive);
        }

        let (rx, mut child) = self
            .manager
            .app_handle()
            .sidecar2()
            .sidecar(SIDECAR_NAME)?
            .envs(sdk_credential_env())
            .spawn()?;
        let mut line = serde_json::to_vec(&MeetingSdkBridgeCommand::Start(start))?;
        line.push(b'\n');
        if let Err(error) = child.write(&line) {
            let _ = child.kill();
            return Err(error.into());
        }

        let status = Arc::new(Mutex::new(ZoomMeetingStatus {
            session_id: Some(session_id.clone()),
            state: ZoomClientState::Launching,
            participants: vec![],
        }));
        *active = Some(ActiveSession {
            session_id: session_id.clone(),
            job_id: checkpoint.job_id.clone(),
            child: Some(child),
            status: status.clone(),
        });
        drop(active);

        let app = self.manager.app_handle().clone();
        tauri::async_runtime::spawn(pump_events(app, session_id.clone(), normalizer, status, rx));
        Ok(session_id)
    }

    pub fn leave(&self) -> Result<()> {
        let state = self.manager.state::<ManagedState>();
        let (job_id, mut child) = {
            let mut active = state.active.lock().unwrap();
            let session = active.as_mut().ok_or(Error::NotActive)?;
            let child = session.child.take().ok_or(Error::NotActive)?;
            session.status.lock().unwrap().state = ZoomClientState::Stopping;
            (session.job_id.clone(), child)
        };

        let mut line = serde_json::to_vec(&MeetingSdkBridgeCommand::Stop { job_id })?;
        line.push(b'\n');
        if child.write(&line).is_err() {
            child.kill()?;
            return Ok(());
        }

        let app = self.manager.app_handle().clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(STOP_GRACE).await;
            let state = app.state::<ManagedState>();
            let still_running = state
                .active
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|session| {
                    session.status.lock().unwrap().state != ZoomClientState::Ended
                });
            if still_running {
                tracing::warn!("zoom_sidecar_stop_timeout");
                let _ = child.kill();
            }
        });
        Ok(())
    }
}

fn idle_status() -> ZoomMeetingStatus {
    ZoomMeetingStatus {
        session_id: None,
        state: ZoomClientState::Idle,
        participants: vec![],
    }
}

async fn pump_events<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
    mut normalizer: MeetingSdkBridgeNormalizer,
    status: Arc<Mutex<ZoomMeetingStatus>>,
    mut rx: tauri::async_runtime::Receiver<CommandEvent>,
) {
    let emit = |event: ZoomClientEvent| {
        if let Err(error) = event.emit(&app) {
            tracing::error!(?error, "zoom_client_event_emit_failed");
        }
    };

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => {
                let line = String::from_utf8_lossy(&bytes);
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let parsed = serde_json::from_str::<MeetingSdkBridgeEvent>(line)
                    .map_err(Error::from)
                    .and_then(|event| {
                        normalizer
                            .accept(event, chrono::Utc::now())
                            .map_err(Error::from)
                    });
                match parsed {
                    Ok(capture) => {
                        apply_to_status(&status, &capture.payload);
                        if let Some(event) =
                            ZoomClientEvent::from_capture(&session_id, capture.payload)
                        {
                            emit(event);
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%error, "zoom_bridge_event_rejected");
                        emit(ZoomClientEvent::Error {
                            session_id: session_id.clone(),
                            message: error.to_string(),
                        });
                    }
                }
            }
            CommandEvent::Stderr(bytes) => {
                tracing::debug!(line = %String::from_utf8_lossy(&bytes).trim(), "zoom_sidecar");
            }
            CommandEvent::Error(message) => {
                tracing::error!(%message, "zoom_sidecar_error");
            }
            CommandEvent::Terminated(payload) => {
                tracing::info!(code = ?payload.code, "zoom_sidecar_exited");
                break;
            }
            _ => {}
        }
    }

    let already_ended = {
        let mut guard = status.lock().unwrap();
        let ended = guard.state == ZoomClientState::Ended;
        guard.state = ZoomClientState::Ended;
        ended
    };
    if !already_ended {
        emit(ZoomClientEvent::StateChanged {
            session_id: session_id.clone(),
            state: ZoomClientState::Ended,
            reason: Some("worker_exited".into()),
        });
    }

    let state = app.state::<ManagedState>();
    let mut active = state.active.lock().unwrap();
    if active
        .as_ref()
        .is_some_and(|session| session.session_id == session_id)
    {
        *active = None;
    }
}

fn apply_to_status(
    status: &Mutex<ZoomMeetingStatus>,
    payload: &anlg_meeting_capture::CaptureEventPayload,
) {
    use anlg_meeting_capture::CaptureEventPayload;

    let mut guard = status.lock().unwrap();
    match payload {
        CaptureEventPayload::Lifecycle(transition) => {
            guard.state = transition.to.into();
        }
        CaptureEventPayload::ParticipantUpserted(participant) => {
            let participant = ZoomParticipant::from(participant.clone());
            match guard
                .participants
                .iter_mut()
                .find(|existing| existing.id == participant.id)
            {
                Some(existing) => *existing = participant,
                None => guard.participants.push(participant),
            }
        }
        CaptureEventPayload::ParticipantLeft { participant_id } => {
            guard
                .participants
                .retain(|participant| &participant.id != participant_id);
        }
        CaptureEventPayload::Transcript(_)
        | CaptureEventPayload::SpeakerUpserted(_)
        | CaptureEventPayload::ActiveSpeakers(_)
        | CaptureEventPayload::RecordingChunkReady(_) => {}
    }
}
