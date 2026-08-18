#![forbid(unsafe_code)]

pub mod api;
pub mod auth;
pub mod capture;
pub mod config;
pub mod projector;
pub mod store;
pub mod zoom;

use std::{future::Future, sync::Arc};

use anyhow::Context;
use auth::StaticTokenAuthenticator;
use config::Config;
use store::PostgresStore;
use tokio::net::TcpListener;
use zoom::{ZoomCaptureDispatcher, ZoomWebhookService};

pub async fn run(config: Config) -> anyhow::Result<()> {
    let state = configured_state(&config).await?;
    let listener = TcpListener::bind(config.bind_address)
        .await
        .with_context(|| format!("failed to bind {}", config.bind_address))?;
    let address = listener.local_addr()?;
    tracing::info!(%address, "enterprise capture control plane listening");
    serve(listener, state, shutdown_signal()).await?;
    Ok(())
}

pub async fn configured_state(config: &Config) -> anyhow::Result<api::AppState> {
    let authenticator = StaticTokenAuthenticator::new(
        config
            .workspace_tokens
            .iter()
            .map(|(workspace_id, token)| (workspace_id.clone(), token.clone())),
    )
    .context("invalid workspace bearer credentials")?;
    let store = PostgresStore::connect(
        &config.database_url,
        config.database_max_connections,
        config.database_acquire_timeout,
    )
    .await
    .context("failed to connect to Postgres")?;
    store
        .migrate()
        .await
        .context("failed to apply database migrations")?;
    let store = store.into_shared();
    let mut state = api::AppState::new(store.clone(), Arc::new(authenticator));
    if let Some(zoom) = &config.zoom {
        let dispatcher = Arc::new(ZoomCaptureDispatcher::new(
            store,
            zoom.credentials().clone(),
        ));
        dispatcher
            .recover_pending()
            .await
            .context("failed to recover durable Zoom dispatches")?;
        dispatcher.clone().spawn_recovery();
        state = state.with_zoom(Arc::new(ZoomWebhookService::new(zoom.clone(), dispatcher)));
    }
    Ok(state)
}

pub async fn serve(
    listener: TcpListener,
    state: api::AppState,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    axum::serve(listener, api::router(state))
        .with_graceful_shutdown(shutdown)
        .await
}

async fn shutdown_signal() {
    let interrupt = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "failed to listen for Ctrl-C");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "failed to listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }
    tracing::info!("shutdown signal received");
}
