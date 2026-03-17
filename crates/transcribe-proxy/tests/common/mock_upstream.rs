use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
    captured_client_messages: Arc<Mutex<Vec<MessageKind>>>,
}

impl MockUpstreamServer {
    async fn with_config(
        recording: WsRecording,
        config: MockUpstreamConfig,
        captured_requests: Arc<Mutex<Vec<String>>>,
        captured_client_messages: Arc<Mutex<Vec<MessageKind>>>,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        Ok(Self {
            recording,
            config,
            listener,
            captured_requests,
            captured_client_messages,
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
        replay_recording(
            ws_stream,
            &self.recording,
            &self.config,
            &self.captured_client_messages,
        )
        .await
    }
}

async fn replay_recording(
    ws_stream: WebSocketStream<TcpStream>,
    recording: &WsRecording,
    config: &MockUpstreamConfig,
    captured_client_messages: &Arc<Mutex<Vec<MessageKind>>>,
) -> Result<(), MockUpstreamError> {
    let (mut sender, mut receiver) = ws_stream.split();
    replay_recording_parts(
        &mut sender,
        &mut receiver,
        recording,
        config,
        captured_client_messages,
    )
    .await
}

async fn replay_recording_parts(
    sender: &mut futures_util::stream::SplitSink<WebSocketStream<TcpStream>, Message>,
    receiver: &mut futures_util::stream::SplitStream<WebSocketStream<TcpStream>>,
    recording: &WsRecording,
    config: &MockUpstreamConfig,
    captured_client_messages: &Arc<Mutex<Vec<MessageKind>>>,
) -> Result<(), MockUpstreamError> {
    let server_messages: Vec<&WsMessage> = recording
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

        if config.use_timing && msg.timestamp_ms > last_timestamp {
            let delay = (msg.timestamp_ms - last_timestamp).min(config.max_delay_ms);
            drain_client_messages(
                receiver,
                captured_client_messages,
                Duration::from_millis(delay),
            )
            .await?;
        }
        last_timestamp = msg.timestamp_ms;

        let ws_msg = ws_message_from_recorded(msg)?;
        let is_close = matches!(msg.kind, MessageKind::Close { .. });

        sender.send(ws_msg).await?;
        msg_index += 1;

        if is_close {
            break;
        }

        drain_client_messages(receiver, captured_client_messages, Duration::from_millis(1)).await?;
    }

    Ok(())
}

async fn drain_client_messages(
    receiver: &mut futures_util::stream::SplitStream<WebSocketStream<TcpStream>>,
    captured_client_messages: &Arc<Mutex<Vec<MessageKind>>>,
    duration: Duration,
) -> Result<(), MockUpstreamError> {
    let deadline = Instant::now() + duration;
    loop {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };

        match tokio::time::timeout(remaining, receiver.next()).await {
            Ok(Some(Ok(message))) => record_client_message(captured_client_messages, &message)?,
            Ok(Some(Err(_))) | Ok(None) | Err(_) => break,
        }
    }

    Ok(())
}

fn record_client_message(
    captured_client_messages: &Arc<Mutex<Vec<MessageKind>>>,
    message: &Message,
) -> Result<(), MockUpstreamError> {
    let kind = match message {
        Message::Text(_) => MessageKind::Text,
        Message::Binary(_) => MessageKind::Binary,
        Message::Close(frame) => {
            let Some(frame) = frame else {
                return Ok(());
            };
            MessageKind::Close {
                code: frame.code.into(),
                reason: frame.reason.to_string(),
            }
        }
        Message::Ping(_) => MessageKind::Ping,
        Message::Pong(_) => MessageKind::Pong,
        Message::Frame(_) => return Ok(()),
    };

    if let Ok(mut messages) = captured_client_messages.lock() {
        messages.push(kind);
    }

    Ok(())
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
    captured_client_messages: Arc<Mutex<Vec<MessageKind>>>,
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

    pub fn captured_client_messages(&self) -> Vec<MessageKind> {
        self.captured_client_messages
            .lock()
            .map(|messages| messages.clone())
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
    let captured_client_messages = Arc::new(Mutex::new(Vec::new()));
    let server = MockUpstreamServer::with_config(
        recording,
        config,
        captured_requests.clone(),
        captured_client_messages.clone(),
    )
    .await?;
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
        captured_client_messages,
        shutdown_tx,
    })
}

pub async fn start_mock_server_group_with_config(
    recordings: Vec<WsRecording>,
    config: MockUpstreamConfig,
) -> std::io::Result<MockServerHandle> {
    let captured_requests = Arc::new(Mutex::new(Vec::new()));
    let captured_client_messages = Arc::new(Mutex::new(Vec::new()));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr().unwrap();
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

    tokio::spawn({
        let captured_requests = captured_requests.clone();
        let captured_client_messages = captured_client_messages.clone();
        async move {
            for recording in recordings {
                tokio::select! {
                    result = listener.accept() => {
                        let Ok((stream, _)) = result else {
                            break;
                        };
                        let captured_requests = captured_requests.clone();
                        let captured_client_messages = captured_client_messages.clone();
                        let config = config.clone();
                        tokio::spawn(async move {
                            let ws_stream = accept_hdr_async(stream, move |req: &Request, resp: Response| {
                                if let Ok(mut requests) = captured_requests.lock() {
                                    requests.push(req.uri().to_string());
                                }
                                Ok(resp)
                            })
                            .await;

                            match ws_stream {
                                Ok(ws_stream) => {
                                    if let Err(e) = replay_recording(
                                        ws_stream,
                                        &recording,
                                        &config,
                                        &captured_client_messages,
                                    )
                                    .await
                                    {
                                        tracing::warn!("mock_server_group_error: {:?}", e);
                                    }
                                }
                                Err(e) => tracing::warn!("mock_server_group_handshake_error: {:?}", e),
                            }
                        });
                    }
                    _ = &mut shutdown_rx => {
                        tracing::debug!("mock_server_group_shutdown");
                        break;
                    }
                }
            }
        }
    });

    tokio::time::sleep(Duration::from_millis(10)).await;

    Ok(MockServerHandle {
        addr,
        captured_requests,
        captured_client_messages,
        shutdown_tx,
    })
}

pub async fn start_split_mock_server_with_config(
    mic_recording: WsRecording,
    spk_recording: WsRecording,
    config: MockUpstreamConfig,
    mic_probe: Vec<u8>,
    spk_probe: Vec<u8>,
) -> std::io::Result<MockServerHandle> {
    let captured_requests = Arc::new(Mutex::new(Vec::new()));
    let captured_client_messages = Arc::new(Mutex::new(Vec::new()));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr().unwrap();
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
    let assignments = Arc::new(Mutex::new(SplitRecordingAssignments {
        mic: Some(mic_recording),
        spk: Some(spk_recording),
    }));

    tokio::spawn({
        let captured_requests = captured_requests.clone();
        let captured_client_messages = captured_client_messages.clone();
        let assignments = assignments.clone();
        async move {
            for _ in 0..2 {
                tokio::select! {
                    result = listener.accept() => {
                        let Ok((stream, _)) = result else {
                            break;
                        };
                        let captured_requests = captured_requests.clone();
                        let captured_client_messages = captured_client_messages.clone();
                        let assignments = assignments.clone();
                        let config = config.clone();
                        let mic_probe = mic_probe.clone();
                        let spk_probe = spk_probe.clone();
                        tokio::spawn(async move {
                            let ws_stream = accept_hdr_async(stream, move |req: &Request, resp: Response| {
                                if let Ok(mut requests) = captured_requests.lock() {
                                    requests.push(req.uri().to_string());
                                }
                                Ok(resp)
                            })
                            .await;

                            match ws_stream {
                                Ok(ws_stream) => {
                                    let (mut sender, mut receiver) = ws_stream.split();
                                    let channel = detect_split_channel(
                                        &mut receiver,
                                        &captured_client_messages,
                                        &mic_probe,
                                        &spk_probe,
                                    )
                                    .await;

                                    let Some(recording) = assignments
                                        .lock()
                                        .ok()
                                        .and_then(|mut assignments| assignments.take(channel))
                                    else {
                                        tracing::warn!("mock_split_server_missing_recording");
                                        return;
                                    };

                                    if let Err(e) = replay_recording_parts(
                                        &mut sender,
                                        &mut receiver,
                                        &recording,
                                        &config,
                                        &captured_client_messages,
                                    )
                                    .await
                                    {
                                        tracing::warn!("mock_split_server_error: {:?}", e);
                                    }
                                }
                                Err(e) => tracing::warn!("mock_split_server_handshake_error: {:?}", e),
                            }
                        });
                    }
                    _ = &mut shutdown_rx => {
                        tracing::debug!("mock_split_server_shutdown");
                        break;
                    }
                }
            }
        }
    });

    tokio::time::sleep(Duration::from_millis(10)).await;

    Ok(MockServerHandle {
        addr,
        captured_requests,
        captured_client_messages,
        shutdown_tx,
    })
}

#[derive(Clone, Copy)]
enum SplitChannel {
    Mic,
    Speaker,
}

async fn detect_split_channel(
    receiver: &mut futures_util::stream::SplitStream<WebSocketStream<TcpStream>>,
    captured_client_messages: &Arc<Mutex<Vec<MessageKind>>>,
    mic_probe: &[u8],
    spk_probe: &[u8],
) -> SplitChannel {
    let deadline = Instant::now() + Duration::from_millis(250);
    loop {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            break;
        };

        match tokio::time::timeout(remaining, receiver.next()).await {
            Ok(Some(Ok(message))) => {
                let _ = record_client_message(captured_client_messages, &message);
                if let Message::Binary(bytes) = message {
                    if bytes.as_ref() == mic_probe {
                        return SplitChannel::Mic;
                    }
                    if bytes.as_ref() == spk_probe {
                        return SplitChannel::Speaker;
                    }
                }
            }
            Ok(Some(Err(_))) | Ok(None) | Err(_) => break,
        }
    }

    SplitChannel::Mic
}

struct SplitRecordingAssignments {
    mic: Option<WsRecording>,
    spk: Option<WsRecording>,
}

impl SplitRecordingAssignments {
    fn take(&mut self, channel: SplitChannel) -> Option<WsRecording> {
        match channel {
            SplitChannel::Mic => self.mic.take().or_else(|| self.spk.take()),
            SplitChannel::Speaker => self.spk.take().or_else(|| self.mic.take()),
        }
    }
}
