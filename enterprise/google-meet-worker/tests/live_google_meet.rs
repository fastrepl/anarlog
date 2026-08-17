use std::{
    env,
    ffi::OsString,
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use anarlog_enterprise_google_meet_worker::{
    AdmissionMonitorConfig, CaptureJobRuntime, ChromiumLaunchConfig, ChunkedRecordingConfig,
    ChunkedRecordingSink, FilesystemRecordingStore, GoogleMeetRuntime, GoogleMeetRuntimeConfig,
    GoogleMeetUrl, WorkerCheckpoint, WorkerLifecycle, X11InputConfig,
};
use anlg_meeting_capture::{BotState, CaptureEventPayload};
use tokio::sync::mpsc;

#[tokio::test]
#[ignore = "requires a disposable live Google Meet and a Linux desktop runtime"]
async fn captures_a_live_google_meet_and_cleans_up() -> Result<(), Box<dyn std::error::Error>> {
    let meeting_url = GoogleMeetUrl::parse(&env::var("ANLG_LIVE_GOOGLE_MEET_URL")?)?;
    let run_timeout = Duration::from_secs(env_u64("ANLG_LIVE_RUN_SECONDS", 300)?);
    let directory = tempfile::tempdir()?;
    let (chromium_profile, authenticated) = chromium_profile(
        directory.path(),
        env::var_os("ANLG_LIVE_CHROMIUM_PROFILE_DIR"),
    )?;
    let recording_root = env::var_os("ANLG_LIVE_RECORDING_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| directory.path().join("recordings"));
    let store = FilesystemRecordingStore::new(&recording_root).await?;
    let sink = ChunkedRecordingSink::new(
        ChunkedRecordingConfig {
            object_prefix: "live-google-meet".into(),
            chunk_duration: Duration::from_secs(10),
            max_lateness: Duration::from_secs(2),
        },
        store,
    )?;
    let mut runtime = GoogleMeetRuntime::new(
        GoogleMeetRuntimeConfig {
            chromium: ChromiumLaunchConfig {
                binary: env_path("ANLG_LIVE_CHROMIUM_BINARY", "/usr/bin/chromium"),
                user_data_dir: chromium_profile,
                locale: "en-US".into(),
                authenticated,
                headless: false,
                disable_sandbox: true,
                startup_timeout: Duration::from_secs(30),
            },
            x11: X11InputConfig {
                binary: env_path("ANLG_LIVE_XDOTOOL_BINARY", "/usr/bin/xdotool"),
                display: env::var("DISPLAY").unwrap_or_else(|_| ":99".into()),
                command_timeout: Duration::from_secs(5),
            },
            bot_name: "Anarlog Reliability Bot".into(),
            admission: AdmissionMonitorConfig {
                timeout: Duration::from_secs(120),
                poll_interval: Duration::from_millis(500),
            },
            runtime_poll_interval: Duration::from_secs(1),
        },
        sink,
    )?;
    let checkpoint = WorkerCheckpoint {
        job_id: "live-google-meet-job".into(),
        bot_id: "live-google-meet-bot".into(),
        provider: anlg_meeting_capture::CaptureProviderKind::Anarlog,
        meeting: anlg_meeting_capture::MeetingReference {
            platform: anlg_meeting_capture::MeetingPlatform::GoogleMeet,
            url: meeting_url.as_str().into(),
            external_id: None,
            calendar_event_id: None,
        },
        state: BotState::Queued,
        next_sequence: 0,
    };
    let mut lifecycle = WorkerLifecycle::new(checkpoint.bot_id.clone());
    let (events_tx, mut events_rx) = mpsc::channel(32);
    let event_collector = tokio::spawn(async move {
        let mut events = Vec::new();
        while let Some(event) = events_rx.recv().await {
            println!("ANLG_LIVE_EVENT {event:?}");
            events.push(event);
        }
        events
    });

    let run_result = tokio::time::timeout(
        run_timeout,
        runtime.run(&checkpoint, &mut lifecycle, events_tx.clone()),
    )
    .await;
    let cleanup_outputs = runtime.cleanup().await?;
    for payload in cleanup_outputs {
        println!("ANLG_LIVE_CLEANUP_OUTPUT {payload:?}");
    }
    drop(events_tx);
    let events = event_collector.await?;

    run_result??;
    assert!(lifecycle.state().is_terminal());
    assert!(events.iter().any(|event| {
        matches!(
            &event.payload,
            CaptureEventPayload::Lifecycle(transition) if transition.to == BotState::Joined
        )
    }));
    assert!(events.iter().any(|event| {
        matches!(
            &event.payload,
            CaptureEventPayload::Lifecycle(transition) if transition.to == BotState::Capturing
        )
    }));
    assert!(
        events
            .iter()
            .any(|event| { matches!(event.payload, CaptureEventPayload::RecordingChunkReady(_)) })
    );
    assert!(recording_root.join("live-google-meet").is_dir());
    Ok(())
}

fn env_u64(name: &str, default: u64) -> Result<u64, Box<dyn std::error::Error>> {
    Ok(match env::var(name) {
        Ok(value) => value.parse()?,
        Err(env::VarError::NotPresent) => default,
        Err(error) => return Err(error.into()),
    })
}

fn env_path(name: &str, default: &str) -> PathBuf {
    env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| default.into())
}

fn chromium_profile(
    temporary_root: &Path,
    configured: Option<OsString>,
) -> io::Result<(PathBuf, bool)> {
    let Some(configured) = configured else {
        return Ok((temporary_root.join("chromium-profile"), false));
    };
    let path = PathBuf::from(configured);
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ANLG_LIVE_CHROMIUM_PROFILE_DIR must be an absolute path",
        ));
    }
    Ok((path, true))
}

#[test]
fn persistent_chromium_profile_enables_authenticated_mode() {
    let temporary_root = tempfile::tempdir().unwrap();
    assert_eq!(
        chromium_profile(temporary_root.path(), None).unwrap(),
        (temporary_root.path().join("chromium-profile"), false)
    );

    let persistent = temporary_root.path().join("persistent-profile");
    assert_eq!(
        chromium_profile(
            temporary_root.path(),
            Some(persistent.clone().into_os_string())
        )
        .unwrap(),
        (persistent, true)
    );
    assert_eq!(
        chromium_profile(temporary_root.path(), Some("relative-profile".into()))
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
}
