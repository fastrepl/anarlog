mod common;

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as WsMessage;

use hypr_cactus::CloudConfig;
use transcribe_cactus::CactusConfig;

fn load_english_10_mono_pcm() -> Vec<u8> {
    let mic = hypr_audio_utils::source_from_path(hypr_data::english_10::AUDIO_MIC_MP3_PATH)
        .expect("failed to open mic audio");
    let mic_f32 = hypr_audio_utils::resample_audio(mic, 16_000).expect("failed to resample mic");

    let spk = hypr_audio_utils::source_from_path(hypr_data::english_10::AUDIO_SPK_MP3_PATH)
        .expect("failed to open spk audio");
    let spk_f32 = hypr_audio_utils::resample_audio(spk, 16_000).expect("failed to resample spk");

    let mixed = hypr_audio_utils::mix_audio_f32(&mic_f32, &spk_f32);

    // First 60 seconds (16000 samples/s × 60s)
    let limit = 16_000 * 60;
    let samples = &mixed[..mixed.len().min(limit)];
    hypr_audio_utils::f32_to_i16_bytes(samples.iter().copied()).to_vec()
}

fn snapshot_settings() -> insta::Settings {
    let mut s = insta::Settings::clone_current();
    s.add_redaction("[].metadata.request_id", "[request_id]");
    s.add_redaction("[].metadata.model_uuid", "[model_uuid]");
    s.add_redaction("[].metadata.extra.started_unix_millis", "[timestamp]");
    s.add_redaction("[].metadata.extra.decode_tps", "[variable]");
    s.add_redaction("[].metadata.extra.prefill_tps", "[variable]");
    s.add_redaction("[].metadata.extra.time_to_first_token_ms", "[variable]");
    s.add_redaction("[].metadata.extra.total_time_ms", "[variable]");
    s.add_redaction("[].metadata.extra.buffer_duration_ms", "[variable]");
    s.add_redaction("[].metadata.extra.decode_tokens", "[variable]");
    s.add_redaction("[].metadata.extra.prefill_tokens", "[variable]");
    s.add_redaction("[].metadata.extra.total_tokens", "[variable]");
    s.add_redaction("[].request_id", "[request_id]");
    s.add_redaction("[].created", "[timestamp]");
    s
}

#[ignore = "requires local cactus model files"]
#[test]
fn e2e_snapshot_english_10() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        let audio = load_english_10_mono_pcm();

        let (addr, shutdown_tx) = common::start_test_server(CactusConfig {
            cloud: CloudConfig {
                threshold: Some(0.0),
                ..Default::default()
            },
            ..Default::default()
        })
        .await;

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
            for chunk in audio.chunks(32_000) {
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

        let mut packets: Vec<serde_json::Value> = Vec::new();
        let mut confirmed_count = 0u32;
        let target_confirmed = 5;

        while let Ok(Some(Ok(msg))) =
            tokio::time::timeout(Duration::from_secs(120), rx.next()).await
        {
            match msg {
                WsMessage::Text(text) => {
                    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                        continue;
                    };
                    let msg_type = v["type"].as_str().unwrap_or("").to_string();
                    if msg_type == "Error" {
                        panic!("ws error: {:?}", v["error_message"]);
                    }
                    if msg_type == "Results" {
                        let is_final = v["is_final"].as_bool().unwrap_or(false);
                        let transcript = v["channel"]["alternatives"][0]["transcript"]
                            .as_str()
                            .unwrap_or("");
                        eprintln!(
                            "[{:>6.1}s] final={} {:?}",
                            t0.elapsed().as_secs_f64(),
                            is_final,
                            &transcript[..transcript.len().min(60)],
                        );
                        if is_final {
                            confirmed_count += 1;
                            if confirmed_count >= target_confirmed {
                                if let Some(tx) = close_tx.take() {
                                    let _ = tx.send(());
                                }
                            }
                        }
                    }
                    let is_terminal = msg_type == "Metadata";
                    packets.push(v);
                    if is_terminal {
                        break;
                    }
                }
                WsMessage::Close(_) => break,
                _ => {}
            }
        }

        let _ = writer.await;
        let _ = shutdown_tx.send(());

        assert!(!packets.is_empty(), "expected at least one packet");

        snapshot_settings().bind(|| {
            insta::assert_json_snapshot!("english_10_packets", &packets);
        });
    });
}
