use std::env;

use anarlog_enterprise_zoom_rtms_worker::{
    ZoomRtmsCredentials, ZoomRtmsSession, ZoomRtmsSessionConfig, ZoomRtmsStarted,
};
use tokio::sync::{mpsc, watch};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init()
        .ok();

    let credentials = ZoomRtmsCredentials::new(
        required_env("ANARLOG_ENTERPRISE_ZOOM_CLIENT_ID")?,
        required_env("ANARLOG_ENTERPRISE_ZOOM_CLIENT_SECRET")?,
    )?;
    let started: ZoomRtmsStarted =
        serde_json::from_str(&required_env("ANARLOG_ENTERPRISE_ZOOM_STARTED_JSON")?)?;
    let mut session =
        ZoomRtmsSession::connect(&credentials, started, ZoomRtmsSessionConfig::default()).await?;
    let (transcripts_tx, mut transcripts_rx) =
        mpsc::channel::<anlg_meeting_capture::TranscriptSegment>(32);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = shutdown_tx.send(true);
    });
    let drain = tokio::spawn(async move {
        while let Some(segment) = transcripts_rx.recv().await {
            tracing::info!(
                sequence = segment.sequence,
                text_bytes = segment.text.len(),
                "received Zoom RTMS transcript segment"
            );
        }
    });
    let outcome = session
        .stream_transcripts(transcripts_tx, shutdown_rx)
        .await?;
    drain.await.ok();
    tracing::info!(?outcome, "Zoom RTMS session ended");
    Ok(())
}

fn required_env(name: &str) -> anyhow::Result<String> {
    env::var(name).map_err(|_| anyhow::anyhow!("missing required configuration: {name}"))
}
