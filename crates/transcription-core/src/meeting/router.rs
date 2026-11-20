//! Chooses the audio engine once per capture and keeps it. The native engine records every
//! two-track capture of the Mac's current default input when its helper is installed; the existing
//! engine keeps a named microphone, partial captures and platforms or builds without the helper.
//! A native failure is an error, never a quiet legacy fallback, and a re-open asks for exactly the
//! scope the capture already had.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use anlg_audio::meeting::{
    CaptureEndReason, CaptureEngine, CaptureIdentity, CaptureInstant, CaptureProvenance,
    CaptureScope, EngineAvailability, MeetingCaptureControl, MeetingCaptureEngine,
    MeetingCaptureEvent, MeetingCaptureRequest, MeetingEventStream, ScopeRequest,
};
use anlg_audio::{AudioProvider, CaptureConfig, CaptureStream, Error};
use futures_util::future::BoxFuture;
use futures_util::{FutureExt, StreamExt};

use super::{MeetingRuntime, staging};

/// Ended captures whose staging waits for adoption; older ones are left to the startup sweep.
const ROUTES_KEPT: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MeetingCaptureCapability {
    pub availability: Option<EngineAvailability>,
    pub engine: CaptureEngine,
    pub reason: Option<String>,
}

struct Route {
    capture: CaptureIdentity,
    engine: CaptureEngine,
    scope: Option<CaptureScope>,
    controls: Vec<Arc<dyn MeetingCaptureControl>>,
    order: u64,
}

type HelperEndHandler = Arc<dyn Fn(&CaptureIdentity, CaptureEndReason) + Send + Sync>;

pub struct MeetingAudioRouter {
    inner: Arc<dyn AudioProvider>,
    runtime: Arc<MeetingRuntime>,
    routes: Mutex<(u64, HashMap<String, Route>)>,
    helper_end: Mutex<Option<HelperEndHandler>>,
    this: Weak<MeetingAudioRouter>,
}

impl MeetingAudioRouter {
    pub fn new(inner: Arc<dyn AudioProvider>, runtime: Arc<MeetingRuntime>) -> Arc<Self> {
        Arc::new_cyclic(|this| Self {
            inner,
            runtime,
            routes: Mutex::default(),
            helper_end: Mutex::default(),
            this: this.clone(),
        })
    }

    /// Called when the native helper itself ended a capture (the call's app quit, the maximum
    /// length was reached). The capture has already finished with that reason; the handler stops
    /// the recorder through the record path, since a reopen would only record nothing or exceed
    /// the limit.
    pub fn on_helper_end(
        &self,
        handler: impl Fn(&CaptureIdentity, CaptureEndReason) + Send + Sync + 'static,
    ) {
        *self
            .helper_end
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(Arc::new(handler));
    }

    /// [`MeetingRuntime::set_account`], then stops the ended capture's native helper. The caller
    /// stops the recorder.
    pub fn set_account(&self, account_id: Option<String>) -> Option<CaptureIdentity> {
        let ended = self.runtime.set_account(account_id)?;
        self.stop_helpers(&ended);
        Some(ended)
    }

    /// True only when this capture is current on the native engine and has an open helper.
    pub fn can_pause(&self, capture: &CaptureIdentity) -> bool {
        self.current_control(capture).is_some()
    }

    /// Pauses the capture's helper and resolves on its acknowledgement, with the runtime already
    /// paused. Legacy, stale or finished captures are refused without touching anything.
    pub fn pause(&self, capture: &CaptureIdentity) -> BoxFuture<'static, Result<(), Error>> {
        self.toggle(capture, true)
    }

    pub fn resume(&self, capture: &CaptureIdentity) -> BoxFuture<'static, Result<(), Error>> {
        self.toggle(capture, false)
    }

    fn current_control(&self, capture: &CaptureIdentity) -> Option<Arc<dyn MeetingCaptureControl>> {
        use anlg_audio::meeting::CaptureAuthority;

        if !self.runtime.is_current(capture) {
            return None;
        }
        self.routes()
            .1
            .get(&capture.capture_id)
            .filter(|route| route.engine == CaptureEngine::NativeMeeting)
            .and_then(|route| route.controls.last().cloned())
    }

    fn toggle(
        &self,
        capture: &CaptureIdentity,
        paused: bool,
    ) -> BoxFuture<'static, Result<(), Error>> {
        let control = self.current_control(capture);
        let already = self
            .runtime
            .active()
            .is_some_and(|active| active.identity == *capture && active.paused == paused);
        let runtime = self.runtime.clone();
        let capture = capture.clone();
        async move {
            let Some(control) = control else {
                return Err(Error::MeetingCaptureUnavailable(
                    "this recording can't pause".into(),
                ));
            };
            if already {
                return Ok(());
            }
            if paused {
                control.pause().await?;
            } else {
                control.resume().await?;
            }
            runtime
                .set_paused(&capture, paused)
                .map_err(|error| Error::MeetingCaptureFailed(error.to_string()))
        }
        .boxed()
    }

    pub fn runtime(&self) -> &Arc<MeetingRuntime> {
        &self.runtime
    }

    pub fn capability(&self, mic_device: Option<&str>) -> MeetingCaptureCapability {
        let availability = self
            .inner
            .meeting_engine()
            .map(|engine| engine.availability());
        // Match the source actor's Bluetooth replacement before predicting which engine opens.
        let mic_device = anlg_listener_core::actors::active_mic_device(
            mic_device
                .filter(|device| !device.is_empty())
                .map(str::to_owned),
            self.inner.as_ref(),
        );
        let (engine, reason) = self.choose(mic_device.as_deref());
        MeetingCaptureCapability {
            availability,
            engine,
            reason,
        }
    }

    fn choose(&self, mic_device: Option<&str>) -> (CaptureEngine, Option<String>) {
        let Some(engine) = self.inner.meeting_engine() else {
            return (
                CaptureEngine::Legacy,
                Some("no native meeting engine on this platform".into()),
            );
        };
        match engine.availability() {
            EngineAvailability::Unavailable { reason } => (CaptureEngine::Legacy, Some(reason)),
            EngineAvailability::Available { selects_microphone } => {
                // A named microphone stays that device for the whole recording, even when it is
                // the default now: an engine that only records the default input follows the Mac
                // to whatever input becomes the default mid-recording.
                let named = mic_device.is_some_and(|device| !device.is_empty());
                if named && !selects_microphone {
                    (
                        CaptureEngine::Legacy,
                        Some("the selected microphone needs the existing engine".into()),
                    )
                } else {
                    (CaptureEngine::NativeMeeting, None)
                }
            }
        }
    }

    fn routes(&self) -> MutexGuard<'_, (u64, HashMap<String, Route>)> {
        self.routes
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// The engine this capture uses, decided at its first open and fixed afterwards.
    fn route_for(
        &self,
        capture: &CaptureIdentity,
        mic_device: Option<&str>,
        partial: bool,
    ) -> (CaptureEngine, Option<CaptureScope>) {
        let mut routes = self.routes();
        let (counter, routes) = &mut *routes;
        if let Some(route) = routes.get(&capture.capture_id) {
            return (route.engine, route.scope.clone());
        }
        let (engine, reason) = if partial {
            // The native engine records both tracks; single-track opens (onboarding) stay legacy.
            (CaptureEngine::Legacy, None)
        } else {
            self.choose(mic_device)
        };
        if let Some(reason) = reason {
            tracing::info!(%reason, "meeting_capture_using_legacy_engine");
        }
        *counter += 1;
        routes.insert(
            capture.capture_id.clone(),
            Route {
                capture: capture.clone(),
                engine,
                scope: None,
                controls: Vec::new(),
                order: *counter,
            },
        );
        (engine, None)
    }

    fn open_native(
        &self,
        capture: CaptureIdentity,
        previous: Option<CaptureScope>,
        config: CaptureConfig,
    ) -> Result<CaptureStream, Error> {
        let engine = self.inner.meeting_engine().ok_or_else(|| {
            Error::MeetingCaptureUnavailable("the native engine went away".into())
        })?;
        let staging_dir = self
            .runtime
            .staging_dir(&capture)
            .map_err(|error| Error::MeetingCaptureFailed(error.to_string()))?;
        let scope = previous
            .clone()
            .map_or(ScopeRequest::Discover, ScopeRequest::Exactly);
        // Each helper counts frames from its own start. A reopened helper's instants are shifted
        // to where it joined this capture's timeline.
        let base = match previous {
            Some(_) => self
                .runtime
                .instant_from_wall_ms(&capture, super::now_ms())
                .unwrap_or(CaptureInstant(0)),
            None => CaptureInstant(0),
        };
        let opened = engine.open(MeetingCaptureRequest {
            capture: capture.clone(),
            scope,
            staging_dir,
            config,
            locale: None,
        })?;
        // The control is routed before `confirm` announces the start, so an observer of `Started`
        // already sees a pausable capture.
        if let Some(route) = self.routes().1.get_mut(&capture.capture_id) {
            route.controls.push(opened.control.clone());
        }
        let confirmed = match &previous {
            Some(expected) if *expected != opened.scope => Err(Error::MeetingCaptureFailed(
                "the engine opened a different scope".into(),
            )),
            _ => self
                .runtime
                .confirm(&capture, opened.scope.clone(), opened.provenance)
                .map_err(|error| Error::MeetingCaptureFailed(error.to_string())),
        };
        if let Err(error) = confirmed {
            if let Some(route) = self.routes().1.get_mut(&capture.capture_id) {
                route
                    .controls
                    .retain(|control| !Arc::ptr_eq(control, &opened.control));
            }
            spawn_stop(opened.control);
            return Err(error);
        }
        if let Some(route) = self.routes().1.get_mut(&capture.capture_id) {
            route.scope = Some(opened.scope.clone());
        }
        // A new helper starts recording: one replacing a paused capture's helper pauses again.
        if self
            .runtime
            .active()
            .is_some_and(|active| active.identity == capture && active.paused)
        {
            spawn_pause(opened.control.clone());
        }
        forward_events(self.this.clone(), capture, base, opened.events);
        Ok(opened.stream)
    }

    fn open_legacy(
        &self,
        capture: &CaptureIdentity,
        sample_rate: u32,
        open: impl FnOnce() -> Result<CaptureStream, Error>,
    ) -> Result<CaptureStream, Error> {
        let stream = open()?;
        self.runtime
            .confirm(
                capture,
                CaptureScope::System,
                CaptureProvenance::legacy(sample_rate),
            )
            .map_err(|error| Error::MeetingCaptureFailed(error.to_string()))?;
        Ok(stream)
    }

    /// Ends the capture in the runtime and asks its native helper to finish its tracks.
    pub fn finish_capture(&self, capture: &CaptureIdentity, reason: CaptureEndReason) {
        self.runtime.finish(capture, reason);
        self.stop_helpers(capture);
    }

    fn stop_helpers(&self, capture: &CaptureIdentity) {
        let controls = self
            .routes()
            .1
            .get(&capture.capture_id)
            .map(|route| route.controls.clone())
            .unwrap_or_default();
        for control in controls {
            spawn_stop(control);
        }
        self.trim_routes();
    }

    fn helper_ended(&self, capture: &CaptureIdentity, reason: CaptureEndReason) {
        use anlg_audio::meeting::CaptureAuthority;

        if !self.runtime.is_current(capture) {
            return;
        }
        self.finish_capture(capture, reason);
        let handler = self
            .helper_end
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone();
        match handler {
            Some(handler) => handler(capture, reason),
            None => tracing::warn!(?reason, "native_meeting_helper_end_without_handler"),
        }
    }

    /// Anarlog durably saved this session's audio: ended captures may drop their raw staging.
    pub fn adopt(&self, session_id: &str) -> usize {
        let adopted: Vec<Route> = {
            let mut routes = self.routes();
            let ids: Vec<String> = routes
                .1
                .values()
                .filter(|route| {
                    route.capture.session_id == session_id
                        && self.runtime.active_for_session(session_id).as_ref()
                            != Some(&route.capture)
                })
                .map(|route| route.capture.capture_id.clone())
                .collect();
            ids.iter().filter_map(|id| routes.1.remove(id)).collect()
        };
        for route in &adopted {
            for control in &route.controls {
                if let Err(error) = control.release_staging() {
                    tracing::warn!(%error, "native_meeting_staging_release_failed");
                }
            }
            if let Err(error) =
                staging::remove_staging(self.runtime.staging_root(), &route.capture.capture_id)
            {
                tracing::warn!(%error, "native_meeting_staging_remove_failed");
            }
        }
        adopted.len()
    }

    fn trim_routes(&self) {
        let active = self
            .runtime
            .active()
            .map(|active| active.identity.capture_id);
        let mut routes = self.routes();
        while routes.1.len() > ROUTES_KEPT {
            let Some(oldest) = routes
                .1
                .values()
                .filter(|route| Some(&route.capture.capture_id) != active.as_ref())
                .min_by_key(|route| route.order)
                .map(|route| route.capture.capture_id.clone())
            else {
                break;
            };
            routes.1.remove(&oldest);
        }
    }

    pub fn engine_of(&self, capture: &CaptureIdentity) -> Option<CaptureEngine> {
        self.routes()
            .1
            .get(&capture.capture_id)
            .map(|route| route.engine)
    }
}

fn spawn_stop(control: Arc<dyn MeetingCaptureControl>) {
    let stop = control.stop();
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.spawn(async move {
                if let Err(error) = stop.await {
                    tracing::warn!(%error, "native_meeting_capture_stop_failed");
                }
            });
        }
        Err(_) => tracing::warn!("native_meeting_capture_stop_without_runtime"),
    }
}

fn spawn_pause(control: Arc<dyn MeetingCaptureControl>) {
    let pause = control.pause();
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.spawn(async move {
                if let Err(error) = pause.await {
                    tracing::warn!(%error, "native_meeting_capture_repause_failed");
                }
            });
        }
        Err(_) => tracing::warn!("native_meeting_capture_repause_without_runtime"),
    }
}

fn rebase(event: MeetingCaptureEvent, base: CaptureInstant) -> MeetingCaptureEvent {
    use MeetingCaptureEvent::*;

    let shift = |at: CaptureInstant| CaptureInstant(at.0.saturating_add(base.0));
    match event {
        Observation(mut observation) => {
            observation.at = shift(observation.at);
            Observation(observation)
        }
        ObservationUnavailable { at, reason } => ObservationUnavailable {
            at: shift(at),
            reason,
        },
        Paused { at } => Paused { at: shift(at) },
        Resumed { at } => Resumed { at: shift(at) },
        MicProcessingChanged { at, mic } => MicProcessingChanged { at: shift(at), mic },
        Ended { at, reason } => Ended {
            at: shift(at),
            reason,
        },
        event @ TrackStatus { .. } => event,
    }
}

fn forward_events(
    router: Weak<MeetingAudioRouter>,
    capture: CaptureIdentity,
    base: CaptureInstant,
    mut events: MeetingEventStream,
) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        tracing::warn!("native_meeting_events_without_runtime");
        return;
    };
    handle.spawn(async move {
        while let Some(event) = events.next().await {
            let event = rebase(event, base);
            let Some(owner) = router.upgrade() else {
                return;
            };
            let runtime = &owner.runtime;
            if !matches!(event, MeetingCaptureEvent::Observation(_))
                && let Err(error) = runtime.record_helper_event(&capture, &event)
            {
                tracing::debug!(%error, "native_meeting_evidence_dropped");
            }
            let result = match event {
                MeetingCaptureEvent::Observation(observation) if observation.capture == capture => {
                    runtime.observe(observation)
                }
                MeetingCaptureEvent::Observation(_) => {
                    tracing::warn!("native_meeting_observation_for_other_capture");
                    Ok(())
                }
                MeetingCaptureEvent::Paused { .. } => runtime.set_paused(&capture, true),
                MeetingCaptureEvent::Resumed { .. } => runtime.set_paused(&capture, false),
                MeetingCaptureEvent::MicProcessingChanged { mic, .. } => {
                    tracing::info!(?mic, "native_meeting_mic_processing_changed");
                    runtime.set_mic_processing(&capture, mic)
                }
                MeetingCaptureEvent::TrackStatus { track, status } => {
                    tracing::info!(?track, ?status, "native_meeting_track_status");
                    Ok(())
                }
                MeetingCaptureEvent::ObservationUnavailable { reason, .. } => {
                    tracing::debug!(%reason, "native_meeting_observation_unavailable");
                    Ok(())
                }
                MeetingCaptureEvent::Ended { reason, .. } => {
                    tracing::info!(?reason, "native_meeting_helper_ended");
                    if matches!(
                        reason,
                        CaptureEndReason::AppClosed | CaptureEndReason::DurationLimit
                    ) {
                        owner.helper_ended(&capture, reason);
                    }
                    Ok(())
                }
            };
            if let Err(error) = result {
                tracing::debug!(%error, "native_meeting_event_dropped");
            }
        }
    });
}

impl AudioProvider for MeetingAudioRouter {
    fn open_capture(&self, config: CaptureConfig) -> Result<CaptureStream, Error> {
        let active = self
            .runtime
            .active()
            .ok_or_else(|| Error::MeetingCaptureFailed("the recording has already ended".into()))?;
        let capture = active.identity;
        match self.route_for(&capture, config.mic_device.as_deref(), false) {
            (CaptureEngine::NativeMeeting, previous) => self.open_native(capture, previous, config),
            (CaptureEngine::Legacy, _) => {
                let sample_rate = config.sample_rate;
                self.open_legacy(&capture, sample_rate, || self.inner.open_capture(config))
            }
        }
    }

    fn open_speaker_capture(
        &self,
        sample_rate: u32,
        chunk_size: usize,
    ) -> Result<CaptureStream, Error> {
        let active = self
            .runtime
            .active()
            .ok_or_else(|| Error::MeetingCaptureFailed("the recording has already ended".into()))?;
        match self.route_for(&active.identity, None, true) {
            (CaptureEngine::NativeMeeting, _) => Err(Error::MeetingCaptureUnavailable(
                "the native engine records both tracks".into(),
            )),
            (CaptureEngine::Legacy, _) => self.open_legacy(&active.identity, sample_rate, || {
                self.inner.open_speaker_capture(sample_rate, chunk_size)
            }),
        }
    }

    fn open_mic_capture(
        &self,
        device: Option<String>,
        sample_rate: u32,
        chunk_size: usize,
    ) -> Result<CaptureStream, Error> {
        let active = self
            .runtime
            .active()
            .ok_or_else(|| Error::MeetingCaptureFailed("the recording has already ended".into()))?;
        match self.route_for(&active.identity, device.as_deref(), true) {
            (CaptureEngine::NativeMeeting, _) => Err(Error::MeetingCaptureUnavailable(
                "the native engine records both tracks".into(),
            )),
            (CaptureEngine::Legacy, _) => self.open_legacy(&active.identity, sample_rate, || {
                self.inner.open_mic_capture(device, sample_rate, chunk_size)
            }),
        }
    }

    fn default_device_name(&self) -> String {
        self.inner.default_device_name()
    }

    fn list_mic_devices(&self) -> Vec<String> {
        self.inner.list_mic_devices()
    }

    fn play_silence(&self) -> std::sync::mpsc::Sender<()> {
        self.inner.play_silence()
    }

    fn play_bytes(&self, bytes: &'static [u8]) -> std::sync::mpsc::Sender<()> {
        self.inner.play_bytes(bytes)
    }

    fn probe_mic(&self, device: Option<String>) -> Result<(), Error> {
        self.inner.probe_mic(device)
    }

    fn probe_speaker(&self) -> Result<(), Error> {
        self.inner.probe_speaker()
    }

    fn meeting_engine(&self) -> Option<Arc<dyn MeetingCaptureEngine>> {
        self.inner.meeting_engine()
    }
}
