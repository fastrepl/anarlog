use std::time::Duration;

use anlg_meeting_capture::TranscriptSegment;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, Utf8Bytes},
};

use crate::{
    ClientReady, KeepAliveResponse, MediaHandshake, MediaMessage, SignalingHandshake,
    SignalingMessage, ZoomRtmsCredentials, ZoomRtmsProtocolError, ZoomRtmsStarted,
};

const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_MAX_MESSAGE_BYTES: usize = 256 * 1024;

type ZoomWebSocket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

#[derive(Debug, Clone, Copy)]
pub struct ZoomRtmsSessionConfig {
    pub handshake_timeout: Duration,
    pub max_message_bytes: usize,
}

impl Default for ZoomRtmsSessionConfig {
    fn default() -> Self {
        Self {
            handshake_timeout: DEFAULT_HANDSHAKE_TIMEOUT,
            max_message_bytes: DEFAULT_MAX_MESSAGE_BYTES,
        }
    }
}

impl ZoomRtmsSessionConfig {
    pub fn validate(self) -> Result<Self, ZoomRtmsSessionConfigError> {
        if self.handshake_timeout.is_zero() || self.handshake_timeout > Duration::from_secs(60) {
            return Err(ZoomRtmsSessionConfigError::InvalidHandshakeTimeout);
        }
        if !(1024..=4 * 1024 * 1024).contains(&self.max_message_bytes) {
            return Err(ZoomRtmsSessionConfigError::InvalidMaxMessageBytes);
        }
        Ok(self)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ZoomRtmsSessionConfigError {
    #[error("Zoom RTMS handshake timeout must be between one nanosecond and 60 seconds")]
    InvalidHandshakeTimeout,
    #[error("Zoom RTMS message limit must be between 1 KiB and 4 MiB")]
    InvalidMaxMessageBytes,
}

pub struct ZoomRtmsSession {
    signaling: ZoomWebSocket,
    media: ZoomWebSocket,
    started: ZoomRtmsStarted,
    max_message_bytes: usize,
    next_segment_sequence: u64,
}

impl ZoomRtmsSession {
    pub async fn connect(
        credentials: &ZoomRtmsCredentials,
        started: ZoomRtmsStarted,
        config: ZoomRtmsSessionConfig,
    ) -> Result<Self, ZoomRtmsSessionError> {
        let config = config.validate()?;
        let (mut signaling, _) = connect_async(started.signaling_url.as_str()).await?;
        send_json(
            &mut signaling,
            &SignalingHandshake::new(&started, credentials),
        )
        .await?;
        let transcript_url = tokio::time::timeout(config.handshake_timeout, async {
            loop {
                let text = next_text(&mut signaling, config.max_message_bytes, "signaling").await?;
                match SignalingMessage::parse(&text)? {
                    SignalingMessage::HandshakeAccepted { transcript_url } => {
                        break Ok::<_, ZoomRtmsSessionError>(transcript_url);
                    }
                    SignalingMessage::KeepAlive { timestamp } => {
                        send_json(&mut signaling, &KeepAliveResponse::new(timestamp)).await?;
                    }
                    SignalingMessage::Other { .. } => {}
                }
            }
        })
        .await
        .map_err(|_| ZoomRtmsSessionError::HandshakeTimeout("signaling"))??;

        let (mut media, _) = connect_async(transcript_url.as_str()).await?;
        send_json(
            &mut media,
            &MediaHandshake::transcripts(&started, credentials),
        )
        .await?;
        tokio::time::timeout(config.handshake_timeout, async {
            loop {
                let text = next_text(&mut media, config.max_message_bytes, "media").await?;
                match MediaMessage::parse(&text)? {
                    MediaMessage::HandshakeAccepted => {
                        send_json(&mut signaling, &ClientReady::new(&started.stream_id)).await?;
                        break Ok::<_, ZoomRtmsSessionError>(());
                    }
                    MediaMessage::KeepAlive { timestamp } => {
                        send_json(&mut media, &KeepAliveResponse::new(timestamp)).await?;
                    }
                    MediaMessage::Transcript(_) | MediaMessage::Other { .. } => {}
                }
            }
        })
        .await
        .map_err(|_| ZoomRtmsSessionError::HandshakeTimeout("media"))??;

        Ok(Self {
            signaling,
            media,
            started,
            max_message_bytes: config.max_message_bytes,
            next_segment_sequence: 0,
        })
    }

    pub async fn stream_transcripts(
        &mut self,
        transcripts: mpsc::Sender<TranscriptSegment>,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<ZoomRtmsSessionOutcome, ZoomRtmsSessionError> {
        loop {
            tokio::select! {
                message = next_text(&mut self.signaling, self.max_message_bytes, "signaling") => {
                    let text = message?;
                    if let SignalingMessage::KeepAlive { timestamp } = SignalingMessage::parse(&text)? {
                        send_json(&mut self.signaling, &KeepAliveResponse::new(timestamp)).await?;
                    }
                }
                message = next_text(&mut self.media, self.max_message_bytes, "media") => {
                    let text = message?;
                    match MediaMessage::parse(&text)? {
                        MediaMessage::KeepAlive { timestamp } => {
                            send_json(&mut self.media, &KeepAliveResponse::new(timestamp)).await?;
                        }
                        MediaMessage::Transcript(transcript) => {
                            let sequence = self.next_segment_sequence;
                            self.next_segment_sequence = self.next_segment_sequence
                                .checked_add(1)
                                .ok_or(ZoomRtmsSessionError::SequenceExhausted)?;
                            transcripts
                                .send(transcript.into_segment(self.started.event_timestamp_ms, sequence))
                                .await
                                .map_err(|_| ZoomRtmsSessionError::TranscriptReceiverClosed)?;
                        }
                        MediaMessage::HandshakeAccepted | MediaMessage::Other { .. } => {}
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        self.close().await;
                        return Ok(ZoomRtmsSessionOutcome::StoppedByRequest);
                    }
                }
            }
        }
    }

    async fn close(&mut self) {
        let _ = self.media.close(None).await;
        let _ = self.signaling.close(None).await;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomRtmsSessionOutcome {
    StoppedByRequest,
}

async fn send_json<T: serde::Serialize>(
    socket: &mut ZoomWebSocket,
    value: &T,
) -> Result<(), ZoomRtmsSessionError> {
    let text = serde_json::to_string(value)?;
    socket.send(Message::Text(Utf8Bytes::from(text))).await?;
    Ok(())
}

async fn next_text(
    socket: &mut ZoomWebSocket,
    max_message_bytes: usize,
    scope: &'static str,
) -> Result<String, ZoomRtmsSessionError> {
    loop {
        match socket.next().await {
            Some(Ok(Message::Text(text))) => {
                if text.len() > max_message_bytes {
                    return Err(ZoomRtmsSessionError::MessageTooLarge);
                }
                return Ok(text.to_string());
            }
            Some(Ok(Message::Ping(payload))) => socket.send(Message::Pong(payload)).await?,
            Some(Ok(Message::Pong(_))) => {}
            Some(Ok(Message::Close(_))) | None => {
                return Err(ZoomRtmsSessionError::ConnectionClosed(scope));
            }
            Some(Ok(Message::Binary(_))) => {
                return Err(ZoomRtmsSessionError::UnexpectedBinaryMessage);
            }
            Some(Ok(Message::Frame(_))) => {}
            Some(Err(error)) => return Err(error.into()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ZoomRtmsSessionError {
    #[error(transparent)]
    Config(#[from] ZoomRtmsSessionConfigError),
    #[error(transparent)]
    Websocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Protocol(#[from] ZoomRtmsProtocolError),
    #[error("Zoom RTMS {0} handshake timed out")]
    HandshakeTimeout(&'static str),
    #[error("Zoom RTMS {0} connection closed")]
    ConnectionClosed(&'static str),
    #[error("Zoom RTMS message exceeded the configured size limit")]
    MessageTooLarge,
    #[error("Zoom RTMS transcript connection sent an unexpected binary message")]
    UnexpectedBinaryMessage,
    #[error("Zoom RTMS transcript receiver closed")]
    TranscriptReceiverClosed,
    #[error("Zoom RTMS transcript sequence was exhausted")]
    SequenceExhausted,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_session_limits() {
        assert!(ZoomRtmsSessionConfig::default().validate().is_ok());
        assert!(matches!(
            ZoomRtmsSessionConfig {
                handshake_timeout: Duration::ZERO,
                ..ZoomRtmsSessionConfig::default()
            }
            .validate(),
            Err(ZoomRtmsSessionConfigError::InvalidHandshakeTimeout)
        ));
        assert!(matches!(
            ZoomRtmsSessionConfig {
                max_message_bytes: 512,
                ..ZoomRtmsSessionConfig::default()
            }
            .validate(),
            Err(ZoomRtmsSessionConfigError::InvalidMaxMessageBytes)
        ));
    }
}
