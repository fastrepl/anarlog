use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Router;
use axum::error_handling::HandleError;
use axum::http::StatusCode;
use transcribe_cactus::TranscribeService;

pub struct LocalServer {
    pub addr: SocketAddr,
    _shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

pub async fn spawn(model_path: PathBuf) -> LocalServer {
    let app = Router::new().route_service(
        "/v1/listen",
        HandleError::new(
            TranscribeService::builder().model_path(model_path).build(),
            |err: String| async move { (StatusCode::INTERNAL_SERVER_ERROR, err) },
        ),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .unwrap();
    });

    LocalServer {
        addr,
        _shutdown_tx: shutdown_tx,
    }
}
