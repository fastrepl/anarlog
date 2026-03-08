mod common;

use std::time::Duration;

fn e2e_audio_secs(default: usize) -> usize {
    std::env::var("E2E_AUDIO_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

fn scale_close_after(audio_secs: usize, default_audio: usize, default_close: u32) -> u32 {
    let ratio = audio_secs as f64 / default_audio as f64;
    ((default_close as f64 * ratio).ceil() as u32).max(1)
}

use axum::error_handling::HandleError;
use axum::{Router, http::StatusCode};
use futures_util::{SinkExt, StreamExt};
use sequential_test::sequential;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

use hypr_cactus::CloudConfig;
use transcribe_cactus::{CactusConfig, TranscribeService};

use common::model_path;

async fn run_single_channel_opts(
    cactus_config: CactusConfig,
    audio_secs: usize,
    close_after_results: u32,
    timeout_secs: u64,
) {
    let app = Router::new().route_service(
        "/v1/listen",
        HandleError::new(
            TranscribeService::builder()
                .model_path(model_path())
                .cactus_config(cactus_config)
                .build(),
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

    let ws_url = format!(
        "ws://{}/v1/listen?channels=1&sample_rate=16000&chunk_size_ms=300",
        addr
    );
    let (ws, _) = connect_async(&ws_url).await.expect("ws connect failed");
    let (mut tx, mut rx) = ws.split();

    let (close_tx, close_rx) = tokio::sync::oneshot::channel::<()>();
    let close_tx = std::cell::Cell::new(Some(close_tx));

    let t0 = std::time::Instant::now();

    let writer = tokio::spawn(async move {
        let audio = hypr_data::english_1::AUDIO;
        for chunk in audio.chunks(32_000).cycle().take(audio_secs) {
            tx.send(WsMessage::Binary(chunk.to_vec().into()))
                .await
                .unwrap();
        }
        let _ = close_rx.await;
        let _ = tx
            .send(WsMessage::Text(
                r#"{"type":"CloseStream"}"#.to_string().into(),
            ))
            .await;
    });

    let mut results = 0u32;
    let mut saw_terminal = false;
    let mut close_sent = false;

    while let Ok(Some(Ok(msg))) =
        tokio::time::timeout(Duration::from_secs(timeout_secs), rx.next()).await
    {
        match msg {
            WsMessage::Text(text) => {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                match v.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                    "Results" => {
                        let transcript = v
                            .pointer("/channel/alternatives/0/transcript")
                            .and_then(|t| t.as_str())
                            .unwrap_or("");
                        let is_final = v.get("is_final").and_then(|f| f.as_bool()).unwrap_or(false);
                        let cloud_corrected = v
                            .pointer("/metadata/extra/cloud_corrected")
                            .and_then(|b| b.as_bool())
                            .unwrap_or(false);
                        println!(
                            "[{:>5.1}s] is_final={} cloud={} {:?}",
                            t0.elapsed().as_secs_f64(),
                            is_final,
                            cloud_corrected,
                            transcript,
                        );
                        if is_final {
                            results += 1;
                        }
                        if results >= close_after_results && !close_sent {
                            close_sent = true;
                            if let Some(tx) = close_tx.take() {
                                let _ = tx.send(());
                            }
                        }
                    }
                    "Metadata" => {
                        println!("[{:>5.1}s] terminal", t0.elapsed().as_secs_f64());
                        saw_terminal = true;
                        break;
                    }
                    "Error" => panic!("ws error: {:?}", v.get("error_message")),
                    _ => {}
                }
            }
            WsMessage::Close(_) => break,
            _ => {}
        }
    }

    let _ = writer.await;
    let _ = shutdown_tx.send(());

    assert!(results > 0, "expected Results messages");
    assert!(saw_terminal, "expected terminal Metadata message");
}

async fn run_single_channel(cactus_config: CactusConfig) {
    let secs = e2e_audio_secs(100);
    run_single_channel_opts(cactus_config, secs, scale_close_after(secs, 100, 3), 120).await;
}

#[ignore = "requires local cactus model files"]
#[sequential]
#[test]
fn e2e_websocket_no_handoff() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(run_single_channel(CactusConfig {
        cloud: CloudConfig {
            threshold: Some(0.0),
            ..Default::default()
        },
        ..Default::default()
    }));
}

#[ignore = "requires local cactus model files"]
#[sequential]
#[test]
fn e2e_websocket_with_handoff() {
    let api_key = std::env::var("CACTUS_CLOUD_API_KEY")
        .expect("CACTUS_CLOUD_API_KEY must be set for this test");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let secs = e2e_audio_secs(120);
    rt.block_on(run_single_channel_opts(
        CactusConfig {
            // Well below model defaults (Whisper=0.4, Moonshine=0.35) to trigger aggressively
            cloud: CloudConfig {
                api_key: Some(api_key),
                threshold: Some(0.05),
            },
            ..Default::default()
        },
        secs,
        scale_close_after(secs, 120, 30),
        180,
    ));
}

#[ignore = "requires local cactus model files"]
#[sequential]
#[test]
fn e2e_websocket_dual_channel_no_handoff() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let dual_secs = e2e_audio_secs(100);
    let dual_close = scale_close_after(dual_secs, 100, 6);
    rt.block_on(async move {
        let app = Router::new().route_service(
            "/v1/listen",
            HandleError::new(
                TranscribeService::builder()
                    .model_path(model_path())
                    .cactus_config(CactusConfig {
                        cloud: CloudConfig {
                            threshold: Some(0.0),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .build(),
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

        let ws_url = format!(
            "ws://{}/v1/listen?channels=2&sample_rate=16000&chunk_size_ms=300",
            addr
        );
        let (ws, _) = connect_async(&ws_url).await.expect("ws connect failed");
        let (mut tx, mut rx) = ws.split();

        let audio = hypr_data::english_1::AUDIO;
        let num_frames = audio.len() / 2;
        let mut interleaved = Vec::with_capacity(num_frames * 4);
        for i in 0..num_frames {
            interleaved.extend_from_slice(&audio[i * 2..i * 2 + 2]);
            interleaved.extend_from_slice(&audio[i * 2..i * 2 + 2]);
        }

        let (close_tx, close_rx) = tokio::sync::oneshot::channel::<()>();
        let close_tx = std::cell::Cell::new(Some(close_tx));

        let t0 = std::time::Instant::now();

        let writer = tokio::spawn(async move {
            for chunk in interleaved.chunks(64_000).cycle().take(dual_secs) {
                tx.send(WsMessage::Binary(chunk.to_vec().into()))
                    .await
                    .unwrap();
            }
            let _ = close_rx.await;
            let _ = tx
                .send(WsMessage::Text(
                    r#"{"type":"CloseStream"}"#.to_string().into(),
                ))
                .await;
        });

        let mut results = 0u32;
        let mut saw_terminal = false;
        let mut close_sent = false;
        let mut channels_seen = std::collections::HashSet::new();

        while let Ok(Some(Ok(msg))) =
            tokio::time::timeout(Duration::from_secs(120), rx.next()).await
        {
            match msg {
                WsMessage::Text(text) => {
                    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                        continue;
                    };
                    match v.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                        "Results" => {
                            let ch = v
                                .pointer("/channel_index/0")
                                .and_then(|c| c.as_i64())
                                .unwrap_or(-1);
                            let transcript = v
                                .pointer("/channel/alternatives/0/transcript")
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            let is_final =
                                v.get("is_final").and_then(|f| f.as_bool()).unwrap_or(false);
                            println!(
                                "[{:>5.1}s] ch={} is_final={} {:?}",
                                t0.elapsed().as_secs_f64(),
                                ch,
                                is_final,
                                transcript,
                            );
                            channels_seen.insert(ch);
                            results += 1;
                            if results >= dual_close && !close_sent {
                                close_sent = true;
                                if let Some(tx) = close_tx.take() {
                                    let _ = tx.send(());
                                }
                            }
                        }
                        "Metadata" => {
                            println!("[{:>5.1}s] terminal", t0.elapsed().as_secs_f64());
                            saw_terminal = true;
                            break;
                        }
                        "Error" => panic!("ws error: {:?}", v.get("error_message")),
                        _ => {}
                    }
                }
                WsMessage::Close(_) => break,
                _ => {}
            }
        }

        let _ = writer.await;
        let _ = shutdown_tx.send(());

        assert!(results > 0, "expected Results messages");
        assert!(saw_terminal, "expected terminal Metadata message");
        assert!(
            channels_seen.contains(&0) && channels_seen.contains(&1),
            "expected results from both channels, got {channels_seen:?}",
        );
    });
}
