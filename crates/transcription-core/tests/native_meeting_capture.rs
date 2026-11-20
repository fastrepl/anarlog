//! A synthetic recording through the native meeting engine path: the router picks the engine,
//! the existing listener records and transcribes its frames, and the runtime scopes the call,
//! evidence and helper staging to that one capture.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anlg_audio::meeting::*;
use anlg_audio::{AudioProvider, CaptureConfig, CaptureEvent, CaptureFrame, CaptureStream, Error};
use futures_util::{FutureExt, SinkExt, StreamExt, stream};
use ractor::Actor;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use transcription_core::listener::actors::{RootActor, RootArgs, RootMsg, SessionParams};
use transcription_core::listener::{
    ListenerRuntime, SessionDataEvent, SessionErrorEvent, SessionLifecycleEvent,
    SessionProgressEvent, TranscriptionMode,
};
use transcription_core::meeting::{MeetingAudioRouter, MeetingRuntime};

const MEET: &str = "https://meet.google.com/abc-defg-hij";

fn meet_call() -> CaptureScope {
    CaptureScope::Call(ConfirmedCall {
        pid: 4242,
        bundle_id: "com.google.Chrome".into(),
        platform: CallPlatform::GoogleMeet,
        call_url: Some(MEET.into()),
    })
}

struct Control {
    stopped: AtomicUsize,
    released: AtomicUsize,
}

impl MeetingCaptureControl for Control {
    fn pause(&self) -> futures_util::future::BoxFuture<'static, Result<(), Error>> {
        async { Ok(()) }.boxed()
    }
    fn resume(&self) -> futures_util::future::BoxFuture<'static, Result<(), Error>> {
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

/// Stands in for the helper adapter: 16 kHz frames on the existing stream contract, plus one
/// accessibility observation for the capture it was opened for.
#[derive(Default)]
struct SyntheticEngine {
    requests: Mutex<Vec<ScopeRequest>>,
    controls: Mutex<Vec<Arc<Control>>>,
}

impl MeetingCaptureEngine for SyntheticEngine {
    fn availability(&self) -> EngineAvailability {
        EngineAvailability::Available {
            selects_microphone: false,
        }
    }

    fn calls_in_progress(&self) -> Result<Vec<CallApp>, Error> {
        Ok(vec![])
    }

    fn open(&self, request: MeetingCaptureRequest) -> Result<MeetingCapture, Error> {
        assert!(request.staging_dir.is_dir());
        std::fs::write(request.staging_dir.join("microphone.caf"), b"raw").unwrap();
        self.requests.lock().unwrap().push(request.scope.clone());
        let control = Arc::new(Control {
            stopped: AtomicUsize::new(0),
            released: AtomicUsize::new(0),
        });
        self.controls.lock().unwrap().push(control.clone());
        let chunk = request.config.chunk_size;
        let period = Duration::from_secs_f64(chunk as f64 / request.config.sample_rate as f64);
        let frames = stream::unfold(0_u64, move |index| async move {
            tokio::time::sleep(period).await;
            let tone: Vec<f32> = (0..chunk)
                .map(|i| ((index as usize * chunk + i) as f32 * 0.05).sin() * 0.2)
                .collect();
            let frame = CaptureFrame {
                raw_mic: Arc::from(tone.clone()),
                raw_speaker: Arc::from(tone),
                aec_mic: None,
            };
            Some((Ok(CaptureEvent::Frame(frame)), index + 1))
        });
        let events = stream::iter([MeetingCaptureEvent::Observation(SpeakerObservation {
            capture: request.capture.clone(),
            source: ObservationSource::Accessibility,
            platform: CallPlatform::GoogleMeet,
            at: CaptureInstant::from_millis(250.0).unwrap(),
            participants: vec![ObservedParticipant {
                participant_id: "p1".into(),
                display_name: Some("Ada".into()),
                is_self: Some(false),
            }],
            roster_complete: false,
            participant_count: Some(2),
            speaking: Some(vec![SpeakingCue {
                participant_id: Some("p1".into()),
                display_name: Some("Ada".into()),
            }]),
        })]);
        Ok(MeetingCapture {
            stream: CaptureStream::with_events(frames),
            scope: match request.scope {
                ScopeRequest::Exactly(scope) => scope,
                ScopeRequest::Discover => meet_call(),
            },
            provenance: CaptureProvenance {
                engine: CaptureEngine::NativeMeeting,
                source_sample_rate: NATIVE_SAMPLE_RATE,
                mic: MicProcessing::VoiceProcessed,
            },
            events: Box::pin(events),
            control,
        })
    }
}

struct SyntheticAudio(Arc<SyntheticEngine>);

impl AudioProvider for SyntheticAudio {
    fn open_capture(&self, _: CaptureConfig) -> Result<CaptureStream, Error> {
        panic!("the legacy engine must not be opened for an opted-in capture")
    }
    fn open_speaker_capture(&self, _: u32, _: usize) -> Result<CaptureStream, Error> {
        panic!("unexpected speaker-only capture")
    }
    fn open_mic_capture(
        &self,
        _: Option<String>,
        _: u32,
        _: usize,
    ) -> Result<CaptureStream, Error> {
        panic!("unexpected microphone-only capture")
    }
    fn default_device_name(&self) -> String {
        "Built-in".into()
    }
    fn list_mic_devices(&self) -> Vec<String> {
        vec!["Built-in".into()]
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
        Some(self.0.clone())
    }
}

/// Mirrors the Tauri runtime's lifecycle hook: a stopped recording ends its capture.
struct Runtime {
    vault: PathBuf,
    errors: mpsc::UnboundedSender<String>,
    router: Arc<MeetingAudioRouter>,
    data: mpsc::UnboundedSender<SessionDataEvent>,
    lifecycle: mpsc::UnboundedSender<SessionLifecycleEvent>,
}

impl anlg_storage::StorageRuntime for Runtime {
    fn global_base(&self) -> Result<PathBuf, anlg_storage::Error> {
        Ok(self.vault.clone())
    }
    fn vault_base(&self) -> Result<PathBuf, anlg_storage::Error> {
        Ok(self.vault.clone())
    }
}

impl ListenerRuntime for Runtime {
    fn emit_lifecycle(&self, event: SessionLifecycleEvent) {
        if let SessionLifecycleEvent::Finalizing { session_id }
        | SessionLifecycleEvent::Inactive { session_id, .. } = &event
            && let Some(capture) = self.router.runtime().active_for_session(session_id)
        {
            self.router
                .finish_capture(&capture, CaptureEndReason::UserStopped);
        }
        let _ = self.lifecycle.send(event);
    }
    fn emit_progress(&self, _: SessionProgressEvent) {}
    fn emit_error(&self, event: SessionErrorEvent) {
        if let SessionErrorEvent::AudioError { error, .. } = event {
            let _ = self.errors.send(error);
        }
    }
    fn emit_data(&self, event: SessionDataEvent) {
        let _ = self.data.send(event);
    }
}

fn deepgram_result(word: &str) -> Message {
    let raw = serde_json::json!({
        "type": "Results", "start": 0.0, "duration": 1.0,
        "is_final": true, "speech_final": true, "from_finalize": false,
        "channel_index": [0, 2],
        "channel": {"alternatives": [{"transcript": format!("{word} speech"), "confidence": 1.0,
            "words": [
                {"word": word, "start": 0.0, "end": 0.5, "confidence": 1.0, "speaker": 0},
                {"word": "speech", "start": 0.5, "end": 1.0, "confidence": 1.0, "speaker": 0}
            ]}]},
        "metadata": {"request_id": "test", "model_uuid": "test", "model_info": {"name": "test", "version": "1", "arch": "test"}}
    });
    Message::Text(raw.to_string().into())
}

async fn fake_stt() -> std::net::SocketAddr {
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = socket.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((stream, _)) = socket.accept().await {
            tokio::spawn(async move {
                let Ok(mut ws) = accept_async(stream).await else {
                    return;
                };
                // Answer once a full second of dual-channel 16 kHz audio has arrived, so the word's
                // time lies inside the audio the listener sent.
                let mut received = 0;
                let mut answered = false;
                while let Some(Ok(message)) = ws.next().await {
                    match message {
                        Message::Binary(audio) if !answered => {
                            received += audio.len();
                            if received >= 16_000 * 2 * 2 {
                                answered = true;
                                let _ = ws.send(deepgram_result("native")).await;
                            }
                        }
                        Message::Text(text) if text.contains("Finalize") => {
                            let _ = ws.close(None).await;
                            return;
                        }
                        _ => {}
                    }
                }
            });
        }
    });
    address
}

fn params(session_id: &str, address: std::net::SocketAddr) -> SessionParams {
    SessionParams {
        session_id: session_id.to_string(),
        retain_audio: Some(true),
        languages: vec![],
        onboarding: false,
        transcription_mode: TranscriptionMode::Live,
        model: "cloud".into(),
        base_url: format!("http://{address}/stt"),
        api_key: "test".into(),
        keywords: vec![],
        mic_device: None,
        participant_human_ids: vec![],
        self_human_id: None,
        speaker_assignments: vec![],
    }
}

async fn record(
    root: &ractor::ActorRef<RootMsg>,
    vault: &std::path::Path,
    session_id: &str,
    address: std::net::SocketAddr,
    data: &mut mpsc::UnboundedReceiver<SessionDataEvent>,
    lifecycle: &mut mpsc::UnboundedReceiver<SessionLifecycleEvent>,
) {
    std::fs::create_dir_all(vault.join("sessions").join(session_id)).unwrap();
    ractor::call!(root, RootMsg::StartSession, params(session_id, address))
        .unwrap()
        .unwrap();
    timeout(Duration::from_secs(15), async {
        while let Some(event) = data.recv().await {
            if let SessionDataEvent::TranscriptDelta { delta, .. } = event
                && delta
                    .new_words
                    .iter()
                    .any(|word| word.text.trim() == "native")
            {
                return;
            }
        }
    })
    .await
    .expect("the native engine's audio should reach the live transcript");
    ractor::call!(root, RootMsg::StopSession).unwrap();
    timeout(Duration::from_secs(15), async {
        while let Some(event) = lifecycle.recv().await {
            if matches!(event, SessionLifecycleEvent::Inactive { .. }) {
                return;
            }
        }
    })
    .await
    .expect("the recording should stop");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_synthetic_meeting_records_transcribes_and_scopes_its_call_per_capture() {
    let vault = tempfile::tempdir().unwrap();
    let engine = Arc::new(SyntheticEngine::default());
    let runtime = MeetingRuntime::new(vault.path().join("meeting-capture"));
    let router = MeetingAudioRouter::new(Arc::new(SyntheticAudio(engine.clone())), runtime.clone());
    let (data_tx, mut data) = mpsc::unbounded_channel();
    let (lifecycle_tx, mut lifecycle) = mpsc::unbounded_channel();
    let (errors_tx, mut errors) = mpsc::unbounded_channel();
    let listener = Arc::new(Runtime {
        vault: vault.path().to_path_buf(),
        errors: errors_tx,
        router: router.clone(),
        data: data_tx,
        lifecycle: lifecycle_tx,
    });
    let (root, root_task) = Actor::spawn(
        None,
        RootActor,
        RootArgs {
            runtime: listener,
            audio: router.clone(),
        },
    )
    .await
    .unwrap();
    // The root's startup cleanup must finish before a recording starts, as it does at app launch.
    timeout(Duration::from_secs(10), async {
        while let Some(error) = errors.recv().await {
            if error == "audio_deletion_completed" {
                return;
            }
        }
    })
    .await
    .expect("startup cleanup should finish");
    let address = fake_stt().await;
    let session_id = uuid::Uuid::new_v4().to_string();

    let first = runtime
        .begin("acct", &session_id, LifecycleOwner::App)
        .unwrap();
    record(
        &root,
        vault.path(),
        &session_id,
        address,
        &mut data,
        &mut lifecycle,
    )
    .await;

    assert_eq!(router.engine_of(&first), Some(CaptureEngine::NativeMeeting));
    assert!(
        !runtime.is_current(&first),
        "a stopped capture cannot be acted for"
    );
    let evidence = runtime.observations(&first);
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].capture, first);
    let audio = vault
        .path()
        .join("sessions")
        .join(&session_id)
        .join("audio.mp3");
    assert!(
        audio.metadata().unwrap().len() > 0,
        "the recorder kept the audio"
    );
    let staging = runtime.staging_root().join(&first.capture_id);
    assert!(staging.is_dir(), "raw staging waits for adoption");
    let first_control = engine.controls.lock().unwrap()[0].clone();
    assert_eq!(first_control.stopped.load(Ordering::SeqCst), 1);

    // Recording the same session again starts a new capture that rediscovers its call.
    let second = runtime
        .begin("acct", &session_id, LifecycleOwner::App)
        .unwrap();
    assert_ne!(second.capture_id, first.capture_id);
    assert!(runtime.observations(&second).is_empty());
    record(
        &root,
        vault.path(),
        &session_id,
        address,
        &mut data,
        &mut lifecycle,
    )
    .await;
    assert_eq!(
        engine.requests.lock().unwrap().as_slice(),
        [ScopeRequest::Discover, ScopeRequest::Discover]
    );

    assert_eq!(router.adopt(&session_id), 2);
    assert_eq!(first_control.released.load(Ordering::SeqCst), 1);
    assert!(!staging.exists());
    assert!(audio.metadata().unwrap().len() > 0);

    root.stop(None);
    root_task.await.unwrap();
}
