mod error;
mod service;

pub use error::*;
pub use service::*;

#[cfg(test)]
// cargo test -p transcribe-whisper-local test_service -- --nocapture
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use futures_util::StreamExt;
    use hypr_audio_utils::AudioFormatExt;
    use tokio_tungstenite::{connect_async, tungstenite::Error as TungsteniteError};

    #[tokio::test]
    async fn test_service() -> Result<(), Box<dyn std::error::Error>> {
        let model_path = dirs::data_dir()
            .unwrap()
            .join("hyprnote")
            .join("models/stt/ggml-small-q8_0.bin");

        let app = TranscribeService::builder()
            .model_path(model_path)
            .build()
            .into_router(|err: String| async move { (StatusCode::INTERNAL_SERVER_ERROR, err) });

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        let server = axum::serve(listener, app);
        let server_handle = tokio::spawn(async move {
            if let Err(e) = server.await {
                println!("Server error: {}", e);
            }
        });

        let client = owhisper_client::ListenClient::builder()
            .api_base(format!("http://{}/v1", addr))
            .build_single()
            .await;

        let audio = rodio::Decoder::try_from(
            std::fs::File::open(hypr_data::english_1::AUDIO_PATH).unwrap(),
        )
        .unwrap()
        .to_i16_le_chunks(16000, 512);
        let input = audio.map(|chunk| owhisper_interface::MixedMessage::Audio(chunk));

        let _ = client.from_realtime_audio(input).await.unwrap();

        server_handle.abort();
        Ok(())
    }

    #[tokio::test]
    async fn websocket_invalid_model_path_fails_before_upgrade() {
        let app = TranscribeService::builder()
            .model_path(std::env::temp_dir().join("missing-whisper-model.bin"))
            .build()
            .into_router(|err: String| async move { (StatusCode::INTERNAL_SERVER_ERROR, err) });

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
        let app = TranscribeService::builder()
            .model_path(std::env::temp_dir().join("missing-whisper-model.bin"))
            .build()
            .into_router(|err: String| async move { (StatusCode::INTERNAL_SERVER_ERROR, err) });

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

        let response = reqwest::Client::new()
            .post(format!(
                "http://{addr}/v1/listen?channels=1&sample_rate=16000"
            ))
            .header("content-type", "audio/wav")
            .body(std::fs::read(hypr_data::english_1::AUDIO_PATH).unwrap())
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["error"], "model_load_failed");

        let _ = shutdown_tx.send(());
    }

    #[tokio::test]
    async fn batch_sse_invalid_model_path_returns_http_500_json_error() {
        let app = TranscribeService::builder()
            .model_path(std::env::temp_dir().join("missing-whisper-model.bin"))
            .build()
            .into_router(|err: String| async move { (StatusCode::INTERNAL_SERVER_ERROR, err) });

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

        let response = reqwest::Client::new()
            .post(format!(
                "http://{addr}/v1/listen?channels=1&sample_rate=16000"
            ))
            .header("content-type", "audio/wav")
            .header("accept", "text/event-stream")
            .body(std::fs::read(hypr_data::english_1::AUDIO_PATH).unwrap())
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["error"], "model_load_failed");

        let _ = shutdown_tx.send(());
    }
}
