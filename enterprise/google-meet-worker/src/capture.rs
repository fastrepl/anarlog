use serde::Deserialize;

use crate::{
    AudioFrame, CaptureProtocol, CaptureProtocolError, CdpBindingEventStream, CdpError, CdpPage,
};

const CAPTURE_BINDING: &str = "anlgCapture";
const CAPTURE_AUDIO_EXPRESSION: &str = include_str!("capture_audio.js");
const STOP_CAPTURE_EXPRESSION: &str =
    "globalThis.__anlgCapture ? globalThis.__anlgCapture.stop().then(() => true) : true";

pub struct BrowserCapture {
    events: CdpBindingEventStream,
    protocol: CaptureProtocol,
    last_warning: Option<CaptureWarning>,
}

impl BrowserCapture {
    pub async fn install(page: &mut CdpPage) -> Result<(Self, usize), BrowserCaptureError> {
        page.ensure_binding_events_available()?;
        page.add_binding(CAPTURE_BINDING).await?;
        let installed: CaptureInstallation = page.evaluate(CAPTURE_AUDIO_EXPRESSION).await?;
        let events = page.take_binding_events()?;
        Ok((
            Self {
                events,
                protocol: CaptureProtocol::default(),
                last_warning: None,
            },
            installed.stream_count,
        ))
    }

    pub async fn next_frame(&mut self) -> Result<AudioFrame, BrowserCaptureError> {
        loop {
            let payload = self.events.next_payload(CAPTURE_BINDING).await?;
            if let Some(frame) = self.decode_payload(&payload)? {
                return Ok(frame);
            }
        }
    }

    pub async fn stop_and_drain(
        &mut self,
        page: &mut CdpPage,
    ) -> Result<Vec<AudioFrame>, BrowserCaptureError> {
        let stopped = page.evaluate::<bool>(STOP_CAPTURE_EXPRESSION).await?;
        if !stopped {
            return Err(BrowserCaptureError::StopRejected);
        }
        let mut frames = Vec::new();
        while let Some(payload) = self.events.try_next_payload(CAPTURE_BINDING)? {
            if let Some(frame) = self.decode_payload(&payload)? {
                frames.push(frame);
            }
        }
        page.close_binding_events().await?;
        Ok(frames)
    }

    pub async fn stop(page: &mut CdpPage) -> Result<(), BrowserCaptureError> {
        let stop_result = page.evaluate::<bool>(STOP_CAPTURE_EXPRESSION).await;
        page.close_binding_events().await?;
        let stopped = stop_result?;
        if !stopped {
            return Err(BrowserCaptureError::StopRejected);
        }
        Ok(())
    }

    pub fn take_last_warning(&mut self) -> Option<CaptureWarning> {
        self.last_warning.take()
    }

    fn decode_payload(&mut self, payload: &str) -> Result<Option<AudioFrame>, BrowserCaptureError> {
        if payload.len() <= 1024
            && let Ok(signal) = serde_json::from_str::<CaptureSignal>(payload)
            && signal.version == 1
        {
            let scope = signal.scope.unwrap_or_else(|| "unknown".into());
            let message = signal.message.unwrap_or_else(|| "unknown failure".into());
            if signal.kind == "warning" {
                self.last_warning = Some(CaptureWarning { scope, message });
                return Ok(None);
            }
            if signal.kind == "error" {
                return Err(BrowserCaptureError::Script { scope, message });
            }
        }
        self.protocol.decode(payload).map(Some).map_err(Into::into)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureWarning {
    pub scope: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptureInstallation {
    stream_count: usize,
}

#[derive(Debug, Deserialize)]
struct CaptureSignal {
    #[serde(rename = "v")]
    version: u8,
    kind: String,
    scope: Option<String>,
    message: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum BrowserCaptureError {
    #[error(transparent)]
    Cdp(#[from] CdpError),
    #[error(transparent)]
    Protocol(#[from] CaptureProtocolError),
    #[error("Google Meet capture script failed in {scope}: {message}")]
    Script { scope: String, message: String },
    #[error("Google Meet capture script rejected stop")]
    StopRejected,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures_util::{SinkExt, StreamExt};
    use serde_json::{Value, json};
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, connect_async, tungstenite::Message};

    use super::*;

    #[test]
    fn shares_one_audio_context_across_participant_streams() {
        assert_eq!(
            CAPTURE_AUDIO_EXPRESSION.matches("new AudioContext").count(),
            1
        );
        assert_eq!(
            CAPTURE_AUDIO_EXPRESSION
                .matches("audioWorklet.addModule")
                .count(),
            1
        );
        assert!(CAPTURE_AUDIO_EXPRESSION.contains("postMessage(\"flush\")"));
        assert!(CAPTURE_AUDIO_EXPRESSION.contains("kind: \"flushed\""));
    }

    #[tokio::test]
    async fn installs_capture_and_decodes_a_binding_frame() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut websocket = accept_async(stream).await.unwrap();

            let binding = next_command(&mut websocket).await;
            assert_eq!(binding["method"], "Runtime.addBinding");
            reply(&mut websocket, &binding, json!({})).await;

            let evaluate = next_command(&mut websocket).await;
            assert_eq!(evaluate["method"], "Runtime.evaluate");
            assert!(
                evaluate["params"]["expression"]
                    .as_str()
                    .unwrap()
                    .contains("AudioWorkletNode")
            );
            reply(
                &mut websocket,
                &evaluate,
                json!({"result": {"value": {"streamCount": 2}}}),
            )
            .await;

            websocket
                .send(Message::Text(
                    json!({
                        "method": "Runtime.bindingCalled",
                        "params": {
                            "name": CAPTURE_BINDING,
                            "payload": json!({
                                "v": 1,
                                "kind": "warning",
                                "scope": "connect_stream",
                                "message": "track disappeared"
                            }).to_string()
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .unwrap();
            websocket
                .send(Message::Text(
                    json!({
                        "method": "Runtime.bindingCalled",
                        "params": {
                            "name": CAPTURE_BINDING,
                            "payload": json!({
                                "v": 1,
                                "kind": "audio",
                                "sequence": 1,
                                "track_index": 2,
                                "sample_rate": 16000,
                                "start_ms": 256,
                                "pcm_s16le": "AQD//w=="
                            }).to_string()
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .unwrap();

            assert!(matches!(
                websocket.next().await,
                Some(Ok(Message::Close(_)))
            ));
        });

        let (websocket, _) = connect_async(format!("ws://{address}")).await.unwrap();
        let mut page = CdpPage::from_test_websocket(websocket);
        let (mut capture, initial_streams) = BrowserCapture::install(&mut page).await.unwrap();

        assert_eq!(initial_streams, 2);
        assert_eq!(capture.next_frame().await.unwrap().samples, vec![1, -1]);
        assert_eq!(
            capture.take_last_warning(),
            Some(CaptureWarning {
                scope: "connect_stream".into(),
                message: "track disappeared".into(),
            })
        );
        page.close().await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn retries_install_after_evaluation_failure_without_losing_events() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut websocket = accept_async(stream).await.unwrap();

            let binding = next_command(&mut websocket).await;
            assert_eq!(binding["method"], "Runtime.addBinding");
            reply(&mut websocket, &binding, json!({})).await;

            let failed_evaluation = next_command(&mut websocket).await;
            reply(
                &mut websocket,
                &failed_evaluation,
                json!({"exceptionDetails": {"text": "capture failed"}}),
            )
            .await;

            let retried_evaluation = next_command(&mut websocket).await;
            assert_eq!(retried_evaluation["method"], "Runtime.evaluate");
            reply(
                &mut websocket,
                &retried_evaluation,
                json!({"result": {"value": {"streamCount": 0}}}),
            )
            .await;

            assert!(matches!(
                websocket.next().await,
                Some(Ok(Message::Close(_)))
            ));
        });

        let (websocket, _) = connect_async(format!("ws://{address}")).await.unwrap();
        let mut page = CdpPage::from_test_websocket(websocket);

        assert!(matches!(
            BrowserCapture::install(&mut page).await,
            Err(BrowserCaptureError::Cdp(CdpError::EvaluationException(_)))
        ));
        let (_, initial_streams) = BrowserCapture::install(&mut page).await.unwrap();
        assert_eq!(initial_streams, 0);

        page.close().await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn stop_unblocks_a_waiting_frame_consumer() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut websocket = accept_async(stream).await.unwrap();

            let binding = next_command(&mut websocket).await;
            reply(&mut websocket, &binding, json!({})).await;
            let install = next_command(&mut websocket).await;
            reply(
                &mut websocket,
                &install,
                json!({"result": {"value": {"streamCount": 0}}}),
            )
            .await;

            let stop = next_command(&mut websocket).await;
            assert_eq!(stop["method"], "Runtime.evaluate");
            assert!(
                stop["params"]["expression"]
                    .as_str()
                    .unwrap()
                    .contains("__anlgCapture.stop")
            );
            reply(&mut websocket, &stop, json!({"result": {"value": true}})).await;

            assert!(matches!(
                websocket.next().await,
                Some(Ok(Message::Close(_)))
            ));
        });

        let (websocket, _) = connect_async(format!("ws://{address}")).await.unwrap();
        let mut page = CdpPage::from_test_websocket(websocket);
        let (mut capture, _) = BrowserCapture::install(&mut page).await.unwrap();

        let (frame_result, stop_result) = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::join!(capture.next_frame(), BrowserCapture::stop(&mut page))
        })
        .await
        .unwrap();
        assert!(matches!(
            frame_result,
            Err(BrowserCaptureError::Cdp(CdpError::BindingEventStreamClosed))
        ));
        stop_result.unwrap();
        assert!(matches!(
            BrowserCapture::install(&mut page).await,
            Err(BrowserCaptureError::Cdp(
                CdpError::BindingEventsAlreadyTaken
            ))
        ));

        page.close().await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn drains_the_final_audio_frame_before_closing_capture_events() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut websocket = accept_async(stream).await.unwrap();

            let binding = next_command(&mut websocket).await;
            reply(&mut websocket, &binding, json!({})).await;
            let install = next_command(&mut websocket).await;
            reply(
                &mut websocket,
                &install,
                json!({"result": {"value": {"streamCount": 1}}}),
            )
            .await;

            let stop = next_command(&mut websocket).await;
            websocket
                .send(Message::Text(
                    json!({
                        "method": "Runtime.bindingCalled",
                        "params": {
                            "name": CAPTURE_BINDING,
                            "payload": json!({
                                "v": 1,
                                "kind": "audio",
                                "sequence": 1,
                                "track_index": 0,
                                "sample_rate": 16000,
                                "start_ms": 0,
                                "pcm_s16le": "AQA="
                            }).to_string()
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .unwrap();
            reply(&mut websocket, &stop, json!({"result": {"value": true}})).await;

            assert!(matches!(
                websocket.next().await,
                Some(Ok(Message::Close(_)))
            ));
        });

        let (websocket, _) = connect_async(format!("ws://{address}")).await.unwrap();
        let mut page = CdpPage::from_test_websocket(websocket);
        let (mut capture, _) = BrowserCapture::install(&mut page).await.unwrap();

        let frames = capture.stop_and_drain(&mut page).await.unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].samples, vec![1]);

        page.close().await.unwrap();
        server.await.unwrap();
    }

    async fn next_command(
        websocket: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    ) -> Value {
        let Message::Text(command) = websocket.next().await.unwrap().unwrap() else {
            panic!("expected text command")
        };
        serde_json::from_str(command.as_ref()).unwrap()
    }

    async fn reply(
        websocket: &mut tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
        command: &Value,
        result: Value,
    ) {
        websocket
            .send(Message::Text(
                json!({"id": command["id"], "result": result})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
    }
}
