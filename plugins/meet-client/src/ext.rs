use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::Instant;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_specta::Event;

use crate::{
    Error, MeetActiveSpeakers, MeetClientEvent, MeetClientState, MeetMeetingStatus,
    MeetObservation, MeetParticipant, MeetSelectors, Result, WINDOW_LABEL,
};

const OBSERVER_JS: &str = include_str!("observer.js");

pub fn is_enabled() -> bool {
    cfg!(debug_assertions) || option_env!("ANARLOG_MEET_CLIENT").is_some()
}

pub fn is_meet_url(raw: &str) -> bool {
    url::Url::parse(raw).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str() == Some("meet.google.com")
            && url
                .path()
                .trim_start_matches('/')
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
    })
}

#[derive(Default)]
pub struct ManagedState {
    selectors: Mutex<Option<MeetSelectors>>,
    active: Mutex<Option<ActiveSession>>,
}

struct ActiveSession {
    session_id: String,
    status: MeetMeetingStatus,
    capture_started: Option<Instant>,
    speaking: BTreeSet<String>,
}

pub struct MeetClientPlugin<'a, R: tauri::Runtime, M: Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

pub trait MeetClientPluginExt<R: tauri::Runtime>: Manager<R> + Sized {
    fn meet_client(&self) -> MeetClientPlugin<'_, R, Self> {
        MeetClientPlugin {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}

impl<R: tauri::Runtime, M: Manager<R>> MeetClientPluginExt<R> for M {}

impl<R: tauri::Runtime, M: Manager<R>> MeetClientPlugin<'_, R, M> {
    pub fn status(&self) -> MeetMeetingStatus {
        let state = self.manager.state::<ManagedState>();
        let active = state.active.lock().unwrap();
        match active.as_ref() {
            Some(session) => session.status.clone(),
            None => idle_status(),
        }
    }

    pub fn selectors(&self) -> MeetSelectors {
        let state = self.manager.state::<ManagedState>();
        state.selectors.lock().unwrap().clone().unwrap_or_default()
    }

    pub fn set_selectors(&self, selectors: MeetSelectors) -> Result<()> {
        let state = self.manager.state::<ManagedState>();
        *state.selectors.lock().unwrap() = Some(selectors.clone());
        if let Some(window) = self.manager.get_webview_window(WINDOW_LABEL) {
            let config = serde_json::to_string(&selectors)?;
            window.eval(format!(
                "window.__anarlogMeet && window.__anarlogMeet.configure({config});"
            ))?;
        }
        Ok(())
    }

    pub fn join(&self, meeting_url: String) -> Result<String> {
        if !is_enabled() {
            return Err(Error::Disabled);
        }
        if !is_meet_url(&meeting_url) {
            return Err(Error::InvalidMeetingUrl);
        }
        let url = url::Url::parse(&meeting_url).map_err(|_| Error::InvalidMeetingUrl)?;

        let state = self.manager.state::<ManagedState>();
        let mut active = state.active.lock().unwrap();
        if active
            .as_ref()
            .is_some_and(|session| session.status.state != MeetClientState::Ended)
        {
            return Err(Error::AlreadyActive);
        }
        if let Some(stale) = self.manager.get_webview_window(WINDOW_LABEL) {
            let _ = stale.close();
        }

        let session_id = uuid::Uuid::new_v4().to_string();
        let config = serde_json::to_string(&self.selectors())?;
        let app = self.manager.app_handle().clone();
        WebviewWindowBuilder::new(&app, WINDOW_LABEL, WebviewUrl::External(url))
            .title("Google Meet")
            .inner_size(1280.0, 800.0)
            .initialization_script(format!("window.__ANARLOG_MEET_CONFIG__ = {config};"))
            .initialization_script(OBSERVER_JS)
            .build()?;

        *active = Some(ActiveSession {
            session_id: session_id.clone(),
            status: MeetMeetingStatus {
                session_id: Some(session_id.clone()),
                state: MeetClientState::Launching,
                participants: vec![],
            },
            capture_started: None,
            speaking: BTreeSet::new(),
        });
        drop(active);

        let on_close_app = app.clone();
        let on_close_session = session_id.clone();
        if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
            window.on_window_event(move |event| {
                if matches!(event, tauri::WindowEvent::Destroyed) {
                    end_session(&on_close_app, &on_close_session, "window_closed");
                }
            });
        }

        emit(
            &app,
            MeetClientEvent::StateChanged {
                session_id: session_id.clone(),
                state: MeetClientState::Launching,
                reason: None,
            },
        );
        Ok(session_id)
    }

    #[cfg(test)]
    pub(crate) fn begin_for_test(&self, session_id: &str) {
        let state = self.manager.state::<ManagedState>();
        *state.active.lock().unwrap() = Some(ActiveSession {
            session_id: session_id.to_string(),
            status: MeetMeetingStatus {
                session_id: Some(session_id.to_string()),
                state: MeetClientState::Launching,
                participants: vec![],
            },
            capture_started: None,
            speaking: BTreeSet::new(),
        });
    }

    pub fn leave(&self) -> Result<()> {
        let state = self.manager.state::<ManagedState>();
        let session_id = {
            let mut active = state.active.lock().unwrap();
            let session = active.as_mut().ok_or(Error::NotActive)?;
            if session.status.state == MeetClientState::Ended {
                return Err(Error::NotActive);
            }
            session.status.state = MeetClientState::Stopping;
            session.session_id.clone()
        };
        let app = self.manager.app_handle().clone();
        emit(
            &app,
            MeetClientEvent::StateChanged {
                session_id: session_id.clone(),
                state: MeetClientState::Stopping,
                reason: None,
            },
        );
        match app.get_webview_window(WINDOW_LABEL) {
            Some(window) => window.close()?,
            None => end_session(&app, &session_id, "stopped_by_request"),
        }
        Ok(())
    }

    /// Applies a DOM snapshot from the Meet webview to the active session.
    pub fn observe(&self, observation: MeetObservation) -> Result<()> {
        let app = self.manager.app_handle().clone();
        let state = self.manager.state::<ManagedState>();
        let mut active = state.active.lock().unwrap();
        let Some(session) = active.as_mut() else {
            return Ok(());
        };
        if matches!(
            session.status.state,
            MeetClientState::Stopping | MeetClientState::Ended
        ) {
            return Ok(());
        }
        let session_id = session.session_id.clone();

        if observation.ended && session.capture_started.is_some() {
            drop(active);
            end_session(&app, &session_id, "meeting_ended");
            return Ok(());
        }

        if !observation.in_meeting {
            if session.status.state == MeetClientState::Launching {
                session.status.state = MeetClientState::WaitingForAdmission;
                emit(
                    &app,
                    MeetClientEvent::StateChanged {
                        session_id,
                        state: MeetClientState::WaitingForAdmission,
                        reason: None,
                    },
                );
            }
            return Ok(());
        }

        let started = *session.capture_started.get_or_insert_with(|| {
            session.status.state = MeetClientState::Capturing;
            emit(
                &app,
                MeetClientEvent::StateChanged {
                    session_id: session_id.clone(),
                    state: MeetClientState::Capturing,
                    reason: None,
                },
            );
            Instant::now()
        });

        let mut seen = BTreeSet::new();
        let mut speaking = BTreeSet::new();
        for observed in observation.participants {
            seen.insert(observed.id.clone());
            if observed.speaking {
                speaking.insert(observed.id.clone());
            }
            let participant = MeetParticipant {
                id: observed.id,
                display_name: observed.display_name,
                email: None,
            };
            let changed = match session
                .status
                .participants
                .iter_mut()
                .find(|existing| existing.id == participant.id)
            {
                Some(existing) if *existing == participant => false,
                Some(existing) => {
                    *existing = participant.clone();
                    true
                }
                None => {
                    session.status.participants.push(participant.clone());
                    true
                }
            };
            if changed {
                emit(
                    &app,
                    MeetClientEvent::ParticipantUpserted {
                        session_id: session_id.clone(),
                        participant,
                    },
                );
            }
        }

        let left: Vec<String> = session
            .status
            .participants
            .iter()
            .filter(|participant| !seen.contains(&participant.id))
            .map(|participant| participant.id.clone())
            .collect();
        session
            .status
            .participants
            .retain(|participant| seen.contains(&participant.id));
        for participant_id in left {
            emit(
                &app,
                MeetClientEvent::ParticipantLeft {
                    session_id: session_id.clone(),
                    participant_id,
                },
            );
        }

        if speaking != session.speaking {
            session.speaking = speaking.clone();
            emit(
                &app,
                MeetClientEvent::ActiveSpeakers {
                    session_id,
                    speakers: MeetActiveSpeakers {
                        at_ms: started.elapsed().as_millis() as u64,
                        participant_ids: speaking.into_iter().collect(),
                    },
                },
            );
        }
        Ok(())
    }
}

fn idle_status() -> MeetMeetingStatus {
    MeetMeetingStatus {
        session_id: None,
        state: MeetClientState::Idle,
        participants: vec![],
    }
}

fn emit<R: tauri::Runtime>(app: &tauri::AppHandle<R>, event: MeetClientEvent) {
    if let Err(error) = event.emit(app) {
        tracing::error!(?error, "meet_client_event_emit_failed");
    }
}

fn end_session<R: tauri::Runtime>(app: &tauri::AppHandle<R>, session_id: &str, reason: &str) {
    let state = app.state::<ManagedState>();
    let ended_now = {
        let mut active = state.active.lock().unwrap();
        match active.as_mut() {
            Some(session)
                if session.session_id == session_id
                    && session.status.state != MeetClientState::Ended =>
            {
                session.status.state = MeetClientState::Ended;
                true
            }
            _ => false,
        }
    };
    if ended_now {
        emit(
            app,
            MeetClientEvent::StateChanged {
                session_id: session_id.to_string(),
                state: MeetClientState::Ended,
                reason: Some(reason.to_string()),
            },
        );
        if let Some(window) = app.get_webview_window(WINDOW_LABEL) {
            let _ = window.close();
        }
    }
}
