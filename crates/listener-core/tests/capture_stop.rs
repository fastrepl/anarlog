//! A recording started for one capture stops only as that capture: a stop delayed past the
//! session's next recording leaves that recording running, a stop that beat its start leaves
//! nothing recording, and every lifecycle event names the capture it belongs to.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use anlg_audio::{AudioProvider, CaptureConfig, CaptureStream, Error};
use listener_core::actors::{RootActor, RootArgs, RootMsg, SessionParams};
use listener_core::{
    ListenerRuntime, SessionDataEvent, SessionErrorEvent, SessionLifecycleEvent,
    SessionProgressEvent, StartSessionError, State, TranscriptionMode,
};
use ractor::{Actor, ActorRef};

struct Runtime {
    vault: PathBuf,
    events: Mutex<Vec<(&'static str, Option<String>)>>,
    /// Captures the meeting runtime already ended.
    ended: Mutex<HashSet<String>>,
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
    fn capture_is_current(&self, capture_id: &str) -> bool {
        !self.ended.lock().unwrap().contains(capture_id)
    }
    fn emit_lifecycle(&self, _: SessionLifecycleEvent) {}
    fn emit_capture_lifecycle(&self, event: SessionLifecycleEvent, capture_id: Option<&str>) {
        let kind = match event {
            SessionLifecycleEvent::Active { .. } => "active",
            SessionLifecycleEvent::Finalizing { .. } => "finalizing",
            SessionLifecycleEvent::Inactive { .. } => "inactive",
        };
        self.events
            .lock()
            .unwrap()
            .push((kind, capture_id.map(str::to_string)));
    }
    fn emit_progress(&self, _: SessionProgressEvent) {}
    fn emit_error(&self, _: SessionErrorEvent) {}
    fn emit_data(&self, _: SessionDataEvent) {}
}

/// Audio that never delivers a frame: a recording stays active until it is stopped.
impl AudioProvider for Runtime {
    fn open_capture(&self, _: CaptureConfig) -> Result<CaptureStream, Error> {
        Ok(CaptureStream::new(futures_util::stream::pending()))
    }
    fn open_speaker_capture(&self, _: u32, _: usize) -> Result<CaptureStream, Error> {
        unreachable!()
    }
    fn open_mic_capture(
        &self,
        _: Option<String>,
        _: u32,
        _: usize,
    ) -> Result<CaptureStream, Error> {
        unreachable!()
    }
    fn default_device_name(&self) -> String {
        "test".into()
    }
    fn list_mic_devices(&self) -> Vec<String> {
        vec![]
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
}

fn recording(session_id: &str) -> SessionParams {
    SessionParams {
        session_id: session_id.to_string(),
        retain_audio: None,
        languages: vec![],
        onboarding: false,
        transcription_mode: TranscriptionMode::Batch,
        model: String::new(),
        base_url: String::new(),
        api_key: String::new(),
        keywords: vec![],
        mic_device: None,
        participant_human_ids: vec![],
        self_human_id: None,
        speaker_assignments: vec![],
        capture_started_at_ms: None,
    }
}

async fn wait_until_inactive(root: &ActorRef<RootMsg>) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while ractor::call!(root, RootMsg::GetSnapshot).unwrap().state != State::Inactive {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the stopped recording should finish");
}

#[tokio::test]
async fn a_late_stop_of_an_earlier_capture_does_not_end_the_sessions_next_recording() {
    let vault = tempfile::tempdir().unwrap();
    let runtime = std::sync::Arc::new(Runtime {
        vault: vault.path().to_path_buf(),
        events: Default::default(),
        ended: Default::default(),
    });
    let (root, task) = Actor::spawn(
        None,
        RootActor,
        RootArgs {
            runtime: runtime.clone(),
            audio: runtime.clone(),
        },
    )
    .await
    .unwrap();
    let session_id = uuid::Uuid::new_v4().to_string();

    let start = |capture: &str| (recording(&session_id), capture.to_string());

    let (params, capture) = start("first");
    ractor::call!(root, RootMsg::StartCapture, params, capture)
        .unwrap()
        .unwrap();
    // Sign-out ends the first capture; its stop is delayed while the person stops and records
    // the same session again.
    let late_stop = root.call(
        |reply| RootMsg::StopCaptureIfActive("first".to_string(), reply),
        None,
    );
    ractor::call!(root, RootMsg::StopSession).unwrap();
    wait_until_inactive(&root).await;
    let (params, capture) = start("second");
    ractor::call!(root, RootMsg::StartCapture, params, capture)
        .unwrap()
        .unwrap();

    assert!(matches!(
        late_stop.await,
        Ok(ractor::rpc::CallResult::Success(false))
    ));
    let snapshot = ractor::call!(root, RootMsg::GetSnapshot).unwrap();
    assert_eq!(snapshot.active_session_id, Some(session_id.clone()));
    assert!(ractor::call!(root, RootMsg::StopCaptureIfActive, "second".to_string()).unwrap());
    wait_until_inactive(&root).await;
    // The first capture's late Inactive names it, so it cannot retire the second capture.
    let captured = |kind, capture: &str| (kind, Some(capture.to_string()));
    assert_eq!(
        *runtime.events.lock().unwrap(),
        [
            captured("active", "first"),
            captured("finalizing", "first"),
            captured("inactive", "first"),
            captured("active", "second"),
            captured("finalizing", "second"),
            captured("inactive", "second"),
        ]
    );
    root.stop(None);
    task.await.unwrap();
}

/// Sign-out, or a browser start being cancelled, stops a capture the record path has begun but
/// whose start hasn't reached the recorder yet: the recorder isn't recording it, so the stop
/// ends it in the meeting runtime instead. Its start then arrives and must not record.
#[tokio::test]
async fn a_capture_stopped_before_its_start_arrives_never_records() {
    let vault = tempfile::tempdir().unwrap();
    let runtime = std::sync::Arc::new(Runtime {
        vault: vault.path().to_path_buf(),
        events: Default::default(),
        ended: Default::default(),
    });
    let (root, task) = Actor::spawn(
        None,
        RootActor,
        RootArgs {
            runtime: runtime.clone(),
            audio: runtime.clone(),
        },
    )
    .await
    .unwrap();
    let session_id = uuid::Uuid::new_v4().to_string();

    assert!(!ractor::call!(root, RootMsg::StopCaptureIfActive, "first".to_string()).unwrap());
    runtime.ended.lock().unwrap().insert("first".to_string());
    assert_eq!(
        ractor::call!(
            root,
            RootMsg::StartCapture,
            recording(&session_id),
            "first".to_string()
        )
        .unwrap(),
        Err(StartSessionError::CaptureEnded)
    );
    let snapshot = ractor::call!(root, RootMsg::GetSnapshot).unwrap();
    assert_eq!(snapshot.state, State::Inactive);
    assert!(runtime.events.lock().unwrap().is_empty());

    // The person's next recording of the note starts as usual.
    ractor::call!(
        root,
        RootMsg::StartCapture,
        recording(&session_id),
        "second".to_string()
    )
    .unwrap()
    .unwrap();
    assert!(ractor::call!(root, RootMsg::StopCaptureIfActive, "second".to_string()).unwrap());
    wait_until_inactive(&root).await;
    root.stop(None);
    task.await.unwrap();
}
