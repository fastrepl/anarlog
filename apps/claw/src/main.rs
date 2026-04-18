mod config;
mod http;

use std::net::SocketAddr;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();

    let cfg = config::build()?;
    tracing::info!(
        fallback = ?cfg.providers.fallback,
        telegram = cfg.channels.telegram.is_some(),
        "claw starting"
    );

    let http_addr = control_plane_addr();

    tokio::try_join!(
        http::serve(http_addr, http::ControlPlaneState::default()),
        start_channels(cfg),
    )?;
    Ok(())
}

async fn start_channels(cfg: zeroclaw_config::schema::Config) -> Result<()> {
    zeroclaw_channels::orchestrator::start_channels(cfg).await?;
    Ok(())
}

fn control_plane_addr() -> SocketAddr {
    let port: u16 = std::env::var("CLAW_HTTP_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);
    SocketAddr::from(([0, 0, 0, 0], port))
}

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info,claw=debug".into()))
        .init();
}
