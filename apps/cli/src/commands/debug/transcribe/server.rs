use std::net::SocketAddr;

use axum::Router;

use crate::error::{CliError, CliResult};

pub struct LocalServer {
    addr: SocketAddr,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl LocalServer {
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn api_base(&self, suffix: &str) -> String {
        format!("http://{}{}", self.addr, suffix)
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

pub async fn spawn_router(app: Router) -> CliResult<LocalServer> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| CliError::operation_failed("bind local proxy port", e.to_string()))?;
    let addr = listener
        .local_addr()
        .map_err(|e| CliError::operation_failed("read local proxy address", e.to_string()))?;
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .expect("axum serve error");
    });

    Ok(LocalServer {
        addr,
        shutdown_tx: Some(shutdown_tx),
    })
}
