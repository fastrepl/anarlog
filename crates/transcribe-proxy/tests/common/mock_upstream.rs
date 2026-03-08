use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::{WebSocketStream, accept_hdr_async};

use super::recording::{MessageKind, WsMessage, WsRecording};

#[derive(Debug, Clone)]
pub struct MockUpstreamConfig {
    pub use_timing: bool,
    pub max_delay_ms: u64,
}

impl Default for MockUpstreamConfig {
    fn default() -> Self {
        Self {
            use_timing: false,
            max_delay_ms: 1000,
        }
    }
}

impl MockUpstreamConfig {
    pub fn use_timing(mut self, use_timing: bool) -> Self {
        self.use_timing = use_timing;
        self
    }

    pub fn max_delay_ms(mut self, max_delay_ms: u64) -> Self {
        self.max_delay_ms = max_delay_ms;
        self
    }
}

struct MockUpstreamServer {
    recording: WsRecording,
    config: MockUpstreamConfig,
    listener: TcpListener,
    captured_requests: Arc<Mutex<Vec<String>>>,
}

impl MockUpstreamServer {
    async fn with_config(
        recording: WsRecording,
        config: MockUpstreamConfig,
        captured_requests: Arc<Mutex<Vec<String>>>,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        Ok(Self {
            recording,
            config,
            listener,
            captured_requests,
        })
    }

    fn addr(&self) -> SocketAddr {
        self.listener.local_addr().unwrap()
    }

    async fn accept_one(&self) -> Result<(), MockUpstreamError> {
        let (stream, _) = self.listener.accept().await?;
        let captured_requests = self.captured_requests.clone();
        let ws_stream = accept_hdr_async(stream, move |req: &Request, resp: Response| {
            if let Ok(mut requests) = captured_requests.lock() {
                requests.push(req.uri().to_string());
            }
            Ok(resp)
        })
        .await?;
        self.handle_connection(ws_stream).await
    }

    async fn handle_connection(
        &self,
        ws_stream: WebSocketStream<TcpStream>,
    ) -> Result<(), MockUpstreamError> {
        let (mut sender, mut receiver) = ws_stream.split();

        let server_messages: Vec<&WsMessage> = self
            .recording
            .messages
            .iter()
            .filter(|m| m.is_from_upstream())
            .collect();

        let mut last_timestamp = 0u64;
        let mut msg_index = 0;

        loop {
            if msg_index >= server_messages.len() {
                break;
            }

            let msg = server_messages[msg_index];

            if self.config.use_timing && msg.timestamp_ms > last_timestamp {
                let delay = (msg.timestamp_ms - last_timestamp).min(self.config.max_delay_ms);
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            last_timestamp = msg.timestamp_ms;

            let ws_msg = ws_message_from_recorded(msg)?;
            let is_close = matches!(msg.kind, MessageKind::Close { .. });

            sender.send(ws_msg).await?;
            msg_index += 1;

            if is_close {
                break;
            }

            while let Ok(Some(_)) =
                tokio::time::timeout(Duration::from_millis(1), receiver.next()).await
            {}
        }

        Ok(())
    }
}

fn ws_message_from_recorded(msg: &WsMessage) -> Result<Message, MockUpstreamError> {
    match &msg.kind {
        MessageKind::Text => Ok(Message::Text(msg.content.clone().into())),
        MessageKind::Binary => {
            let data = msg.decode_binary()?;
            Ok(Message::Binary(data.into()))
        }
        MessageKind::Close { code, reason } => Ok(Message::Close(Some(CloseFrame {
            code: CloseCode::from(*code),
            reason: reason.clone().into(),
        }))),
        MessageKind::Ping => {
            let data = if msg.content.is_empty() {
                vec![]
            } else {
                msg.decode_binary()?
            };
            Ok(Message::Ping(data.into()))
        }
        MessageKind::Pong => {
            let data = if msg.content.is_empty() {
                vec![]
            } else {
                msg.decode_binary()?
            };
            Ok(Message::Pong(data.into()))
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MockUpstreamError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
}

pub struct MockServerHandle {
    addr: SocketAddr,
    captured_requests: Arc<Mutex<Vec<String>>>,
    #[allow(dead_code)]
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl MockServerHandle {
    pub fn ws_url(&self) -> String {
        format!("ws://{}", self.addr)
    }

    pub fn captured_requests(&self) -> Vec<String> {
        self.captured_requests
            .lock()
            .map(|requests| requests.clone())
            .unwrap_or_default()
    }
}

/// Starts a mock upstream server that replays recorded WebSocket messages.
///
/// Note: This server only accepts a single connection. After one client connects
/// and the recording is replayed, the server will shut down. This is intentional
/// for test isolation - each test should create its own mock server instance.
pub async fn start_mock_server_with_config(
    recording: WsRecording,
    config: MockUpstreamConfig,
) -> std::io::Result<MockServerHandle> {
    let captured_requests = Arc::new(Mutex::new(Vec::new()));
    let server =
        MockUpstreamServer::with_config(recording, config, captured_requests.clone()).await?;
    let addr = server.addr();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        tokio::select! {
            result = server.accept_one() => {
                if let Err(e) = result {
                    tracing::warn!("mock_server_error: {:?}", e);
                }
            }
            _ = shutdown_rx => {
                tracing::debug!("mock_server_shutdown");
            }
        }
    });

    tokio::time::sleep(Duration::from_millis(10)).await;

    Ok(MockServerHandle {
        addr,
        captured_requests,
        shutdown_tx,
    })
}
