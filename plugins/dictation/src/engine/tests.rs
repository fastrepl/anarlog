use std::sync::{Arc, Mutex};
use std::time::Duration;

use anlg_audio::{AudioProvider, CaptureConfig, CaptureFrame, CaptureStream};
use futures_util::{SinkExt, StreamExt, stream};
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{
    Alternatives, Channel as TranscriptChannel, Metadata, StreamResponse,
};
use tauri::ipc::{Channel, InvokeResponseBody};
use tokio_tungstenite::tungstenite::Message;

use super::*;
use crate::ducking::NoDucker;

const FRAME: usize = 1_600;
const A: &str = "00000000-0000-4000-8000-00000000000a";
const B: &str = "00000000-0000-4000-8000-00000000000b";
const C: &str = "00000000-0000-4000-8000-00000000000c";

/// Plays `frames` (one level per 100 ms frame) as fast as the engine reads them, then stays open
/// and silent until the dictation stops, like a microphone nobody speaks into.
struct ScriptedAudio(Vec<f32>);

impl AudioProvider for ScriptedAudio {
    fn open_capture(&self, _config: CaptureConfig) -> Result<CaptureStream, anlg_audio::Error> {
        unreachable!()
    }

    fn open_speaker_capture(&self, _: u32, _: usize) -> Result<CaptureStream, anlg_audio::Error> {
        unreachable!()
    }

    fn open_mic_capture(
        &self,
        _device: Option<String>,
        _sample_rate: u32,
        _chunk_size: usize,
    ) -> Result<CaptureStream, anlg_audio::Error> {
        let frames: Vec<_> = self
            .0
            .iter()
            .map(|level| {
                Ok(CaptureFrame {
                    raw_mic: Arc::from(vec![*level; FRAME]),
                    raw_speaker: Arc::from([]),
                    aec_mic: None,
                })
            })
            .collect();
        Ok(CaptureStream::new(
            stream::iter(frames)
                .then(|frame| async move {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                    frame
                })
                .chain(stream::pending()),
        ))
    }

    fn default_device_name(&self) -> String {
        "test-mic".into()
    }

    fn list_mic_devices(&self) -> Vec<String> {
        vec![self.default_device_name()]
    }

    fn play_silence(&self) -> std::sync::mpsc::Sender<()> {
        std::sync::mpsc::channel().0
    }

    fn play_bytes(&self, _bytes: &'static [u8]) -> std::sync::mpsc::Sender<()> {
        std::sync::mpsc::channel().0
    }

    fn probe_mic(&self, _device: Option<String>) -> Result<(), anlg_audio::Error> {
        Ok(())
    }

    fn probe_speaker(&self) -> Result<(), anlg_audio::Error> {
        Ok(())
    }
}

fn speech(voiced: usize, silent: usize, voiced_again: usize) -> Arc<dyn AudioProvider> {
    let mut levels = vec![0.1; voiced];
    levels.extend(vec![0.0; silent]);
    levels.extend(vec![0.1; voiced_again]);
    Arc::new(ScriptedAudio(levels))
}

#[derive(Clone, Default)]
struct Updates(Arc<Mutex<Vec<serde_json::Value>>>);

impl Updates {
    fn channel(&self) -> Channel<DictationUpdate> {
        let received = self.0.clone();
        Channel::new(move |body| {
            if let InvokeResponseBody::Json(json) = body {
                received
                    .lock()
                    .unwrap()
                    .push(serde_json::from_str(&json).unwrap());
            }
            Ok(())
        })
    }

    fn count(&self, kind: &str) -> usize {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|update| update["type"] == kind)
            .count()
    }

    async fn wait_for(&self, kind: &str, count: usize) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while self.count(kind) < count {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("no {count} {kind} updates"));
    }
}

fn engine(recovery: Option<PathBuf>) -> Engine {
    Engine::new(recovery, Arc::new(NoDucker), Limits::default())
}

fn policy(provider: &str, local_only: bool) -> DictationPolicy {
    DictationPolicy {
        provider: provider.into(),
        model: None,
        local_only,
    }
}

fn stream_config(provider: &str, base_url: String) -> PreviewConfig {
    PreviewConfig {
        provider: provider.into(),
        base_url,
        api_key: "test".into(),
        params: ListenParams::default(),
    }
}

fn request(id: &str, surface: DictationSurface, policy: DictationPolicy) -> BeginDictation {
    BeginDictation {
        operation_id: id.into(),
        surface,
        account_id: "account-a".into(),
        microphone_device: None,
        policy,
        stream: None,
        recovery: false,
        ducking: false,
    }
}

fn final_words(start: f64, duration: f64, text: &str, from_finalize: bool) -> String {
    serde_json::to_string(&StreamResponse::TranscriptResponse {
        start,
        duration,
        is_final: true,
        speech_final: true,
        from_finalize,
        channel: TranscriptChannel {
            alternatives: vec![Alternatives {
                transcript: text.into(),
                words: vec![],
                confidence: 1.0,
                languages: vec![],
            }],
        },
        metadata: Metadata::default(),
        channel_index: vec![0, 1],
    })
    .unwrap()
}

/// A provider that commits "Hello there." for the first second of audio, then drops the
/// connection before it has finalized the rest.
async fn failing_provider() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(socket).await.unwrap();
        while !matches!(ws.next().await, Some(Ok(Message::Binary(_)))) {}
        ws.send(Message::Text(
            final_words(0.0, 1.0, "Hello there.", false).into(),
        ))
        .await
        .unwrap();
        drop(ws);
    });
    format!("http://{address}")
}

/// A provider that finalizes everything when asked to and reports every audio byte it got.
async fn finalizing_provider(received: Arc<Mutex<Vec<u8>>>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(socket).await.unwrap();
        while let Some(Ok(message)) = ws.next().await {
            match message {
                Message::Binary(bytes) => received.lock().unwrap().extend_from_slice(&bytes),
                Message::Text(text) if text.contains("Finalize") => {
                    let reply = final_words(0.0, 3.5, "All of it, finalized.", true);
                    ws.send(Message::Text(reply.into())).await.unwrap();
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });
    format!("http://{address}")
}

fn wav_samples(path: &str) -> Vec<i16> {
    hound::WavReader::open(path)
        .unwrap()
        .into_samples()
        .map(Result::unwrap)
        .collect()
}

#[tokio::test]
async fn a_stream_that_drops_after_committed_words_retries_only_the_unheard_suffix() {
    let engine = engine(None);
    let updates = Updates::default();
    let mut begin = request(A, DictationSurface::System, policy("deepgram", false));
    begin.stream = Some(stream_config("deepgram", failing_provider().await));
    engine
        .begin(begin, speech(15, 5, 15), updates.channel())
        .unwrap();
    updates.wait_for("amplitude", 35).await;
    updates.wait_for("previewUnavailable", 1).await;

    let finish = engine.finish(A).await.unwrap();
    let DictationFinish::Suffix {
        committed_text,
        audio_path,
        duration_ms,
    } = &finish
    else {
        panic!("expected a suffix retry, got {finish:?}");
    };
    assert_eq!(committed_text, "Hello there.");
    assert_eq!(*duration_ms, 3_500);
    // The second after the committed boundary in the first phrase, then the whole second phrase.
    assert_eq!(wav_samples(audio_path).len(), 10 * FRAME + 15 * FRAME);

    assert_eq!(
        engine.complete(A, None).unwrap_err().code,
        DictationErrorCode::Invalid
    );
    assert_eq!(
        engine.complete(A, Some("how are you?".into())).unwrap(),
        "Hello there. how are you?"
    );
    assert_eq!(
        engine
            .complete(A, Some("how are you?".into()))
            .unwrap_err()
            .code,
        DictationErrorCode::Stale
    );
    let directory = PathBuf::from(audio_path).parent().unwrap().to_path_buf();
    engine.release(A, false).unwrap();
    assert!(!directory.exists());
}

#[tokio::test]
async fn without_proven_timing_a_failed_stream_is_replaced_by_the_full_retry() {
    let engine = engine(None);
    let updates = Updates::default();
    let mut begin = request(A, DictationSurface::Chat, policy("anarlog", false));
    begin.stream = Some(stream_config("anarlog", failing_provider().await));
    engine
        .begin(begin, speech(15, 5, 15), updates.channel())
        .unwrap();
    updates.wait_for("amplitude", 35).await;
    updates.wait_for("previewUnavailable", 1).await;

    let finish = engine.finish(A).await.unwrap();
    let DictationFinish::Full { audio_path, .. } = &finish else {
        panic!("expected a full retry, got {finish:?}");
    };
    assert_eq!(wav_samples(audio_path).len(), 35 * FRAME);
    assert_eq!(
        engine
            .complete(A, Some("Hello there, how are you?".into()))
            .unwrap(),
        "Hello there, how are you?"
    );
    engine.release(A, false).unwrap();
}

#[tokio::test]
async fn audio_heard_before_the_stream_attaches_is_sent_first_and_finalized() {
    let engine = engine(None);
    let updates = Updates::default();
    let received = Arc::new(Mutex::new(Vec::new()));
    let levels: Vec<f32> = (1..=35).map(|frame| frame as f32 / 100.0).collect();
    engine
        .begin(
            request(A, DictationSurface::System, policy("deepgram", false)),
            Arc::new(ScriptedAudio(levels.clone())),
            updates.channel(),
        )
        .unwrap();
    updates.wait_for("ready", 1).await;
    updates.wait_for("amplitude", 10).await;
    engine
        .attach_stream(
            A,
            stream_config("deepgram", finalizing_provider(received.clone()).await),
        )
        .unwrap();
    assert_eq!(
        engine
            .attach_stream(A, stream_config("deepgram", "http://127.0.0.1:9".into()))
            .unwrap_err()
            .code,
        DictationErrorCode::Invalid
    );
    updates.wait_for("amplitude", 35).await;

    let finish = engine.finish(A).await.unwrap();
    assert_eq!(
        finish,
        DictationFinish::Final {
            text: "All of it, finalized.".into(),
            duration_ms: 3_500
        }
    );
    let expected: Vec<u8> = levels
        .iter()
        .flat_map(|level| wav::to_pcm16(&[*level; FRAME]))
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    assert_eq!(*received.lock().unwrap(), expected);
    assert_eq!(engine.complete(A, None).unwrap(), "All of it, finalized.");
    engine.release(A, false).unwrap();
}

#[tokio::test]
async fn a_full_stream_queue_keeps_the_unsent_audio_for_retry() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let mut live = super::stream::LiveStream::start(
        stream_config("anarlog", finalizing_provider(received.clone()).await),
        1,
        Updates::default().channel(),
    );
    let frame = vec![0.1; FRAME];
    let pcm = wav::to_pcm16(&frame);
    assert!(live.send(&pcm));
    // No yield between sends: the single available slot still holds the first frame.
    assert!(!live.send(&pcm));
    let report = live.finish(Duration::from_secs(5)).await;
    assert_eq!(received.lock().unwrap().len(), pcm.len() * 2);

    let dir = tempfile::tempdir().unwrap();
    let recorded: Vec<i16> = pcm.iter().chain(&pcm).copied().collect();
    wav::write_all(&dir.path().join(AUDIO), &recorded).unwrap();
    let mut phrases = phrases::Phrases::default();
    phrases.push(&frame, &pcm);
    phrases.push(&frame, &pcm);
    phrases.flush();
    let finish = outcome(
        capture::Captured {
            samples: recorded.len() as u64,
            phrases,
            stream: Some(report),
        },
        dir.path(),
    )
    .unwrap();
    let DictationFinish::Full { audio_path, .. } = finish else {
        panic!("unsent audio must be retried, got {finish:?}");
    };
    assert_eq!(wav_samples(&audio_path), recorded);
}

#[tokio::test]
async fn a_short_tap_uploads_nothing_and_leaves_no_audio() {
    let engine = engine(None);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let updates = Updates::default();
    engine
        .begin(
            request(A, DictationSurface::System, policy("deepgram", false)),
            speech(3, 0, 0),
            updates.channel(),
        )
        .unwrap();
    updates.wait_for("amplitude", 3).await;
    engine.cancel(A).await;

    assert!(
        tokio::time::timeout(Duration::from_millis(100), listener.accept())
            .await
            .is_err()
    );
    assert_eq!(
        engine.finish(A).await.unwrap_err().code,
        DictationErrorCode::Stale
    );
    assert_eq!(
        engine
            .attach_stream(A, stream_config("deepgram", "http://127.0.0.1:9".into()))
            .unwrap_err()
            .code,
        DictationErrorCode::Stale
    );
}

#[tokio::test]
async fn a_cancel_that_arrives_before_the_start_wins() {
    let engine = engine(None);
    engine.cancel(A).await;
    let error = engine
        .begin(
            request(A, DictationSurface::Chat, policy("deepgram", false)),
            speech(5, 0, 0),
            Updates::default().channel(),
        )
        .unwrap_err();
    assert_eq!(error.code, DictationErrorCode::Cancelled);
}

#[tokio::test]
async fn one_microphone_is_shared_and_a_new_press_supersedes_its_surface() {
    let engine = engine(None);
    let updates = Updates::default();
    engine
        .begin(
            request(A, DictationSurface::System, policy("deepgram", false)),
            speech(12, 0, 0),
            updates.channel(),
        )
        .unwrap();
    let busy = engine
        .begin(
            request(B, DictationSurface::Chat, policy("deepgram", false)),
            speech(5, 0, 0),
            Updates::default().channel(),
        )
        .unwrap_err();
    assert_eq!(busy.code, DictationErrorCode::Busy);

    updates.wait_for("amplitude", 12).await;
    let DictationFinish::Full { audio_path, .. } = engine.finish(A).await.unwrap() else {
        panic!("expected a full batch");
    };
    // The microphone is free while the first dictation is still being transcribed.
    engine
        .begin(
            request(B, DictationSurface::Chat, policy("deepgram", false)),
            speech(5, 0, 0),
            Updates::default().channel(),
        )
        .unwrap();
    engine.cancel(B).await;
    engine
        .begin(
            request(C, DictationSurface::System, policy("deepgram", false)),
            speech(5, 0, 0),
            Updates::default().channel(),
        )
        .unwrap();
    assert_eq!(
        engine.complete(A, Some("late".into())).unwrap_err().code,
        DictationErrorCode::Stale
    );
    assert!(!PathBuf::from(audio_path).exists());
    engine.cancel(C).await;
}

#[tokio::test]
async fn silence_finishes_without_audio_to_transcribe() {
    let engine = engine(None);
    let updates = Updates::default();
    engine
        .begin(
            request(A, DictationSurface::Chat, policy("deepgram", false)),
            speech(0, 20, 0),
            updates.channel(),
        )
        .unwrap();
    updates.wait_for("amplitude", 20).await;
    assert_eq!(
        engine.finish(A).await.unwrap(),
        DictationFinish::Silent { duration_ms: 2_000 }
    );
    assert_eq!(engine.complete(A, None).unwrap(), "");
    // Its field takes a delivery only between completion and release.
    assert_eq!(engine.deliverable(A).as_deref(), Some("account-a"));
    engine.release(A, false).unwrap();
    assert_eq!(engine.deliverable(A), None);
}

#[tokio::test]
async fn the_duration_cap_ends_capture_and_keeps_the_audio() {
    let engine = Engine::new(
        None,
        Arc::new(NoDucker),
        Limits {
            max_samples: 10 * FRAME as u64,
            early_frames: EARLY_FRAMES,
        },
    );
    let updates = Updates::default();
    engine
        .begin(
            request(A, DictationSurface::System, policy("deepgram", false)),
            speech(30, 0, 0),
            updates.channel(),
        )
        .unwrap();
    updates.wait_for("captureEnded", 1).await;
    assert_eq!(updates.0.lock().unwrap().last().unwrap()["reason"], "limit");
    let DictationFinish::Full {
        audio_path,
        duration_ms,
    } = engine.finish(A).await.unwrap()
    else {
        panic!("expected the capped recording");
    };
    assert_eq!(duration_ms, 1_000);
    assert_eq!(wav_samples(&audio_path).len(), 10 * FRAME);
    engine.release(A, false).unwrap();
}

#[tokio::test]
async fn a_local_only_dictation_never_streams_off_the_machine() {
    let engine = engine(None);
    let mut remote = request(A, DictationSurface::Chat, policy("argmax", true));
    remote.stream = Some(stream_config("argmax", "https://api.example.com".into()));
    assert_eq!(
        engine
            .begin(remote, speech(5, 0, 0), Updates::default().channel())
            .unwrap_err()
            .code,
        DictationErrorCode::Policy
    );

    engine
        .begin(
            request(A, DictationSurface::Chat, policy("argmax", true)),
            speech(5, 0, 0),
            Updates::default().channel(),
        )
        .unwrap();
    for stream in [
        stream_config("deepgram", "http://127.0.0.1:9".into()),
        stream_config("argmax", "http://10.0.0.2:9".into()),
    ] {
        assert_eq!(
            engine.attach_stream(A, stream).unwrap_err().code,
            DictationErrorCode::Policy
        );
    }
    engine
        .attach_stream(A, stream_config("argmax", "http://localhost:9".into()))
        .unwrap();
    engine.cancel(A).await;
}

#[tokio::test]
async fn signing_out_retires_the_dictation_and_its_recovery() {
    let root = tempfile::tempdir().unwrap();
    let engine = engine(Some(root.path().join("recovery")));
    let updates = Updates::default();
    let mut begin = request(A, DictationSurface::System, policy("deepgram", false));
    begin.recovery = true;
    assert!(
        engine
            .begin(begin, speech(12, 0, 0), updates.channel())
            .unwrap()
            .recovery
    );
    updates.wait_for("amplitude", 12).await;
    assert!(root.path().join("recovery").join(A).exists());

    engine
        .retire(RetireReason::Account {
            account_id: "account-a".into(),
        })
        .await;
    assert_eq!(
        engine.finish(A).await.unwrap_err().code,
        DictationErrorCode::Stale
    );
    assert!(!root.path().join("recovery").join(A).exists());
    assert!(
        engine
            .recovery()
            .unwrap()
            .list("account-a", now_ms())
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn a_kept_local_recording_is_retried_locally_then_claimed_once() {
    let root = tempfile::tempdir().unwrap();
    let engine = engine(Some(root.path().join("recovery")));
    let updates = Updates::default();
    let mut begin = request(A, DictationSurface::Chat, policy("argmax", true));
    begin.recovery = true;
    engine
        .begin(begin, speech(12, 0, 0), updates.channel())
        .unwrap();
    updates.wait_for("amplitude", 12).await;
    let DictationFinish::Full { .. } = engine.finish(A).await.unwrap() else {
        panic!("expected a batch");
    };
    // The batch failed (offline): keep the audio.
    assert_eq!(engine.release(A, true).unwrap().as_deref(), Some(A));

    let store = engine.recovery().unwrap();
    let items = store.list("account-a", now_ms()).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].state,
        crate::recovery::DictationRecoveryState::Audio
    );
    assert_eq!(items[0].duration_ms, 1_200);
    assert_eq!(
        store
            .prepare_audio("account-a", A, &policy("deepgram", false), now_ms())
            .unwrap_err()
            .code,
        DictationErrorCode::Policy
    );
    assert_eq!(
        store
            .prepare_audio("account-b", A, &policy("argmax", true), now_ms())
            .unwrap_err()
            .code,
        DictationErrorCode::RecoveryNotFound
    );
    let path = store
        .prepare_audio("account-a", A, &policy("argmax", true), now_ms())
        .unwrap();
    assert_eq!(wav_samples(&path).len(), 12 * FRAME);

    store
        .save_text("account-a", A, "Recovered words.".into(), now_ms())
        .unwrap();
    assert!(!PathBuf::from(&path).exists());
    let items = store.list("account-a", now_ms()).unwrap();
    assert_eq!(items[0].text.as_deref(), Some("Recovered words."));
    assert_eq!(
        store.claim("account-a", A, now_ms()).unwrap(),
        "Recovered words."
    );
    assert_eq!(
        store.claim("account-a", A, now_ms()).unwrap_err().code,
        DictationErrorCode::RecoveryNotFound
    );
}

#[tokio::test]
async fn a_full_recovery_store_starts_without_recovery_instead_of_evicting() {
    let root = tempfile::tempdir().unwrap().keep();
    let recovery = root.join("recovery");
    let store = RecoveryStore::new(recovery.clone(), Limits::default().recording_bytes() * 2);
    for index in 0..crate::recovery::MAX_RECORDS {
        let id = format!("00000000-0000-4000-8000-{index:012}");
        store
            .reserve(
                &id,
                "account-a",
                DictationSurface::Chat,
                &policy("deepgram", false),
                now_ms(),
            )
            .unwrap()
            .unwrap();
        store.keep(&id, 1_000, Some("kept".into())).unwrap();
    }

    let engine = engine(Some(recovery));
    let mut begin = request(A, DictationSurface::Chat, policy("deepgram", false));
    begin.recovery = true;
    let ticket = engine
        .begin(begin, speech(5, 0, 0), Updates::default().channel())
        .unwrap();
    assert!(!ticket.recovery);
    engine.cancel(A).await;
    assert_eq!(
        engine
            .recovery()
            .unwrap()
            .list("account-a", now_ms())
            .unwrap()
            .len(),
        crate::recovery::MAX_RECORDS
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn an_interrupted_recording_becomes_recoverable_audio_and_expires_after_a_day() {
    let root = tempfile::tempdir().unwrap();
    let recovery = root.path().join("recovery");
    let created = now_ms();
    let store = RecoveryStore::new(recovery.clone(), 1_024);
    let directory = store
        .reserve(
            A,
            "account-a",
            DictationSurface::System,
            &policy("deepgram", false),
            created,
        )
        .unwrap()
        .unwrap();
    let mut writer = wav::WavWriter::create(&directory.join(AUDIO)).unwrap();
    writer.write(&vec![100; 8 * FRAME]).unwrap();
    // The process dies here: no finish, no keep.
    drop((writer, store));

    let store = RecoveryStore::new(recovery.clone(), 1_024);
    let items = store.list("account-a", created).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].duration_ms, 800);
    assert_eq!(items[0].expires_at, created + crate::recovery::EXPIRY_MS);
    // Another account never sees it, and listing as them deletes it.
    let other = RecoveryStore::new(recovery.clone(), 1_024);
    assert!(other.list("account-b", created).unwrap().is_empty());
    assert!(store.list("account-a", created).unwrap().is_empty());

    let directory = store
        .reserve(
            B,
            "account-a",
            DictationSurface::System,
            &policy("deepgram", false),
            created,
        )
        .unwrap()
        .unwrap();
    wav::write_all(&directory.join(AUDIO), &[1; FRAME]).unwrap();
    store.keep(B, 100, None).unwrap();
    assert_eq!(
        store
            .list("account-a", created + crate::recovery::EXPIRY_MS - 1)
            .unwrap()
            .len(),
        1
    );
    assert!(
        store
            .list("account-a", created + crate::recovery::EXPIRY_MS)
            .unwrap()
            .is_empty()
    );
    assert!(!directory.exists());
}

#[test]
fn startup_deletes_expired_audio_without_creating_a_store() {
    let root = tempfile::tempdir().unwrap();
    let recovery = root.path().join("recovery");
    RecoveryStore::new(recovery.clone(), 1_024).prune(now_ms());
    assert!(!recovery.exists());

    let created = now_ms();
    let store = RecoveryStore::new(recovery.clone(), 1_024);
    let directory = store
        .reserve(
            A,
            "account-a",
            DictationSurface::Chat,
            &policy("deepgram", false),
            created,
        )
        .unwrap()
        .unwrap();
    wav::write_all(&directory.join(AUDIO), &[1; FRAME]).unwrap();
    store.keep(A, 100, None).unwrap();

    RecoveryStore::new(recovery, 1_024).prune(created + crate::recovery::EXPIRY_MS);
    assert!(!directory.exists());
}
