use std::{env, path::PathBuf, time::Duration};

use anarlog_enterprise_google_meet_worker::{
    AdmissionMonitorConfig, CaptureJobSupervisor, CaptureJobSupervisorConfig,
    CaptureJobSupervisorOutcome, ChromiumLaunchConfig, ChunkedRecordingConfig,
    ChunkedRecordingSink, ControlPlaneEventSink, ControlPlaneEventSinkConfig,
    FilesystemRecordingStore, GoogleMeetRuntime, GoogleMeetRuntimeConfig, X11InputConfig,
};
use anlg_meeting_capture::MeetingPlatform;
use serde::Deserialize;
use tokio::sync::watch;
use tracing_subscriber::EnvFilter;
use url::Url;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init()
        .ok();

    let config = WorkerConfig::from_env()?;
    if let Some(job_id) = config.job_id.clone() {
        return run_job(&config, job_id).await;
    }

    tracing::info!("polling control plane for dispatched Google Meet capture jobs");
    let client = reqwest::Client::new();
    let mut shutdown = false;
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = shutdown_tx.send(true);
    });
    loop {
        if *shutdown_rx.borrow() {
            break;
        }
        match list_dispatched_meet_jobs(&client, &config).await {
            Ok(job_ids) => {
                for job_id in job_ids {
                    if *shutdown_rx.borrow() {
                        shutdown = true;
                        break;
                    }
                    if let Err(error) = run_job(&config, job_id.clone()).await {
                        tracing::warn!(job_id, error = %error, "google meet capture job failed");
                    }
                }
            }
            Err(error) => {
                tracing::warn!(error = %error, "failed to list dispatched capture jobs");
            }
        }
        if shutdown {
            break;
        }
        tokio::select! {
            _ = shutdown_rx.changed() => break,
            _ = tokio::time::sleep(Duration::from_secs(15)) => {}
        }
    }
    Ok(())
}

async fn run_job(config: &WorkerConfig, job_id: String) -> anyhow::Result<()> {
    let store = FilesystemRecordingStore::new(&config.recording_root).await?;
    let sink = ChunkedRecordingSink::new(
        ChunkedRecordingConfig {
            object_prefix: format!("{}/{}", config.workspace_id, job_id),
            chunk_duration: Duration::from_secs(60),
            max_lateness: Duration::from_secs(5),
        },
        store,
    )?;
    let runtime = GoogleMeetRuntime::new(
        GoogleMeetRuntimeConfig {
            chromium: ChromiumLaunchConfig {
                binary: config.chromium_binary.clone(),
                user_data_dir: config.chromium_profile.clone(),
                locale: "en-US".into(),
                authenticated: config.authenticated,
                headless: false,
                disable_sandbox: config.disable_sandbox,
                startup_timeout: Duration::from_secs(30),
            },
            x11: X11InputConfig {
                binary: config.xdotool_binary.clone(),
                display: config.display.clone(),
                command_timeout: Duration::from_secs(5),
            },
            bot_name: config.bot_name.clone(),
            admission: AdmissionMonitorConfig::default(),
            runtime_poll_interval: Duration::from_secs(1),
        },
        sink,
    )?;
    let control_plane = ControlPlaneEventSink::new(ControlPlaneEventSinkConfig::new(
        config.control_plane_url.clone(),
        config.workspace_id.clone(),
        job_id,
        config.workspace_token.clone(),
    ))?;
    let supervisor = CaptureJobSupervisor::new(
        control_plane,
        runtime,
        config.worker_id.clone(),
        format!("lease-{}", std::process::id()),
        CaptureJobSupervisorConfig::default(),
    )?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = shutdown_tx.send(true);
    });

    match supervisor.run(shutdown_rx).await? {
        CaptureJobSupervisorOutcome::AlreadyTerminal(state) => {
            tracing::info!(?state, "capture job was already terminal");
        }
        CaptureJobSupervisorOutcome::ShutdownBeforeClaim => {
            tracing::info!("shutdown received before the capture lease was claimed");
        }
        CaptureJobSupervisorOutcome::Terminal(state) => {
            tracing::info!(?state, "capture job reached a terminal state");
        }
    }
    Ok(())
}

async fn list_dispatched_meet_jobs(
    client: &reqwest::Client,
    config: &WorkerConfig,
) -> anyhow::Result<Vec<String>> {
    let url = config.control_plane_url.join(&format!(
        "/v1/workspaces/{}/scheduled-captures",
        config.workspace_id
    ))?;
    let response = client
        .get(url)
        .bearer_auth(&config.workspace_token)
        .send()
        .await?;
    if !response.status().is_success() {
        anyhow::bail!("control plane returned {}", response.status());
    }
    let scheduled: Vec<ListedCapture> = response.json().await?;
    Ok(scheduled
        .into_iter()
        .filter(|row| {
            row.status == "dispatched"
                && row.meeting.platform == MeetingPlatform::GoogleMeet
                && row.job_id.is_some()
        })
        .filter_map(|row| row.job_id)
        .collect())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedCapture {
    job_id: Option<String>,
    status: String,
    meeting: ListedMeeting,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedMeeting {
    platform: MeetingPlatform,
}

struct WorkerConfig {
    control_plane_url: Url,
    workspace_id: String,
    job_id: Option<String>,
    workspace_token: String,
    worker_id: String,
    bot_name: String,
    chromium_binary: PathBuf,
    chromium_profile: PathBuf,
    xdotool_binary: PathBuf,
    display: String,
    recording_root: PathBuf,
    authenticated: bool,
    disable_sandbox: bool,
}

impl WorkerConfig {
    fn from_env() -> anyhow::Result<Self> {
        let recording_root = required_path("ANARLOG_ENTERPRISE_RECORDING_ROOT")?;
        Ok(Self {
            control_plane_url: required_env("ANARLOG_ENTERPRISE_CONTROL_PLANE_URL")?.parse()?,
            workspace_id: required_env("ANARLOG_ENTERPRISE_WORKSPACE_ID")?,
            job_id: env::var("ANARLOG_ENTERPRISE_CAPTURE_JOB_ID")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            workspace_token: required_env("ANARLOG_ENTERPRISE_WORKSPACE_TOKEN")?,
            worker_id: env::var("ANARLOG_ENTERPRISE_WORKER_ID")
                .unwrap_or_else(|_| format!("google-meet-{}", hostname())),
            bot_name: env::var("ANARLOG_ENTERPRISE_BOT_NAME")
                .unwrap_or_else(|_| "Anarlog Notetaker".into()),
            chromium_binary: env_path("ANARLOG_ENTERPRISE_CHROMIUM_BINARY", "/usr/bin/chromium"),
            chromium_profile: env::var_os("ANARLOG_ENTERPRISE_CHROMIUM_PROFILE")
                .map(PathBuf::from)
                .unwrap_or_else(|| recording_root.join("chromium-profile")),
            xdotool_binary: env_path("ANARLOG_ENTERPRISE_XDOTOOL_BINARY", "/usr/bin/xdotool"),
            display: env::var("DISPLAY").unwrap_or_else(|_| ":99".into()),
            recording_root,
            authenticated: env::var("ANARLOG_ENTERPRISE_AUTHENTICATED")
                .ok()
                .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true")),
            disable_sandbox: env::var("ANARLOG_ENTERPRISE_DISABLE_SANDBOX")
                .ok()
                .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
                .unwrap_or(true),
        })
    }
}

fn required_env(name: &str) -> anyhow::Result<String> {
    env::var(name).map_err(|_| anyhow::anyhow!("missing required configuration: {name}"))
}

fn required_path(name: &str) -> anyhow::Result<PathBuf> {
    Ok(PathBuf::from(required_env(name)?))
}

fn env_path(name: &str, default: &str) -> PathBuf {
    env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}

fn hostname() -> String {
    env::var("HOSTNAME").unwrap_or_else(|_| "worker".into())
}

async fn shutdown_signal() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }
}
