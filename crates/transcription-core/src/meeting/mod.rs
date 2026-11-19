//! The one owner of a meeting recording's context: which capture is current, for which account,
//! what call it confirmed, who attached to it, and what was observed during it. Everything that
//! acts asynchronously for a recording asks this runtime before acting.

mod router;
mod staging;

pub use router::*;
pub use staging::*;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{SystemTime, UNIX_EPOCH};

use anlg_audio::meeting::{
    CallPlatform, CaptureAuthority, CaptureEndReason, CaptureIdentity, CaptureInstant,
    CaptureProvenance, CaptureScope, ConfirmedCall, LifecycleOwner, MeetingCaptureEvent,
    MeetingLifecycleEvent, MeetingLifecycleObserver, MicProcessing, ObservationSource,
    SpeakerObservation,
};
use anlg_transcript::MeetingSpeakerEvidence;

/// Bounds one capture's evidence: a long call with constant turn-taking sends a few per second.
const MAX_OBSERVATIONS: usize = 50_000;
/// Ended captures whose evidence final reconciliation may still read.
const ENDED_KEPT: usize = 4;
/// A wall time further than this from now is the reader's clock, not ours.
const MAX_WALL_SKEW_MS: i64 = 5 * 60 * 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LinkToken(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetachOutcome {
    KeepRecording,
    StopRecording,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MeetingError {
    #[error("another recording is already active")]
    CaptureActive,
    #[error("this recording is no longer current")]
    Stale,
    #[error("this recording belongs to another account")]
    AccountMismatch,
    #[error("no call was confirmed for this recording")]
    NoConfirmedCall,
    #[error("this is not the recording's confirmed call")]
    CallMismatch,
    #[error("this browser link is no longer current")]
    StaleLink,
    #[error("this extension start is no longer current")]
    StaleExtensionStart,
    #[error("the recording is paused")]
    Paused,
    #[error("{0}")]
    Io(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveCapture {
    pub identity: CaptureIdentity,
    pub owner: LifecycleOwner,
    pub scope: Option<CaptureScope>,
    pub provenance: Option<CaptureProvenance>,
    pub paused: bool,
    pub linked: bool,
}

struct Active {
    capture: ActiveCapture,
    origin_wall_ms: Option<i64>,
    /// Epoch ms of this capture's transcript start, which evidence times are relative to.
    transcript_started_at_ms: Option<i64>,
    evidence: Option<MeetingSpeakerEvidence>,
    connected: bool,
    link: Option<LinkToken>,
    extension_actions: bool,
    /// The extension start whose reservation this capture took.
    extension_request: Option<String>,
    observations: Vec<SpeakerObservation>,
}

/// An extension start the window was told to record: only the record path's begin carrying this
/// request id, for this note, is the extension's, and it gets this capture id.
struct Reservation {
    request_id: String,
    session_id: String,
    capture_id: String,
}

struct Ended {
    identity: CaptureIdentity,
    observations: Vec<SpeakerObservation>,
    evidence: Option<MeetingSpeakerEvidence>,
}

#[derive(Default)]
struct State {
    /// The signed-in account; `None` while signed out.
    account_id: Option<String>,
    active: Option<Active>,
    ended: VecDeque<Ended>,
    extension_reservation: Option<Reservation>,
}

impl State {
    /// An account change ends the active capture, so "still active" also means "same account".
    fn current(&self, capture: &CaptureIdentity) -> Result<&Active, MeetingError> {
        self.active
            .as_ref()
            .filter(|active| active.capture.identity == *capture)
            .ok_or(MeetingError::Stale)
    }

    fn current_mut(&mut self, capture: &CaptureIdentity) -> Result<&mut Active, MeetingError> {
        self.current(capture)?;
        Ok(self.active.as_mut().expect("checked above"))
    }

    fn start(
        &mut self,
        identity: CaptureIdentity,
        owner: LifecycleOwner,
        extension_request: Option<String>,
    ) -> CaptureIdentity {
        self.active = Some(Active {
            capture: ActiveCapture {
                identity: identity.clone(),
                owner,
                scope: None,
                provenance: None,
                paused: false,
                linked: false,
            },
            origin_wall_ms: None,
            transcript_started_at_ms: None,
            evidence: None,
            connected: false,
            link: None,
            extension_actions: false,
            extension_request,
            observations: Vec::new(),
        });
        identity
    }

    /// Moves the active capture to the ended list with its evidence closed. Idempotent; an older
    /// capture finishing never ends a newer one.
    fn end(&mut self, capture: &CaptureIdentity) -> bool {
        let Some(mut active) = self
            .active
            .take_if(|active| active.capture.identity == *capture)
        else {
            return false;
        };
        if let (Some(evidence), Some(origin)) = (&mut active.evidence, active.origin_wall_ms) {
            evidence.close(
                CaptureInstant::from_millis((now_ms() - origin).max(0) as f64)
                    .unwrap_or(CaptureInstant(0)),
            );
        }
        self.ended.push_back(Ended {
            identity: active.capture.identity,
            observations: active.observations,
            evidence: active.evidence,
        });
        while self.ended.len() > ENDED_KEPT {
            self.ended.pop_front();
        }
        true
    }
}

/// Lifecycle events in the order the state changed. They are queued under the state lock and
/// delivered outside it by one thread at a time, so no observer hears an attach or an end before
/// the start it follows, and an observer may call back into the runtime.
#[derive(Default)]
struct Outbox {
    queue: VecDeque<MeetingLifecycleEvent>,
    delivering: bool,
}

pub struct MeetingRuntime {
    state: Mutex<State>,
    outbox: Mutex<Outbox>,
    observers: Mutex<Vec<Arc<dyn MeetingLifecycleObserver>>>,
    staging_root: PathBuf,
    this: Weak<MeetingRuntime>,
}

impl MeetingRuntime {
    pub fn new(staging_root: PathBuf) -> Arc<Self> {
        Arc::new_cyclic(|this| Self {
            state: Mutex::default(),
            outbox: Mutex::default(),
            observers: Mutex::default(),
            staging_root,
            this: this.clone(),
        })
    }

    pub fn staging_root(&self) -> &std::path::Path {
        &self.staging_root
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    pub fn register_observer(&self, observer: Arc<dyn MeetingLifecycleObserver>) {
        self.observers
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .push(observer);
    }

    fn outbox(&self) -> MutexGuard<'_, Outbox> {
        self.outbox
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// Queues an event. Call with the state lock held, right where the state changed.
    fn queue(&self, event: MeetingLifecycleEvent) {
        self.outbox().queue.push_back(event);
    }

    /// Delivers queued events in order. Call after releasing the state lock.
    fn deliver(&self) {
        let Some(this) = self.this.upgrade() else {
            return;
        };
        let authority: Arc<dyn CaptureAuthority> = this;
        loop {
            let event = {
                let mut outbox = self.outbox();
                if outbox.delivering {
                    return;
                }
                let Some(event) = outbox.queue.pop_front() else {
                    return;
                };
                outbox.delivering = true;
                event
            };
            let observers = self
                .observers
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .clone();
            for observer in observers {
                observer.on_lifecycle(&event, authority.clone());
            }
            self.outbox().delivering = false;
        }
    }

    pub fn account(&self) -> Option<String> {
        self.state().account_id.clone()
    }

    /// The signed-in account changed (`None`: signed out). Any change drops a pending extension
    /// reservation, so a start asked for under one account never records under another. Signing
    /// in from signed out keeps the active capture, so a local-library recording continues.
    /// Signing out or switching accounts ends it before returning, with `AccountChanged`.
    /// Returns that capture so the caller stops its recorder.
    pub fn set_account(&self, account_id: Option<String>) -> Option<CaptureIdentity> {
        let ended = {
            let mut state = self.state();
            let previous = std::mem::replace(&mut state.account_id, account_id);
            if previous == state.account_id {
                return None;
            }
            state.extension_reservation = None;
            // Signing in from signed out keeps a local-library recording.
            previous.as_ref()?;
            let capture = state.active.as_ref()?.capture.identity.clone();
            state.end(&capture);
            self.queue(MeetingLifecycleEvent::Ended {
                capture: capture.clone(),
                reason: CaptureEndReason::AccountChanged,
            });
            capture
        };
        self.deliver();
        Some(ended)
    }

    /// Reserves the capture of extension start `request_id`: only the record path's begin that
    /// carries this request id for exactly `session_id` takes it, as the extension's, with the
    /// returned capture id. One reservation at a time, and none while recording.
    pub fn reserve_extension_start(
        &self,
        request_id: &str,
        session_id: &str,
    ) -> Result<String, MeetingError> {
        let mut state = self.state();
        if state.active.is_some() || state.extension_reservation.is_some() {
            return Err(MeetingError::CaptureActive);
        }
        let capture_id = uuid::Uuid::new_v4().to_string();
        state.extension_reservation = Some(Reservation {
            request_id: request_id.to_string(),
            session_id: session_id.to_string(),
            capture_id: capture_id.clone(),
        });
        Ok(capture_id)
    }

    /// Withdraws the reservation of `request_id`, so its begin is refused from now on. If its
    /// begin came first and that capture is still active, returns it so the caller stops it
    /// through the record path.
    pub fn cancel_extension_start(&self, request_id: &str) -> Option<CaptureIdentity> {
        let mut state = self.state();
        if state
            .extension_reservation
            .take_if(|reservation| reservation.request_id == request_id)
            .is_some()
        {
            return None;
        }
        state
            .active
            .as_ref()
            .filter(|active| active.extension_request.as_deref() == Some(request_id))
            .map(|active| active.capture.identity.clone())
    }

    /// The record path's begin. A start the extension asked for carries its request id: it is the
    /// extension's capture only while that request's reservation for this note is current, and is
    /// refused otherwise, never begun as anything else. Without one, it is the app's capture
    /// whatever is reserved.
    pub fn begin_recording(
        &self,
        account_id: &str,
        session_id: &str,
        extension_start: Option<&str>,
    ) -> Result<CaptureIdentity, MeetingError> {
        let Some(request_id) = extension_start else {
            return self.begin(account_id, session_id, LifecycleOwner::App);
        };
        let mut state = self.state();
        let reserved = state
            .extension_reservation
            .as_ref()
            .is_some_and(|reservation| {
                reservation.request_id == request_id && reservation.session_id == session_id
            });
        if !reserved {
            return Err(MeetingError::StaleExtensionStart);
        }
        if state.active.is_some() {
            return Err(MeetingError::CaptureActive);
        }
        let reservation = state.extension_reservation.take().expect("checked above");
        let identity = CaptureIdentity {
            account_id: account_id.to_string(),
            session_id: session_id.to_string(),
            capture_id: reservation.capture_id,
        };
        Ok(state.start(
            identity,
            LifecycleOwner::Extension,
            Some(reservation.request_id),
        ))
    }

    /// A capture with exactly this owner. Never takes an extension reservation.
    pub fn begin(
        &self,
        account_id: &str,
        session_id: &str,
        owner: LifecycleOwner,
    ) -> Result<CaptureIdentity, MeetingError> {
        let mut state = self.state();
        if state.active.is_some() {
            return Err(MeetingError::CaptureActive);
        }
        let identity = CaptureIdentity {
            account_id: account_id.to_string(),
            session_id: session_id.to_string(),
            capture_id: uuid::Uuid::new_v4().to_string(),
        };
        Ok(state.start(identity, owner, None))
    }

    /// Records what the engine actually opened. The first confirmation starts the capture clock
    /// and announces the recording, once per capture, with the engine that opened; a later one
    /// must match exactly, never broaden it.
    pub fn confirm(
        &self,
        capture: &CaptureIdentity,
        scope: CaptureScope,
        provenance: CaptureProvenance,
    ) -> Result<(), MeetingError> {
        {
            let mut state = self.state();
            let active = state.current_mut(capture)?;
            match &active.capture.scope {
                Some(existing) if *existing != scope => return Err(MeetingError::CallMismatch),
                Some(_) => return Ok(()),
                None => {}
            }
            active.capture.scope = Some(scope.clone());
            active.capture.provenance = Some(provenance);
            let origin = now_ms();
            active.origin_wall_ms = Some(origin);
            active.evidence = Some(MeetingSpeakerEvidence::new(
                capture.capture_id.clone(),
                active.transcript_started_at_ms.unwrap_or(origin),
            ));
            let owner = active.capture.owner;
            self.queue(MeetingLifecycleEvent::Started {
                capture: capture.clone(),
                scope,
                owner,
                engine: provenance.engine,
            });
        }
        self.deliver();
        Ok(())
    }

    pub fn set_paused(&self, capture: &CaptureIdentity, paused: bool) -> Result<(), MeetingError> {
        {
            let mut state = self.state();
            let active = state.current_mut(capture)?;
            if active.capture.paused == paused {
                return Ok(());
            }
            active.capture.paused = paused;
            self.queue(if paused {
                MeetingLifecycleEvent::Paused {
                    capture: capture.clone(),
                }
            } else {
                MeetingLifecycleEvent::Resumed {
                    capture: capture.clone(),
                }
            });
        }
        self.deliver();
        Ok(())
    }

    /// The OS switched the microphone between voice processing and raw input. The capture's
    /// provenance follows, so nothing presents processed audio as raw or cancels its echo again.
    pub fn set_mic_processing(
        &self,
        capture: &CaptureIdentity,
        mic: MicProcessing,
    ) -> Result<(), MeetingError> {
        let mut state = self.state();
        let active = state.current_mut(capture)?;
        if let Some(provenance) = &mut active.capture.provenance {
            provenance.mic = mic;
        }
        Ok(())
    }

    /// Ends the capture. Idempotent; an older capture finishing never ends a newer one.
    pub fn finish(&self, capture: &CaptureIdentity, reason: CaptureEndReason) {
        {
            let mut state = self.state();
            if !state.end(capture) {
                return;
            }
            self.queue(MeetingLifecycleEvent::Ended {
                capture: capture.clone(),
                reason,
            });
        }
        self.deliver();
    }

    pub fn active(&self) -> Option<ActiveCapture> {
        self.state()
            .active
            .as_ref()
            .map(|active| active.capture.clone())
    }

    pub fn active_for_session(&self, session_id: &str) -> Option<CaptureIdentity> {
        self.active()
            .map(|active| active.identity)
            .filter(|identity| identity.session_id == session_id)
    }

    pub fn observe(&self, observation: SpeakerObservation) -> Result<(), MeetingError> {
        let mut state = self.state();
        let active = state.current_mut(&observation.capture)?;
        if active.capture.paused {
            return Err(MeetingError::Paused);
        }
        match observation.source {
            ObservationSource::Accessibility => {
                let call = active
                    .capture
                    .scope
                    .as_ref()
                    .and_then(CaptureScope::call)
                    .ok_or(MeetingError::NoConfirmedCall)?;
                if call.platform != observation.platform {
                    return Err(MeetingError::CallMismatch);
                }
            }
            ObservationSource::BrowserExtension => {
                if active.link.is_none() || observation.platform != CallPlatform::GoogleMeet {
                    return Err(MeetingError::CallMismatch);
                }
            }
        }
        if let Some(evidence) = &mut active.evidence {
            evidence.observe(&observation);
        }
        if active.observations.len() < MAX_OBSERVATIONS {
            active.observations.push(observation);
        }
        Ok(())
    }

    /// Where this capture's transcript starts, so evidence lands on its word offsets. Set before
    /// the engine opens; later calls change nothing.
    pub fn set_transcript_start(
        &self,
        capture: &CaptureIdentity,
        started_at_ms: i64,
    ) -> Result<(), MeetingError> {
        let mut state = self.state();
        let active = state.current_mut(capture)?;
        active.transcript_started_at_ms.get_or_insert(started_at_ms);
        Ok(())
    }

    /// A helper fact for this capture's evidence. Observations still go through [`Self::observe`].
    pub fn record_helper_event(
        &self,
        capture: &CaptureIdentity,
        event: &MeetingCaptureEvent,
    ) -> Result<(), MeetingError> {
        let mut state = self.state();
        let active = state.current_mut(capture)?;
        if let Some(evidence) = &mut active.evidence {
            evidence.apply(event);
        }
        Ok(())
    }

    /// The live transcription stream connected. Every connection after the first restarts the
    /// provider's voice indices.
    pub fn transcription_connected(&self, capture: &CaptureIdentity) -> Result<(), MeetingError> {
        let mut state = self.state();
        let active = state.current_mut(capture)?;
        let Some(origin) = active.origin_wall_ms else {
            return Ok(());
        };
        if std::mem::replace(&mut active.connected, true)
            && let Some(evidence) = &mut active.evidence
        {
            evidence.provider_restarted(
                CaptureInstant::from_millis((now_ms() - origin).max(0) as f64)
                    .unwrap_or(CaptureInstant(0)),
            );
        }
        Ok(())
    }

    /// A snapshot of this capture's evidence to persist: open while recording, closed after.
    pub fn evidence(&self, capture: &CaptureIdentity) -> Option<MeetingSpeakerEvidence> {
        let state = self.state();
        if let Some(active) = state
            .active
            .as_ref()
            .filter(|active| active.capture.identity == *capture)
        {
            return active.evidence.clone();
        }
        state
            .ended
            .iter()
            .find(|ended| ended.identity == *capture)
            .and_then(|ended| ended.evidence.clone())
    }

    /// Evidence for exactly this capture, also shortly after it ended for final reconciliation.
    pub fn observations(&self, capture: &CaptureIdentity) -> Vec<SpeakerObservation> {
        let state = self.state();
        if let Some(active) = state
            .active
            .as_ref()
            .filter(|active| active.capture.identity == *capture)
        {
            return active.observations.clone();
        }
        state
            .ended
            .iter()
            .find(|ended| ended.identity == *capture)
            .map(|ended| ended.observations.clone())
            .unwrap_or_default()
    }

    pub fn instant_from_wall_ms(
        &self,
        capture: &CaptureIdentity,
        wall_ms: i64,
    ) -> Option<CaptureInstant> {
        let origin = self.state().current(capture).ok()?.origin_wall_ms?;
        let now = now_ms();
        let wall_ms = if (now - wall_ms).abs() > MAX_WALL_SKEW_MS {
            now
        } else {
            wall_ms
        };
        (wall_ms >= origin).then(|| CaptureInstant::from_millis((wall_ms - origin) as f64))?
    }

    /// Links a browser tab to the capture whose confirmed call is exactly `call_url`. A new link
    /// replaces the previous one, whose token then resolves as stale.
    pub fn link(
        &self,
        capture: &CaptureIdentity,
        call_url: &str,
    ) -> Result<LinkToken, MeetingError> {
        let token = {
            let mut state = self.state();
            let active = state.current_mut(capture)?;
            let call = active
                .capture
                .scope
                .as_ref()
                .and_then(CaptureScope::call)
                .ok_or(MeetingError::NoConfirmedCall)?;
            let confirmed = call.call_url.as_deref().ok_or(MeetingError::CallMismatch)?;
            if call.platform != CallPlatform::GoogleMeet || !same_meet_call(confirmed, call_url) {
                return Err(MeetingError::CallMismatch);
            }
            let token = LinkToken(uuid::Uuid::new_v4().simple().to_string());
            let first = active.link.replace(token.clone()).is_none();
            active.capture.linked = true;
            active.extension_actions = true;
            if first {
                self.queue(MeetingLifecycleEvent::ExtensionAttached {
                    capture: capture.clone(),
                });
            }
            token
        };
        self.deliver();
        Ok(token)
    }

    pub fn resolve_link(&self, token: &LinkToken) -> Result<CaptureIdentity, MeetingError> {
        let state = self.state();
        let active = state
            .active
            .as_ref()
            .filter(|active| active.link.as_ref() == Some(token))
            .ok_or(MeetingError::StaleLink)?;
        let identity = active.capture.identity.clone();
        state
            .current(&identity)
            .map(|_| identity.clone())
            .map_err(|_| MeetingError::StaleLink)
    }

    /// Decides only; the caller stops through the one recorder path on `StopRecording`. The
    /// extension keeps the call's actions after detaching so native code never repeats them.
    pub fn detach(&self, token: &LinkToken) -> Result<DetachOutcome, MeetingError> {
        let owner = {
            let mut state = self.state();
            let active = state
                .active
                .as_mut()
                .filter(|active| active.link.as_ref() == Some(token))
                .ok_or(MeetingError::StaleLink)?;
            active.link = None;
            active.capture.linked = false;
            let capture = active.capture.identity.clone();
            let owner = active.capture.owner;
            self.queue(MeetingLifecycleEvent::ExtensionDetached { capture });
            owner
        };
        self.deliver();
        Ok(match owner {
            LifecycleOwner::App => DetachOutcome::KeepRecording,
            LifecycleOwner::Extension => DetachOutcome::StopRecording,
        })
    }

    pub fn staging_dir(&self, capture: &CaptureIdentity) -> Result<PathBuf, MeetingError> {
        self.state().current(capture)?;
        staging::create_staging_dir(&self.staging_root, capture)
            .map_err(|error| MeetingError::Io(error.to_string()))
    }
}

impl CaptureAuthority for MeetingRuntime {
    fn is_current(&self, capture: &CaptureIdentity) -> bool {
        self.state().current(capture).is_ok()
    }

    fn confirmed_call(&self, capture: &CaptureIdentity) -> Option<ConfirmedCall> {
        self.state()
            .current(capture)
            .ok()?
            .capture
            .scope
            .as_ref()?
            .call()
            .cloned()
    }

    fn extension_handles_actions(&self, capture: &CaptureIdentity) -> bool {
        self.state()
            .current(capture)
            .is_ok_and(|active| active.extension_actions)
    }
}

// Exact canonical comparison until the browser lane's canonicalizer is registered.
fn same_meet_call(confirmed: &str, requested: &str) -> bool {
    confirmed == requested
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
