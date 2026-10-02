mod error;
mod service;

pub use error::*;
pub use service::*;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use std::net::SocketAddr;
    use tokio::sync::oneshot;
    use tokio_tungstenite::{connect_async, tungstenite::Error as TungsteniteError};

    async fn serve_missing_model() -> (SocketAddr, oneshot::Sender<()>) {
        let app = TranscribeService::builder()
            .model_path(std::env::temp_dir().join("missing-whisper-model.bin"))
            .build()
            .into_router(|err: String| async move { (StatusCode::INTERNAL_SERVER_ERROR, err) });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });

        (addr, shutdown_tx)
    }

    #[tokio::test]
    async fn websocket_invalid_model_path_fails_before_upgrade() {
        let (addr, shutdown_tx) = serve_missing_model().await;

        let result = connect_async(format!(
            "ws://{addr}/v1/listen?channels=1&sample_rate=16000"
        ))
        .await;

        match result {
            Err(TungsteniteError::Http(response)) => {
                assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
                let body = response
                    .body()
                    .as_ref()
                    .and_then(|bytes| std::str::from_utf8(bytes).ok())
                    .unwrap_or_default();
                assert!(
                    body.contains("failed to load model"),
                    "unexpected body: {body}"
                );
            }
            other => panic!("expected HTTP upgrade failure, got {other:?}"),
        }

        let _ = shutdown_tx.send(());
    }

    #[tokio::test]
    async fn batch_invalid_model_path_returns_http_500_json_error() {
        for accept in [None, Some("text/event-stream")] {
            let (addr, shutdown_tx) = serve_missing_model().await;
            let mut request = reqwest::Client::new()
                .post(format!(
                    "http://{addr}/v1/listen?channels=1&sample_rate=16000"
                ))
                .header("content-type", "audio/wav")
                .body(std::fs::read(anlg_data::english_1::AUDIO_PATH).unwrap());
            if let Some(accept) = accept {
                request = request.header("accept", accept);
            }
            let response = request.send().await.unwrap();

            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
            let body: serde_json::Value = response.json().await.unwrap();
            assert_eq!(body["error"], "model_load_failed");

            let _ = shutdown_tx.send(());
        }
    }
}
