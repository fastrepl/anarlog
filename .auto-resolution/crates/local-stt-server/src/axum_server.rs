use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use axum::Router;

use crate::events::LocalServerEvent;
use crate::runtime::{LocalServerRuntime, NoopRuntime};

pub struct LocalAxumServer {
    base_url: String,
    shutdown: Option<tokio::sync::watch::Sender<()>>,
    server_task: Option<tokio::task::JoinHandle<()>>,
    runtime: Arc<dyn LocalServerRuntime>,
}

impl LocalAxumServer {
    pub async fn start(router: Router, base_path: &str) -> std::io::Result<Self> {
        Self::start_with_runtime(Arc::new(NoopRuntime), router, base_path).await
    }

    pub async fn start_with_runtime(
        runtime: Arc<dyn LocalServerRuntime>,
        router: Router,
        base_path: &str,
    ) -> std::io::Result<Self> {
        runtime.emit(LocalServerEvent::Starting);

        let listener =
            tokio::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;

        let server_addr = listener.local_addr()?;
        let base_url = format!("http://{}{}", server_addr, base_path);

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(());

        let runtime_for_task = runtime.clone();
        let server_task = tokio::spawn(async move {
            let result = axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    shutdown_rx.changed().await.ok();
                })
                .await;

            if let Err(error) = result {
                tracing::error!(?error, "local_axum_server_error");
                runtime_for_task.emit(LocalServerEvent::Error {
                    error: error.to_string(),
                });
            }
        });

        runtime.emit(LocalServerEvent::Ready {
            base_url: base_url.clone(),
        });

        Ok(Self {
            base_url,
            shutdown: Some(shutdown_tx),
            server_task: Some(server_task),
            runtime,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn stop(&mut self) {
        if self.shutdown.is_none() && self.server_task.is_none() {
            return;
        }

        self.runtime.emit(LocalServerEvent::Stopping);

        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }

        if let Some(task) = self.server_task.take() {
            task.abort();
        }

        self.runtime.emit(LocalServerEvent::Stopped);
    }
}

impl Drop for LocalAxumServer {
    fn drop(&mut self) {
        self.stop();
    }
}
