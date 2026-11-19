//! The process boundary, with a shell script standing in for the helper: it replays recorded
//! wire messages, answers stdin commands, and can refuse, hang, or die. No audio device is used.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use anlg_audio::meeting::{
    CallPlatform, CaptureEndReason, CaptureIdentity, CaptureInstant, ConfirmedCall,
    MeetingCaptureEvent, MicProcessing, TrackKind, TrackStatus,
};
use anlg_audio::{CaptureChannel, CaptureConfig};
use futures_util::StreamExt;

use super::wire::{Track, encode_event, encode_pcm};
use super::*;

const SCRIPT: &str = r#"#!/bin/sh
dir=$(dirname "$0")
printf '%s\n' "$@" > "$dir/args"
case "$1" in version|calls) cat "$dir/$1.bin"; exit 0;; esac
mode=$(cat "$dir/mode")
[ "$mode" = hang ] && exec sleep 30
cat "$dir/start.bin"
[ "$mode" = crash ] && kill -9 $$
[ "$mode" = refuse ] && exit 1
while IFS= read -r line; do
  case "$line" in
    pause) cat "$dir/paused.bin" ;;
    resume) cat "$dir/resumed.bin" ;;
    stop) break ;;
  esac
done
touch "$dir/stopped"
cat "$dir/end.bin"
"#;

struct Fake {
    dir: PathBuf,
}

impl Fake {
    fn new(mode: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "anarlog-native-meeting-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("staging")).unwrap();
        let helper = dir.join(HELPER_NAME);
        std::fs::write(&helper, SCRIPT).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let fake = Self { dir };
        fake.write("mode", mode.as_bytes());
        fake.write(
            "version.bin",
            &encode_event("version", &[("protocol", PROTOCOL_VERSION)]),
        );
        for name in [
            "start.bin",
            "paused.bin",
            "resumed.bin",
            "end.bin",
            "calls.bin",
        ] {
            fake.write(name, &[]);
        }
        fake
    }

    fn write(&self, name: &str, bytes: &[u8]) {
        std::fs::write(self.dir.join(name), bytes).unwrap();
    }

    fn engine(&self, start: Duration, control: Duration) -> NativeMeetingEngine {
        let mut engine = NativeMeetingEngine::with_timeouts(
            Some(self.dir.join(HELPER_NAME)),
            Timeouts {
                start,
                control,
                stop: Duration::from_secs(5),
                probe: Duration::from_secs(5),
            },
        );
        engine.supported = true;
        engine
    }

    fn args(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.join("args"))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn request(&self, scope: ScopeRequest, mic_device: Option<&str>) -> MeetingCaptureRequest {
        MeetingCaptureRequest {
            capture: identity(),
            scope,
            staging_dir: self.dir.join("staging"),
            config: CaptureConfig {
                sample_rate: 16_000,
                chunk_size: 1_920,
                mic_device: mic_device.map(str::to_string),
                enable_aec: true,
            },
            locale: None,
        }
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn identity() -> CaptureIdentity {
    CaptureIdentity {
        account_id: "account".into(),
        session_id: "session".into(),
        capture_id: "capture-1".into(),
    }
}

fn meet() -> ConfirmedCall {
    ConfirmedCall {
        pid: 42,
        bundle_id: "com.google.Chrome".into(),
        platform: CallPlatform::GoogleMeet,
        call_url: Some("https://meet.google.com/abc-defg-hij".into()),
    }
}

fn started() -> Vec<u8> {
    encode_event(
        "started",
        &[
            ("scope", "application"),
            ("pid", "42"),
            ("bundleId", "com.google.Chrome"),
            ("platform", "google-meet"),
            ("callUrl", "https://meet.google.com/abc-defg-hij"),
            ("microphoneName", "MacBook Pro Microphone"),
            ("micProcessing", "voice_processed"),
        ],
    )
}

fn one_second() -> Vec<u8> {
    let mut bytes = encode_event("prepared", &[("captureId", "capture-1")]);
    bytes.extend(encode_event(
        "call-audio-configured",
        &[("scope", "application")],
    ));
    bytes.extend(encode_pcm(Track::Microphone, 0, &[0.25; 48_000]));
    bytes.extend(started());
    bytes.extend(encode_pcm(Track::Remote, 0, &[0.5; 48_000]));
    bytes.extend(encode_event("tick", &[("frame", "48000")]));
    bytes
}

fn ended(reason: &str, frames: &str) -> Vec<u8> {
    encode_event(
        "ended",
        &[
            ("reason", reason),
            ("frame", frames),
            ("frames", frames),
            ("microphoneFrames", "48000"),
            ("remoteFrames", "48000"),
        ],
    )
}

async fn next_event(events: &mut anlg_audio::meeting::MeetingEventStream) -> MeetingCaptureEvent {
    tokio::time::timeout(Duration::from_secs(5), events.next())
        .await
        .expect("an event in time")
        .expect("the event stream is open")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_capture_streams_audio_acknowledges_pause_and_stops_cleanly() {
    let fake = Fake::new("run");
    fake.write("start.bin", &one_second());
    fake.write("paused.bin", &encode_event("paused", &[("frame", "48000")]));
    fake.write(
        "resumed.bin",
        &encode_event("resumed", &[("frame", "96000")]),
    );
    fake.write("end.bin", &ended("user_stopped", "144000"));
    let engine = fake.engine(Duration::from_secs(5), Duration::from_secs(5));
    let mut capture = engine
        .open(fake.request(ScopeRequest::Discover, None))
        .unwrap();
    assert_eq!(capture.scope, CaptureScope::Call(meet()));
    assert_eq!(capture.provenance.mic, MicProcessing::VoiceProcessed);
    assert_eq!(capture.provenance.engine, CaptureEngine::NativeMeeting);
    assert_eq!(
        &fake.args()[..6],
        [
            "record",
            "--output",
            fake.dir.join("staging").to_str().unwrap(),
            "--capture-id",
            "capture-1",
            "--scope",
        ]
    );
    assert_eq!(fake.args()[6], "discover");

    capture.control.pause().await.unwrap();
    capture.control.resume().await.unwrap();
    capture.control.stop().await.unwrap();

    let mut ready = Vec::new();
    let mut samples = 0;
    while let Ok(Some(item)) =
        tokio::time::timeout(Duration::from_millis(300), capture.stream.next_event()).await
    {
        match item.unwrap() {
            CaptureEvent::ChannelReady { channel, .. } => ready.push(channel),
            CaptureEvent::Frame(frame) | CaptureEvent::Gap(frame) => {
                assert!(
                    frame.aec_mic.is_some(),
                    "voice-processed audio is never raw"
                );
                samples += frame.raw_mic.len();
            }
            CaptureEvent::ChannelFailed { .. } => panic!("no channel failed"),
        }
    }
    assert_eq!(ready, [CaptureChannel::Speaker, CaptureChannel::Mic]);
    assert_eq!(
        samples, 48_000,
        "3 s of capture time at 16 kHz, the pause padded"
    );

    let mut seen = Vec::new();
    loop {
        let event = next_event(&mut capture.events).await;
        let done = matches!(event, MeetingCaptureEvent::Ended { .. });
        seen.push(event);
        if done {
            break;
        }
    }
    assert!(seen.contains(&MeetingCaptureEvent::TrackStatus {
        track: TrackKind::Microphone,
        status: TrackStatus::Ready,
    }));
    assert!(seen.contains(&MeetingCaptureEvent::Paused {
        at: CaptureInstant(48_000)
    }));
    assert_eq!(
        seen.last(),
        Some(&MeetingCaptureEvent::Ended {
            at: CaptureInstant(144_000),
            reason: CaptureEndReason::UserStopped,
        })
    );
    // A user stop keeps the stream open: the listener sees no failure while it shuts down.
    assert!(
        tokio::time::timeout(Duration::from_millis(200), capture.stream.next_event())
            .await
            .is_err()
    );
    // Staging is released only on request, after the helper finished.
    assert!(fake.dir.join("staging").exists());
    capture.control.release_staging().unwrap();
    assert!(!fake.dir.join("staging").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_helper_that_dies_fails_the_stream_and_ends_the_capture() {
    let fake = Fake::new("crash");
    fake.write("start.bin", &one_second());
    let engine = fake.engine(Duration::from_secs(5), Duration::from_secs(5));
    let mut capture = engine
        .open(fake.request(ScopeRequest::Discover, None))
        .unwrap();
    let mut failed = None;
    while let Some(item) = capture.stream.next_event().await {
        if let Err(error) = item {
            failed = Some(error);
        }
    }
    assert!(
        matches!(failed, Some(Error::MeetingCaptureFailed(_))),
        "{failed:?}"
    );
    loop {
        if let MeetingCaptureEvent::Ended { reason, .. } = next_event(&mut capture.events).await {
            assert_eq!(reason, CaptureEndReason::CaptureFailed);
            break;
        }
    }
    assert!(capture.control.stop().await.is_err());
    assert!(capture.control.pause().await.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refusal_before_start_is_an_unavailable_error_with_its_reason() {
    let fake = Fake::new("refuse");
    fake.write(
        "start.bin",
        &encode_event(
            "failed",
            &[
                ("code", "microphone_not_default"),
                (
                    "message",
                    "The selected microphone is not the Mac's default input",
                ),
            ],
        ),
    );
    let engine = fake.engine(Duration::from_secs(5), Duration::from_secs(5));
    let error = engine
        .open(fake.request(ScopeRequest::Discover, Some("USB Mic")))
        .err()
        .unwrap();
    assert_eq!(
        error,
        Error::MeetingCaptureUnavailable(
            "microphone_not_default: The selected microphone is not the Mac's default input".into()
        )
    );
    assert!(
        fake.args()
            .windows(2)
            .any(|pair| pair == ["--microphone", "USB Mic"])
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_helper_that_never_starts_is_killed_at_the_deadline() {
    let fake = Fake::new("hang");
    let engine = fake.engine(Duration::from_millis(300), Duration::from_secs(5));
    let started = Instant::now();
    let error = engine
        .open(fake.request(ScopeRequest::Discover, None))
        .err()
        .unwrap();
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(
        error,
        Error::MeetingCaptureFailed("the native helper did not start in time".into())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_reopen_asks_for_exactly_the_call_it_had_and_refuses_another() {
    let fake = Fake::new("run");
    fake.write("start.bin", &one_second());
    let engine = fake.engine(Duration::from_secs(5), Duration::from_secs(5));
    let capture = engine
        .open(fake.request(ScopeRequest::Exactly(CaptureScope::Call(meet())), None))
        .unwrap();
    assert_eq!(
        &fake.args()[5..],
        [
            "--scope",
            "call",
            "--pid",
            "42",
            "--bundle-id",
            "com.google.Chrome",
            "--platform",
            "google-meet",
            "--call-url",
            "https://meet.google.com/abc-defg-hij",
        ]
    );
    drop(capture);
    // A helper that comes back with another scope is not accepted for this capture.
    let fake = Fake::new("run");
    fake.write("start.bin", &one_second());
    let engine = fake.engine(Duration::from_secs(5), Duration::from_secs(5));
    let error = engine
        .open(fake.request(ScopeRequest::Exactly(CaptureScope::System), None))
        .err()
        .unwrap();
    assert_eq!(
        error,
        Error::MeetingCaptureFailed("the native helper opened a different scope".into())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn dropping_the_capture_asks_the_helper_to_finish_its_tracks() {
    let fake = Fake::new("run");
    fake.write("start.bin", &one_second());
    fake.write("end.bin", &ended("user_stopped", "48000"));
    let engine = fake.engine(Duration::from_secs(5), Duration::from_secs(5));
    let capture = engine
        .open(fake.request(ScopeRequest::Discover, None))
        .unwrap();
    drop(capture);
    for _ in 0..100 {
        if fake.dir.join("stopped").exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("the helper was not asked to stop");
}

/// A raw microphone on speakers hears the call. The capture's own AEC setting decides whether
/// Anarlog cancels it; headphones leave it raw.
#[tokio::test(flavor = "multi_thread")]
async fn a_raw_microphone_is_echo_cancelled_when_the_capture_asks() {
    let remote: Vec<f32> = (0..48_000)
        .map(|n| (n as f32 * 0.37).sin() * (n as f32 * 0.0011).sin() * 0.3)
        .collect();
    let mut echo = vec![0.0; 2_304];
    echo.extend(remote[..48_000 - 2_304].iter().map(|s| s * 0.25));
    let mut start = encode_event("microphone-configured", &[("voiceProcessing", "false")]);
    start.extend(encode_event(
        "started",
        &[("scope", "system"), ("micProcessing", "raw")],
    ));
    start.extend(encode_event(
        "call-audio-configured",
        &[("scope", "system")],
    ));
    start.extend(encode_pcm(Track::Microphone, 0, &echo));
    start.extend(encode_pcm(Track::Remote, 0, &remote));
    start.extend(encode_event("tick", &[("frame", "48000")]));
    for enable_aec in [true, false] {
        let fake = Fake::new("run");
        fake.write("start.bin", &start);
        let engine = fake.engine(Duration::from_secs(5), Duration::from_secs(5));
        let mut request = fake.request(ScopeRequest::Discover, None);
        request.config.enable_aec = enable_aec;
        let mut capture = engine.open(request).unwrap();
        assert_eq!(capture.provenance.mic, MicProcessing::Raw);
        let mut frames = 0;
        // Echo cancellation can hold the first frame back well past 500 ms on a loaded runner.
        let mut wait = Duration::from_secs(10);
        while let Ok(Some(item)) = tokio::time::timeout(wait, capture.stream.next_event()).await {
            if let CaptureEvent::Frame(frame) = item.unwrap() {
                frames += 1;
                wait = Duration::from_millis(500);
                let cancelled = frame
                    .aec_mic
                    .as_ref()
                    .is_some_and(|aec| !Arc::ptr_eq(aec, &frame.raw_mic));
                assert_eq!(cancelled, enable_aec);
            }
        }
        assert!(frames > 0);
    }
}

/// A listener that stops reading backs the audio up into the helper instead of buffering it
/// here without bound, and controls fail loudly rather than hang.
#[tokio::test(flavor = "multi_thread")]
async fn an_unread_stream_applies_backpressure_and_controls_time_out() {
    let fake = Fake::new("run");
    let mut start = one_second();
    for second in 1..40u64 {
        let frame = second * 48_000;
        start.extend(encode_pcm(Track::Microphone, frame, &[0.25; 48_000]));
        start.extend(encode_event(
            "tick",
            &[("frame", &(frame + 48_000).to_string())],
        ));
    }
    fake.write("start.bin", &start);
    fake.write("paused.bin", &encode_event("paused", &[("frame", "1")]));
    let engine = fake.engine(Duration::from_secs(5), Duration::from_millis(300));
    let capture = engine
        .open(fake.request(ScopeRequest::Discover, None))
        .unwrap();
    let error = capture.control.pause().await.unwrap_err();
    assert_eq!(
        error,
        Error::MeetingCaptureFailed("the native helper did not acknowledge pause".into())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn availability_and_calls_come_from_the_helper_itself() {
    let fake = Fake::new("run");
    let mut calls = encode_event(
        "call",
        &[
            ("bundleId", "com.google.Chrome"),
            ("platform", "google-meet"),
            ("status", "in-call"),
            ("callUrl", "https://meet.google.com/abc-defg-hij"),
        ],
    );
    calls.extend(encode_event(
        "call",
        &[
            ("bundleId", "us.zoom.xos"),
            ("platform", "zoom"),
            ("status", "unknown"),
        ],
    ));
    calls.extend(encode_event("done", &[]));
    fake.write("calls.bin", &calls);
    let engine = fake.engine(Duration::from_secs(5), Duration::from_secs(5));
    assert_eq!(
        engine.availability(),
        EngineAvailability::Available {
            selects_microphone: false
        }
    );
    let apps = engine.calls_in_progress().unwrap();
    assert_eq!(apps.len(), 2);
    assert_eq!(apps[0].status, CallStatus::InCall);
    assert_eq!(apps[1].call_url, None);

    let stale = Fake::new("run");
    stale.write(
        "version.bin",
        &encode_event("version", &[("protocol", "0")]),
    );
    assert!(matches!(
        stale
            .engine(Duration::from_secs(1), Duration::from_secs(1))
            .availability(),
        EngineAvailability::Unavailable { .. }
    ));
    let mut missing = NativeMeetingEngine::new(None);
    missing.supported = true;
    assert!(matches!(
        missing.availability(),
        EngineAvailability::Unavailable { .. }
    ));
}
