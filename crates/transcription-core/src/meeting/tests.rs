use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anlg_audio::meeting::*;
use anlg_audio::{AudioProvider, CaptureConfig, CaptureEvent, CaptureFrame, CaptureStream, Error};
use futures_util::{FutureExt, stream};

use super::*;

const MEET: &str = "https://meet.google.com/abc-defg-hij";

fn meet_call() -> CaptureScope {
    CaptureScope::Call(ConfirmedCall {
        pid: 42,
        bundle_id: "com.google.Chrome".into(),
        platform: CallPlatform::GoogleMeet,
        call_url: Some(MEET.into()),
    })
}

fn native() -> CaptureProvenance {
    CaptureProvenance {
        engine: CaptureEngine::NativeMeeting,
        source_sample_rate: NATIVE_SAMPLE_RATE,
        mic: MicProcessing::VoiceProcessed,
    }
}

fn observation(capture: &CaptureIdentity, source: ObservationSource) -> SpeakerObservation {
    SpeakerObservation {
        capture: capture.clone(),
        source,
        platform: CallPlatform::GoogleMeet,
        at: CaptureInstant(0),
        participants: vec![],
        roster_complete: false,
        participant_count: None,
        speaking: Some(vec![]),
    }
}

fn runtime() -> (tempfile::TempDir, Arc<MeetingRuntime>) {
    let dir = tempfile::tempdir().unwrap();
    let runtime = MeetingRuntime::new(dir.path().join("staging"));
    (dir, runtime)
}

#[test]
fn an_earlier_recording_of_the_same_session_cannot_act_for_the_next() {
    let (_dir, runtime) = runtime();
    let first = runtime
        .begin("acct", "session", LifecycleOwner::App)
        .unwrap();
    runtime.confirm(&first, meet_call(), native()).unwrap();
    runtime.finish(&first, CaptureEndReason::UserStopped);

    let second = runtime
        .begin("acct", "session", LifecycleOwner::App)
        .unwrap();
    assert_ne!(first.capture_id, second.capture_id);
    runtime
        .confirm(&second, CaptureScope::System, native())
        .unwrap();

    assert!(!runtime.is_current(&first));
    assert_eq!(runtime.confirmed_call(&first), None);
    assert_eq!(
        runtime.observe(observation(&first, ObservationSource::Accessibility)),
        Err(MeetingError::Stale)
    );
    assert_eq!(runtime.link(&first, MEET), Err(MeetingError::Stale));
    // A late finish of the earlier capture leaves the current one running.
    runtime.finish(&first, CaptureEndReason::CaptureFailed);
    assert!(runtime.is_current(&second));
}

#[test]
fn signing_out_or_switching_ends_the_capture_at_once_but_signing_in_keeps_a_local_one() {
    let (_dir, runtime) = runtime();
    let recorded = Arc::new(Recorded::default());
    runtime.register_observer(recorded.clone());

    // Recording signed out, in a library whose owner is not the account that then signs in.
    let local = runtime
        .begin("local-owner", "s1", LifecycleOwner::App)
        .unwrap();
    runtime.confirm(&local, meet_call(), native()).unwrap();
    assert_eq!(runtime.set_account(Some("acct".into())), None);
    assert_eq!(runtime.set_account(Some("acct".into())), None);
    assert!(runtime.is_current(&local));
    assert_eq!(runtime.confirmed_call(&local), meet_call().call().cloned());

    // Signing out ends it before returning, and observers hear why.
    assert_eq!(runtime.set_account(None), Some(local.clone()));
    assert!(!runtime.is_current(&local));
    assert_eq!(
        runtime.observe(observation(&local, ObservationSource::Accessibility)),
        Err(MeetingError::Stale)
    );
    assert!(runtime.evidence(&local).unwrap().ended_at_ms.is_some());
    let events = recorded.0.lock().unwrap().clone();
    assert!(matches!(
        events.last(),
        Some((
            MeetingLifecycleEvent::Ended {
                reason: CaptureEndReason::AccountChanged,
                ..
            },
            false
        ))
    ));

    // A mismatched library still records under the signed-in account; a switch ends it.
    runtime.set_account(Some("acct".into()));
    let mismatched = runtime
        .begin("other-library", "s2", LifecycleOwner::App)
        .unwrap();
    assert_eq!(
        runtime.set_account(Some("other".into())),
        Some(mismatched.clone())
    );
    assert!(!runtime.is_current(&mismatched));
    assert_eq!(
        runtime.confirm(&mismatched, meet_call(), native()),
        Err(MeetingError::Stale)
    );
    assert_eq!(runtime.set_account(Some("other".into())), None);
}

/// Only the record path's begin carrying the reserved request id, for the reserved note, takes
/// the reservation; the person's own start of that note stays the app's, and a withdrawn,
/// replaced or account-changed request is refused rather than begun as anything else.
#[test]
fn an_extension_reservation_is_taken_only_by_its_own_requests_begin() {
    let (_dir, runtime) = runtime();
    runtime.set_account(Some("acct".into()));
    let capture_id = runtime.reserve_extension_start("tab", "note").unwrap();
    assert_eq!(
        runtime.reserve_extension_start("other-tab", "another"),
        Err(MeetingError::CaptureActive)
    );

    // The person's own start of the reserved note, before the window's.
    let own = runtime.begin_recording("acct", "note", None).unwrap();
    assert_eq!(runtime.active().unwrap().owner, LifecycleOwner::App);
    assert_eq!(
        runtime.begin_recording("acct", "note", Some("tab")),
        Err(MeetingError::CaptureActive)
    );
    assert_eq!(runtime.cancel_extension_start("tab"), None, "withdrawn");
    runtime.finish(&own, CaptureEndReason::UserStopped);
    assert_eq!(
        runtime.begin_recording("acct", "note", Some("tab")),
        Err(MeetingError::StaleExtensionStart)
    );
    assert!(runtime.active().is_none());

    // Another request, or this request for another note, never takes it.
    let capture_id_2 = runtime.reserve_extension_start("tab-2", "note").unwrap();
    assert_ne!(capture_id, capture_id_2);
    for (session, request) in [("note", "tab"), ("another", "tab-2")] {
        assert_eq!(
            runtime.begin_recording("acct", session, Some(request)),
            Err(MeetingError::StaleExtensionStart)
        );
    }
    let tab = runtime
        .begin_recording("acct", "note", Some("tab-2"))
        .unwrap();
    assert_eq!(tab.capture_id, capture_id_2);
    assert_eq!(runtime.active().unwrap().owner, LifecycleOwner::Extension);
    // Taken once: cancelling now names exactly that capture.
    assert_eq!(runtime.cancel_extension_start("tab"), None);
    assert_eq!(runtime.cancel_extension_start("tab-2"), Some(tab.clone()));
    runtime.finish(&tab, CaptureEndReason::UserStopped);
    assert_eq!(runtime.cancel_extension_start("tab-2"), None);

    // Any account change drops it, signing in included.
    runtime.reserve_extension_start("tab-3", "note").unwrap();
    runtime.set_account(Some("other".into()));
    runtime.set_account(None);
    runtime.reserve_extension_start("tab-4", "note").unwrap();
    runtime.set_account(Some("acct".into()));
    for request in ["tab-3", "tab-4"] {
        assert_eq!(
            runtime.begin_recording("acct", "note", Some(request)),
            Err(MeetingError::StaleExtensionStart)
        );
    }
}

#[test]
fn a_reopened_capture_cannot_change_its_confirmed_scope() {
    let (_dir, runtime) = runtime();
    let capture = runtime
        .begin("acct", "session", LifecycleOwner::App)
        .unwrap();
    runtime.confirm(&capture, meet_call(), native()).unwrap();
    runtime.confirm(&capture, meet_call(), native()).unwrap();
    assert_eq!(
        runtime.confirm(&capture, CaptureScope::System, native()),
        Err(MeetingError::CallMismatch)
    );
}

#[test]
fn only_the_confirmed_meet_call_can_be_linked_and_relinking_stales_the_old_tab() {
    let (_dir, runtime) = runtime();
    let capture = runtime
        .begin("acct", "session", LifecycleOwner::App)
        .unwrap();
    assert_eq!(
        runtime.link(&capture, MEET),
        Err(MeetingError::NoConfirmedCall)
    );
    runtime.confirm(&capture, meet_call(), native()).unwrap();
    assert_eq!(
        runtime.link(&capture, "https://meet.google.com/zzz-zzzz-zzz"),
        Err(MeetingError::CallMismatch)
    );
    assert!(!runtime.extension_handles_actions(&capture));

    let old = runtime.link(&capture, MEET).unwrap();
    let new = runtime.link(&capture, MEET).unwrap();
    assert_eq!(runtime.resolve_link(&old), Err(MeetingError::StaleLink));
    assert_eq!(runtime.resolve_link(&new), Ok(capture.clone()));

    // Detaching an app-owned recording keeps it, and the extension keeps the call's actions.
    assert_eq!(runtime.detach(&new), Ok(DetachOutcome::KeepRecording));
    assert!(runtime.is_current(&capture));
    assert!(runtime.extension_handles_actions(&capture));
    assert_eq!(runtime.resolve_link(&new), Err(MeetingError::StaleLink));
}

#[test]
fn detaching_stops_only_extension_owned_recordings() {
    let (_dir, runtime) = runtime();
    let capture = runtime
        .begin("acct", "session", LifecycleOwner::Extension)
        .unwrap();
    runtime.confirm(&capture, meet_call(), native()).unwrap();
    let token = runtime.link(&capture, MEET).unwrap();
    assert_eq!(runtime.detach(&token), Ok(DetachOutcome::StopRecording));
    assert_eq!(runtime.resolve_link(&token), Err(MeetingError::StaleLink));
    assert!(runtime.extension_handles_actions(&capture));
}

#[test]
fn speaker_evidence_follows_its_capture_and_closes_on_every_end() {
    let (_dir, runtime) = runtime();
    let capture = runtime
        .begin("acct", "session", LifecycleOwner::App)
        .unwrap();
    runtime.set_transcript_start(&capture, 1_000).unwrap();
    assert_eq!(
        runtime.evidence(&capture),
        None,
        "no evidence before the engine opened"
    );
    runtime.confirm(&capture, meet_call(), native()).unwrap();
    runtime.transcription_connected(&capture).unwrap();
    runtime.transcription_connected(&capture).unwrap();
    runtime
        .observe(observation(&capture, ObservationSource::Accessibility))
        .unwrap();

    let open = runtime.evidence(&capture).unwrap();
    assert_eq!(open.capture_id, capture.capture_id);
    assert_eq!(open.started_at_ms, 1_000);
    assert_eq!(open.ended_at_ms, None);
    assert_eq!(
        open.provider_restarts_ms.len(),
        1,
        "the first connection is not a restart"
    );

    runtime.finish(&capture, CaptureEndReason::CaptureFailed);
    assert!(runtime.evidence(&capture).unwrap().ended_at_ms.is_some());
    let next = runtime
        .begin("acct", "session", LifecycleOwner::App)
        .unwrap();
    assert_eq!(runtime.evidence(&next), None);
}

#[test]
fn system_scope_never_authorizes_attachment() {
    let (_dir, runtime) = runtime();
    let capture = runtime
        .begin("acct", "session", LifecycleOwner::Extension)
        .unwrap();
    runtime
        .confirm(
            &capture,
            CaptureScope::System,
            CaptureProvenance::legacy(16_000),
        )
        .unwrap();
    assert_eq!(
        runtime.link(&capture, MEET),
        Err(MeetingError::NoConfirmedCall)
    );
}

#[test]
fn evidence_is_scoped_to_its_capture_source_and_unpaused_audio() {
    let (_dir, runtime) = runtime();
    let capture = runtime
        .begin("acct", "session", LifecycleOwner::App)
        .unwrap();
    runtime.confirm(&capture, meet_call(), native()).unwrap();

    let mut zoom = observation(&capture, ObservationSource::Accessibility);
    zoom.platform = CallPlatform::Zoom;
    assert_eq!(runtime.observe(zoom), Err(MeetingError::CallMismatch));
    assert_eq!(
        runtime.observe(observation(&capture, ObservationSource::BrowserExtension)),
        Err(MeetingError::CallMismatch),
        "browser cues need an attached tab"
    );
    runtime
        .observe(observation(&capture, ObservationSource::Accessibility))
        .unwrap();
    runtime.set_paused(&capture, true).unwrap();
    assert_eq!(
        runtime.observe(observation(&capture, ObservationSource::Accessibility)),
        Err(MeetingError::Paused)
    );
    runtime.finish(&capture, CaptureEndReason::UserStopped);

    assert_eq!(runtime.observations(&capture).len(), 1);
    let next = runtime
        .begin("acct", "session", LifecycleOwner::App)
        .unwrap();
    assert!(runtime.observations(&next).is_empty());
}

#[derive(Default)]
struct Recorded(StdMutex<Vec<(MeetingLifecycleEvent, bool)>>);

impl MeetingLifecycleObserver for Recorded {
    fn on_lifecycle(&self, event: &MeetingLifecycleEvent, authority: Arc<dyn CaptureAuthority>) {
        let current = authority.is_current(event.capture());
        self.0.lock().unwrap().push((event.clone(), current));
    }
}

#[test]
fn observers_hear_the_confirmed_start_and_a_final_end() {
    let (_dir, runtime) = runtime();
    let recorded = Arc::new(Recorded::default());
    runtime.register_observer(recorded.clone());

    let capture = runtime
        .begin("acct", "session", LifecycleOwner::App)
        .unwrap();
    assert!(
        recorded.0.lock().unwrap().is_empty(),
        "nothing before confirmation"
    );
    runtime.confirm(&capture, meet_call(), native()).unwrap();
    runtime.finish(&capture, CaptureEndReason::AppClosed);
    runtime.finish(&capture, CaptureEndReason::AppClosed);

    let events = recorded.0.lock().unwrap().clone();
    assert_eq!(events.len(), 2);
    assert!(matches!(
        &events[0],
        (MeetingLifecycleEvent::Started { scope, engine: CaptureEngine::NativeMeeting, .. }, true)
            if *scope == meet_call()
    ));
    assert!(matches!(
        &events[1],
        (
            MeetingLifecycleEvent::Ended {
                reason: CaptureEndReason::AppClosed,
                ..
            },
            false
        )
    ));
}

/// Ends the capture from inside its own start notification, as an observer reacting at once would.
struct FinishOnStart(Weak<MeetingRuntime>);

impl MeetingLifecycleObserver for FinishOnStart {
    fn on_lifecycle(&self, event: &MeetingLifecycleEvent, _: Arc<dyn CaptureAuthority>) {
        if let (MeetingLifecycleEvent::Started { capture, .. }, Some(runtime)) =
            (event, self.0.upgrade())
        {
            runtime.finish(capture, CaptureEndReason::CaptureFailed);
        }
    }
}

#[test]
fn every_observer_hears_a_start_before_the_end_that_follows_it() {
    let (_dir, runtime) = runtime();
    runtime.register_observer(Arc::new(FinishOnStart(Arc::downgrade(&runtime))));
    let recorded = Arc::new(Recorded::default());
    runtime.register_observer(recorded.clone());

    let capture = runtime
        .begin("acct", "session", LifecycleOwner::App)
        .unwrap();
    runtime.confirm(&capture, meet_call(), native()).unwrap();

    let events = recorded.0.lock().unwrap().clone();
    assert!(
        matches!(
            &events[..],
            [
                (MeetingLifecycleEvent::Started { .. }, _),
                (MeetingLifecycleEvent::Ended { .. }, false)
            ]
        ),
        "{events:?}"
    );
}

#[derive(Default)]
struct FakeControl {
    released: AtomicUsize,
    stopped: AtomicUsize,
    paused: AtomicUsize,
    resumed: AtomicUsize,
}

impl MeetingCaptureControl for FakeControl {
    fn pause(&self) -> futures_util::future::BoxFuture<'static, Result<(), Error>> {
        self.paused.fetch_add(1, Ordering::SeqCst);
        async { Ok(()) }.boxed()
    }
    fn resume(&self) -> futures_util::future::BoxFuture<'static, Result<(), Error>> {
        self.resumed.fetch_add(1, Ordering::SeqCst);
        async { Ok(()) }.boxed()
    }
    fn stop(&self) -> futures_util::future::BoxFuture<'static, Result<(), Error>> {
        self.stopped.fetch_add(1, Ordering::SeqCst);
        async { Ok(()) }.boxed()
    }
    fn release_staging(&self) -> Result<(), Error> {
        self.released.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[derive(Default)]
struct FakeEngine {
    selects_microphone: bool,
    /// No helper is installed.
    unavailable: AtomicBool,
    fail: bool,
    /// Helper events after the first observation, for every open.
    events: Vec<MeetingCaptureEvent>,
    requests: StdMutex<Vec<(ScopeRequest, std::path::PathBuf)>>,
    controls: StdMutex<Vec<Arc<FakeControl>>>,
}

impl MeetingCaptureEngine for FakeEngine {
    fn availability(&self) -> EngineAvailability {
        if self.unavailable.load(Ordering::SeqCst) {
            return EngineAvailability::Unavailable {
                reason: "no helper".into(),
            };
        }
        EngineAvailability::Available {
            selects_microphone: self.selects_microphone,
        }
    }

    fn calls_in_progress(&self) -> Result<Vec<CallApp>, Error> {
        Ok(vec![])
    }

    fn open(&self, request: MeetingCaptureRequest) -> Result<MeetingCapture, Error> {
        self.requests
            .lock()
            .unwrap()
            .push((request.scope.clone(), request.staging_dir.clone()));
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(Error::MeetingCaptureUnavailable("no helper".into()));
        }
        if self.fail {
            return Err(Error::MeetingCaptureFailed("helper exited".into()));
        }
        let control = Arc::new(FakeControl::default());
        self.controls.lock().unwrap().push(control.clone());
        let frame = CaptureFrame {
            raw_mic: Arc::from(vec![0.0; request.config.chunk_size]),
            raw_speaker: Arc::from(vec![0.0; request.config.chunk_size]),
            aec_mic: None,
        };
        let mut observed = observation(&request.capture, ObservationSource::Accessibility);
        observed.speaking = None;
        Ok(MeetingCapture {
            stream: CaptureStream::with_events(stream::iter([Ok(CaptureEvent::Frame(frame))])),
            scope: match request.scope {
                ScopeRequest::Exactly(scope) => scope,
                ScopeRequest::Discover => meet_call(),
            },
            provenance: native(),
            events: Box::pin(stream::iter(
                std::iter::once(MeetingCaptureEvent::Observation(observed))
                    .chain(self.events.clone()),
            )),
            control,
        })
    }
}

struct FakeAudio {
    engine: Arc<FakeEngine>,
    legacy_opens: AtomicUsize,
}

impl AudioProvider for FakeAudio {
    fn open_capture(&self, _: CaptureConfig) -> Result<CaptureStream, Error> {
        self.legacy_opens.fetch_add(1, Ordering::SeqCst);
        Ok(CaptureStream::with_events(stream::empty()))
    }
    fn open_speaker_capture(&self, _: u32, _: usize) -> Result<CaptureStream, Error> {
        self.legacy_opens.fetch_add(1, Ordering::SeqCst);
        Ok(CaptureStream::with_events(stream::empty()))
    }
    fn open_mic_capture(
        &self,
        _: Option<String>,
        _: u32,
        _: usize,
    ) -> Result<CaptureStream, Error> {
        self.legacy_opens.fetch_add(1, Ordering::SeqCst);
        Ok(CaptureStream::with_events(stream::empty()))
    }
    fn default_device_name(&self) -> String {
        "Built-in".into()
    }
    fn list_mic_devices(&self) -> Vec<String> {
        vec!["Built-in".into(), "USB".into()]
    }
    fn play_silence(&self) -> std::sync::mpsc::Sender<()> {
        std::sync::mpsc::channel().0
    }
    fn play_bytes(&self, _: &'static [u8]) -> std::sync::mpsc::Sender<()> {
        std::sync::mpsc::channel().0
    }
    fn probe_mic(&self, _: Option<String>) -> Result<(), Error> {
        Ok(())
    }
    fn probe_speaker(&self) -> Result<(), Error> {
        Ok(())
    }
    fn meeting_engine(&self) -> Option<Arc<dyn MeetingCaptureEngine>> {
        Some(self.engine.clone())
    }
}

fn router(engine: FakeEngine) -> (tempfile::TempDir, Arc<FakeAudio>, Arc<MeetingAudioRouter>) {
    let (dir, runtime) = runtime();
    let audio = Arc::new(FakeAudio {
        engine: Arc::new(engine),
        legacy_opens: AtomicUsize::new(0),
    });
    let router = MeetingAudioRouter::new(audio.clone(), runtime);
    (dir, audio, router)
}

fn config(mic_device: Option<&str>) -> CaptureConfig {
    CaptureConfig {
        sample_rate: 16_000,
        chunk_size: 320,
        mic_device: mic_device.map(str::to_string),
        enable_aec: true,
    }
}

#[tokio::test]
async fn the_default_input_records_natively_when_the_helper_is_installed_and_keeps_its_engine() {
    let (_dir, audio, router) = router(FakeEngine::default());
    let runtime = router.runtime().clone();

    // Without the helper the existing engine records, says why, and keeps recording the whole
    // capture even once a helper appears.
    audio.engine.unavailable.store(true, Ordering::SeqCst);
    let capability = router.capability(None);
    assert_eq!(capability.engine, CaptureEngine::Legacy);
    assert_eq!(capability.reason.as_deref(), Some("no helper"));
    let legacy = runtime.begin("acct", "s1", LifecycleOwner::App).unwrap();
    router.open_capture(config(None)).unwrap();
    assert_eq!(router.engine_of(&legacy), Some(CaptureEngine::Legacy));
    audio.engine.unavailable.store(false, Ordering::SeqCst);
    router.open_capture(config(None)).unwrap();
    assert_eq!(audio.legacy_opens.load(Ordering::SeqCst), 2);
    assert!(audio.engine.requests.lock().unwrap().is_empty());
    router.finish_capture(&legacy, CaptureEndReason::UserStopped);

    let capability = router.capability(None);
    assert_eq!(capability.engine, CaptureEngine::NativeMeeting);
    assert_eq!(capability.reason, None);
    let capture = runtime.begin("acct", "s2", LifecycleOwner::App).unwrap();
    let mut stream = router.open_capture(config(None)).unwrap();
    assert!(matches!(
        stream.next_event().await,
        Some(Ok(CaptureEvent::Frame(_)))
    ));
    assert_eq!(
        runtime.confirmed_call(&capture),
        meet_call().call().cloned()
    );
    router.open_capture(config(None)).unwrap();
    assert_eq!(audio.legacy_opens.load(Ordering::SeqCst), 2);
    // A helper lost mid-capture fails the reopen; it never reopens as the existing engine.
    audio.engine.unavailable.store(true, Ordering::SeqCst);
    assert!(matches!(
        router.open_capture(config(None)),
        Err(Error::MeetingCaptureUnavailable(_))
    ));
    assert_eq!(audio.legacy_opens.load(Ordering::SeqCst), 2);
    assert_eq!(router.engine_of(&capture), Some(CaptureEngine::NativeMeeting));

    let requests = audio.engine.requests.lock().unwrap().clone();
    assert_eq!(requests[0].0, ScopeRequest::Discover);
    assert_eq!(requests[1].0, ScopeRequest::Exactly(meet_call()));
    assert_ne!(
        requests[0].1, requests[1].1,
        "each open gets a new staging directory"
    );

    tokio::task::yield_now().await;
    assert_eq!(runtime.observations(&capture).len(), 2);
}

#[tokio::test]
async fn a_native_failure_is_reported_instead_of_falling_back() {
    let (_dir, audio, router) = router(FakeEngine {
        fail: true,
        ..Default::default()
    });
    let capture = router
        .runtime()
        .begin("acct", "s", LifecycleOwner::App)
        .unwrap();
    assert!(matches!(
        router.open_capture(config(None)),
        Err(Error::MeetingCaptureFailed(_))
    ));
    assert!(router.open_capture(config(None)).is_err());
    assert_eq!(audio.legacy_opens.load(Ordering::SeqCst), 0);
    assert_eq!(router.runtime().confirmed_call(&capture), None);
}

#[tokio::test]
async fn a_named_microphone_stays_on_the_engine_that_can_select_it() {
    let (_dir, audio, router) = router(FakeEngine::default());
    assert_eq!(router.capability(None).engine, CaptureEngine::NativeMeeting);
    assert_eq!(router.capability(Some("USB")).engine, CaptureEngine::Legacy);
    // Naming today's default still pins it: the native engine would follow the next default.
    assert_eq!(
        router.capability(Some("Built-in")).engine,
        CaptureEngine::Legacy
    );

    for (session, device) in [("s1", "USB"), ("s2", "Built-in")] {
        let capture = router
            .runtime()
            .begin("acct", session, LifecycleOwner::App)
            .unwrap();
        router.open_capture(config(Some(device))).unwrap();
        assert_eq!(router.engine_of(&capture), Some(CaptureEngine::Legacy));
        router.finish_capture(&capture, CaptureEndReason::UserStopped);
    }
    assert_eq!(audio.legacy_opens.load(Ordering::SeqCst), 2);
    assert!(audio.engine.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn staging_is_released_only_after_the_ended_capture_is_adopted() {
    let (_dir, audio, router) = router(FakeEngine::default());
    let runtime = router.runtime().clone();
    let capture = runtime.begin("acct", "s", LifecycleOwner::App).unwrap();
    router.open_capture(config(None)).unwrap();
    let staging = runtime.staging_root().join(&capture.capture_id);
    assert!(staging.is_dir());

    assert_eq!(router.adopt("s"), 0, "a running capture keeps its staging");
    router.finish_capture(&capture, CaptureEndReason::UserStopped);
    tokio::task::yield_now().await;
    let control = audio.engine.controls.lock().unwrap()[0].clone();
    assert_eq!(control.stopped.load(Ordering::SeqCst), 1);

    assert_eq!(router.adopt("s"), 1);
    assert_eq!(control.released.load(Ordering::SeqCst), 1);
    assert!(!staging.exists());
}

#[test]
fn the_sweep_keeps_active_and_unadopted_staging() {
    let (dir, runtime) = runtime();
    let root = runtime.staging_root().to_path_buf();
    let identity = |id: &str| CaptureIdentity {
        account_id: "acct".into(),
        session_id: format!("session-{id}"),
        capture_id: id.into(),
    };
    for id in ["active", "adopted", "pending"] {
        staging::create_staging_dir(&root, &identity(id)).unwrap();
    }
    std::fs::create_dir_all(root.join("unreadable")).unwrap();

    let removed = staging::sweep_staging(
        &root,
        Some("active"),
        |staged| staged.capture.session_id == "session-adopted",
        STAGING_MAX_AGE,
    )
    .unwrap();
    assert_eq!(removed, vec!["adopted".to_string()]);
    for kept in ["active", "pending", "unreadable"] {
        assert!(root.join(kept).is_dir(), "{kept} was removed");
    }
    assert!(staging::create_staging_dir(&root, &identity("../escape")).is_err());
    drop(dir);
}

#[tokio::test]
async fn pausing_reaches_the_native_helper_and_is_refused_for_legacy_captures() {
    let (_dir, audio, router) = router(FakeEngine::default());
    let runtime = router.runtime().clone();

    let legacy = runtime.begin("acct", "s1", LifecycleOwner::App).unwrap();
    router.open_capture(config(Some("USB"))).unwrap();
    assert!(!router.can_pause(&legacy));
    assert!(matches!(
        router.pause(&legacy).await,
        Err(Error::MeetingCaptureUnavailable(_))
    ));
    assert!(!runtime.active().unwrap().paused);
    router.finish_capture(&legacy, CaptureEndReason::UserStopped);

    // Whoever hears the start already sees a capture that can pause.
    let pausable = Arc::new(StdMutex::new(None));
    struct SeesPause(Weak<MeetingAudioRouter>, Arc<StdMutex<Option<bool>>>);
    impl MeetingLifecycleObserver for SeesPause {
        fn on_lifecycle(&self, event: &MeetingLifecycleEvent, _: Arc<dyn CaptureAuthority>) {
            if let (MeetingLifecycleEvent::Started { capture, .. }, Some(router)) =
                (event, self.0.upgrade())
            {
                *self.1.lock().unwrap() = Some(router.can_pause(capture));
            }
        }
    }
    runtime.register_observer(Arc::new(SeesPause(
        Arc::downgrade(&router),
        pausable.clone(),
    )));
    let capture = runtime.begin("acct", "s2", LifecycleOwner::App).unwrap();
    router.open_capture(config(None)).unwrap();
    assert_eq!(*pausable.lock().unwrap(), Some(true));

    router.pause(&capture).await.unwrap();
    assert!(runtime.active().unwrap().paused);
    router.pause(&capture).await.unwrap();
    let control = audio.engine.controls.lock().unwrap()[0].clone();
    assert_eq!(control.paused.load(Ordering::SeqCst), 1, "already paused");

    // A helper replacing a paused capture's helper pauses again.
    router.open_capture(config(None)).unwrap();
    tokio::task::yield_now().await;
    let reopened = audio.engine.controls.lock().unwrap()[1].clone();
    assert_eq!(reopened.paused.load(Ordering::SeqCst), 1);

    router.resume(&capture).await.unwrap();
    assert!(!runtime.active().unwrap().paused);
    assert_eq!(reopened.resumed.load(Ordering::SeqCst), 1);

    router.finish_capture(&capture, CaptureEndReason::UserStopped);
    assert!(!router.can_pause(&capture));
    assert!(router.pause(&capture).await.is_err());
}

#[tokio::test]
async fn a_helper_ending_the_call_or_its_length_ends_the_recording_once() {
    for reason in [CaptureEndReason::AppClosed, CaptureEndReason::DurationLimit] {
        let (_dir, _audio, router) = router(FakeEngine {
            events: vec![MeetingCaptureEvent::Ended {
                at: CaptureInstant(48_000),
                reason,
            }],
            ..Default::default()
        });
        let runtime = router.runtime().clone();
        let recorded = Arc::new(Recorded::default());
        runtime.register_observer(recorded.clone());
        let stops = Arc::new(StdMutex::new(Vec::new()));
        let seen = stops.clone();
        router.on_helper_end(move |capture, reason| {
            seen.lock().unwrap().push((capture.clone(), reason));
        });

        let capture = runtime.begin("acct", "s", LifecycleOwner::App).unwrap();
        router.open_capture(config(None)).unwrap();
        tokio::task::yield_now().await;

        assert!(!runtime.is_current(&capture));
        assert_eq!(*stops.lock().unwrap(), vec![(capture.clone(), reason)]);
        assert_eq!(
            runtime.evidence(&capture).unwrap().ended_at_ms,
            Some(1_000),
            "evidence ends at the helper's own end"
        );
        let events = recorded.0.lock().unwrap().clone();
        assert!(matches!(
            events.last(),
            Some((MeetingLifecycleEvent::Ended { reason: ended, .. }, false)) if *ended == reason
        ));
    }
}

#[tokio::test]
async fn a_mic_processing_change_updates_the_capture_provenance() {
    let (_dir, _audio, router) = router(FakeEngine {
        events: vec![MeetingCaptureEvent::MicProcessingChanged {
            at: CaptureInstant(4_800),
            mic: MicProcessing::Raw,
        }],
        ..Default::default()
    });
    let runtime = router.runtime().clone();
    runtime.begin("acct", "s", LifecycleOwner::App).unwrap();
    router.open_capture(config(None)).unwrap();
    tokio::task::yield_now().await;
    assert_eq!(
        runtime.active().unwrap().provenance.unwrap().mic,
        MicProcessing::Raw
    );
}

#[tokio::test]
async fn signing_out_stops_the_helper_and_refuses_late_audio_opens() {
    let (_dir, audio, router) = router(FakeEngine::default());
    let runtime = router.runtime().clone();
    runtime.set_account(Some("acct".into()));
    let capture = runtime.begin("acct", "s", LifecycleOwner::App).unwrap();
    router.open_capture(config(None)).unwrap();

    assert_eq!(router.set_account(None), Some(capture.clone()));
    tokio::task::yield_now().await;
    let control = audio.engine.controls.lock().unwrap()[0].clone();
    assert_eq!(control.stopped.load(Ordering::SeqCst), 1);
    assert!(!router.can_pause(&capture));
    // A source task can reach the audio router after the capture ended, even if the root
    // admitted its start earlier. It must not reopen as whole-system legacy capture.
    for opened in [
        router.open_capture(config(None)),
        router.open_speaker_capture(16_000, 320),
        router.open_mic_capture(None, 16_000, 320),
    ] {
        assert!(matches!(opened, Err(Error::MeetingCaptureFailed(_))));
    }
}

#[tokio::test]
async fn a_reopened_helper_reports_on_the_captures_timeline_not_its_own() {
    let (_dir, _audio, router) = router(FakeEngine::default());
    let runtime = router.runtime().clone();
    let capture = runtime.begin("acct", "s", LifecycleOwner::App).unwrap();
    router.open_capture(config(None)).unwrap();
    tokio::task::yield_now().await;
    tokio::time::sleep(std::time::Duration::from_millis(60)).await;

    // Both helpers report an observation at their own frame 0.
    router.open_capture(config(None)).unwrap();
    tokio::task::yield_now().await;
    let observations = runtime.observations(&capture);
    assert_eq!(observations.len(), 2);
    assert_eq!(observations[0].at, CaptureInstant(0));
    assert!(
        observations[1].at >= CaptureInstant::from_millis(60.0).unwrap(),
        "{:?}",
        observations[1].at
    );
}
