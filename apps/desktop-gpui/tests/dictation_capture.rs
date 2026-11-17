//! Dictation's recorder against the shared mock microphone. Its own binary, so the mock's
//! speaker playback can be switched off before any thread reads the environment.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anlg_audio::AudioProvider;
use desktop_gpui::dictation::recorder::{Recorder, SAMPLE_RATE, Update};

#[test]
fn records_the_mock_microphone_for_one_owner_at_a_time_then_removes_the_audio() {
    // SAFETY: this is the binary's only test and runs before anything reads the variable.
    unsafe { std::env::set_var("MOCK_PLAYBACK", "0") };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let recorder = Recorder::new(runtime.handle().clone());
    let audio: Arc<dyn AudioProvider> = Arc::new(anlg_audio_mock::MockAudio::new(1));
    let levels = Arc::new(Mutex::new(Vec::new()));
    let seen = levels.clone();
    recorder
        .start(
            audio.clone(),
            None,
            "system".into(),
            None,
            Some(Arc::new(move |update| {
                if let Update::Amplitude(level) = update {
                    seen.lock().unwrap().push(level);
                }
            })),
        )
        .unwrap();
    // Chat's dictation cannot take the microphone, stop or cancel the system recording.
    assert!(
        recorder
            .start(audio.clone(), None, "chat".into(), None, None)
            .is_err()
    );
    runtime.block_on(async {
        assert!(recorder.stop("chat").await.is_err());
        assert!(recorder.cancel("chat").await.is_err());
        // The clip opens with silence; wait for speech to register a level.
        tokio::time::timeout(Duration::from_secs(20), async {
            while !levels.lock().unwrap().iter().any(|level| *level > 0.) {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        let recorded = recorder.stop("system").await.unwrap();
        let reader = hound::WavReader::open(&recorded.path).unwrap();
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().sample_rate, SAMPLE_RATE);
        assert!(reader.len() > 0 && recorded.duration_ms > 0);
        recorder.discard(&recorded.path).unwrap();
        assert!(!recorded.path.exists());
        assert!(recorder.discard(&recorded.path).is_err());
    });

    // Cancelling frees the microphone for the next owner.
    recorder
        .start(audio.clone(), None, "chat".into(), None, None)
        .unwrap();
    runtime.block_on(async {
        recorder.cancel("chat").await.unwrap();
        recorder.cancel("chat").await.unwrap();
    });
    recorder
        .start(audio, None, "system".into(), None, None)
        .unwrap();
    runtime.block_on(async { recorder.cancel("system").await.unwrap() });
}
