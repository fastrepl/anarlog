use std::{fmt::Write as _, time::Duration};

use anlg_meeting_capture::{Speaker, TranscriptSegment};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use url::Url;

pub const SIGNALING_HANDSHAKE_REQUEST: u8 = 1;
pub const SIGNALING_HANDSHAKE_RESPONSE: u8 = 2;
pub const MEDIA_HANDSHAKE_REQUEST: u8 = 3;
pub const MEDIA_HANDSHAKE_RESPONSE: u8 = 4;
pub const CLIENT_READY_ACK: u8 = 7;
pub const KEEP_ALIVE_REQUEST: u8 = 12;
pub const KEEP_ALIVE_RESPONSE: u8 = 13;
pub const TRANSCRIPT_DATA: u8 = 17;
pub const TRANSCRIPT_MEDIA_TYPE: u8 = 8;
pub const RTMS_PROTOCOL_VERSION: u8 = 1;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct ZoomRtmsCredentials {
    client_id: String,
    client_secret: String,
}

impl ZoomRtmsCredentials {
    pub fn new(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Result<Self, ZoomRtmsConfigError> {
        let client_id = client_id.into();
        let client_secret = client_secret.into();
        validate_secret(&client_id).map_err(|()| ZoomRtmsConfigError::InvalidClientId)?;
        validate_secret(&client_secret).map_err(|()| ZoomRtmsConfigError::InvalidClientSecret)?;
        Ok(Self {
            client_id,
            client_secret,
        })
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    pub fn signature(&self, meeting_uuid: &str, stream_id: &str) -> String {
        hmac_hex(
            self.client_secret.as_bytes(),
            format!("{},{meeting_uuid},{stream_id}", self.client_id).as_bytes(),
        )
    }
}

impl std::fmt::Debug for ZoomRtmsCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ZoomRtmsCredentials")
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .finish()
    }
}

fn validate_secret(value: &str) -> Result<(), ()> {
    if value.is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err(());
    }
    Ok(())
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ZoomRtmsConfigError {
    #[error("Zoom RTMS client ID must contain 1-512 non-control characters")]
    InvalidClientId,
    #[error("Zoom RTMS client secret must contain 1-512 non-control characters")]
    InvalidClientSecret,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoomRtmsWebhookEvent {
    UrlValidation { plain_token: String },
    Started(ZoomRtmsStarted),
    Stopped(ZoomRtmsTerminal),
    Interrupted(ZoomRtmsTerminal),
    Other { event: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoomRtmsStarted {
    pub account_id: String,
    pub meeting_uuid: String,
    pub meeting_id: String,
    pub stream_id: String,
    pub signaling_url: Url,
    pub event_timestamp_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoomRtmsTerminal {
    pub meeting_uuid: String,
    pub stream_id: String,
    pub event_timestamp_ms: u64,
}

impl ZoomRtmsWebhookEvent {
    pub fn parse(body: &[u8]) -> Result<Self, ZoomRtmsProtocolError> {
        let webhook: WebhookEnvelope = serde_json::from_slice(body)?;
        match webhook.event.as_str() {
            "endpoint.url_validation" => {
                let payload: UrlValidationPayload = serde_json::from_value(webhook.payload)?;
                validate_token(&payload.plain_token, "URL validation token")?;
                Ok(Self::UrlValidation {
                    plain_token: payload.plain_token,
                })
            }
            "meeting.rtms_started" => {
                let payload: StartedPayload = serde_json::from_value(webhook.payload)?;
                let meeting_id = payload.meeting_id.into_string();
                validate_token(&payload.account_id, "account ID")?;
                validate_token(&payload.meeting_uuid, "meeting UUID")?;
                validate_token(&meeting_id, "meeting ID")?;
                validate_token(&payload.rtms_stream_id, "RTMS stream ID")?;
                let signaling_url = validate_websocket_url(&payload.server_urls)?;
                Ok(Self::Started(ZoomRtmsStarted {
                    account_id: payload.account_id,
                    meeting_uuid: payload.meeting_uuid,
                    meeting_id,
                    stream_id: payload.rtms_stream_id,
                    signaling_url,
                    event_timestamp_ms: webhook.event_ts,
                }))
            }
            "meeting.rtms_stopped" | "meeting.rtms_interrupted" => {
                let payload: TerminalPayload = serde_json::from_value(webhook.payload)?;
                validate_token(&payload.meeting_uuid, "meeting UUID")?;
                validate_token(&payload.rtms_stream_id, "RTMS stream ID")?;
                let terminal = ZoomRtmsTerminal {
                    meeting_uuid: payload.meeting_uuid,
                    stream_id: payload.rtms_stream_id,
                    event_timestamp_ms: webhook.event_ts,
                };
                if webhook.event == "meeting.rtms_stopped" {
                    Ok(Self::Stopped(terminal))
                } else {
                    Ok(Self::Interrupted(terminal))
                }
            }
            _ => Ok(Self::Other {
                event: webhook.event,
            }),
        }
    }
}

#[derive(Deserialize)]
struct WebhookEnvelope {
    event: String,
    event_ts: u64,
    payload: Value,
}

#[derive(Deserialize)]
struct StartedPayload {
    account_id: String,
    meeting_uuid: String,
    meeting_id: StringOrU64,
    rtms_stream_id: String,
    server_urls: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UrlValidationPayload {
    plain_token: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StringOrU64 {
    String(String),
    Number(u64),
}

impl StringOrU64 {
    fn into_string(self) -> String {
        match self {
            Self::String(value) => value,
            Self::Number(value) => value.to_string(),
        }
    }
}

#[derive(Deserialize)]
struct TerminalPayload {
    meeting_uuid: String,
    rtms_stream_id: String,
}

#[derive(Clone)]
pub struct ZoomWebhookVerifier {
    secret: String,
    max_age: Duration,
}

impl ZoomWebhookVerifier {
    pub fn new(
        secret: impl Into<String>,
        max_age: Duration,
    ) -> Result<Self, ZoomWebhookVerificationError> {
        let secret = secret.into();
        if validate_secret(&secret).is_err() {
            return Err(ZoomWebhookVerificationError::InvalidSecret);
        }
        if max_age.is_zero() || max_age > Duration::from_secs(60 * 60) {
            return Err(ZoomWebhookVerificationError::InvalidMaxAge);
        }
        Ok(Self { secret, max_age })
    }

    pub fn verify(
        &self,
        timestamp: &str,
        signature: &str,
        body: &[u8],
        now_unix_seconds: u64,
    ) -> Result<(), ZoomWebhookVerificationError> {
        let timestamp_seconds = timestamp
            .parse::<u64>()
            .map_err(|_| ZoomWebhookVerificationError::InvalidTimestamp)?;
        if timestamp_seconds > now_unix_seconds.saturating_add(60) {
            return Err(ZoomWebhookVerificationError::TimestampInFuture);
        }
        if now_unix_seconds.saturating_sub(timestamp_seconds) > self.max_age.as_secs() {
            return Err(ZoomWebhookVerificationError::StaleTimestamp);
        }
        let provided = signature
            .strip_prefix("v0=")
            .ok_or(ZoomWebhookVerificationError::InvalidSignature)?;
        let provided =
            decode_hex_digest(provided).ok_or(ZoomWebhookVerificationError::InvalidSignature)?;
        let mut mac = HmacSha256::new_from_slice(self.secret.as_bytes())
            .expect("HMAC accepts keys of every size");
        mac.update(format!("v0:{timestamp}:").as_bytes());
        mac.update(body);
        mac.verify_slice(&provided)
            .map_err(|_| ZoomWebhookVerificationError::SignatureMismatch)
    }

    pub fn validation_response(
        &self,
        plain_token: impl Into<String>,
    ) -> Result<ZoomWebhookValidationResponse, ZoomWebhookVerificationError> {
        let plain_token = plain_token.into();
        if validate_secret(&plain_token).is_err() {
            return Err(ZoomWebhookVerificationError::InvalidPlainToken);
        }
        Ok(ZoomWebhookValidationResponse {
            encrypted_token: hmac_hex(self.secret.as_bytes(), plain_token.as_bytes()),
            plain_token,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomWebhookValidationResponse {
    pub plain_token: String,
    pub encrypted_token: String,
}

impl std::fmt::Debug for ZoomWebhookVerifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ZoomWebhookVerifier")
            .field("secret", &"[REDACTED]")
            .field("max_age", &self.max_age)
            .finish()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ZoomWebhookVerificationError {
    #[error("Zoom webhook secret must contain 1-512 non-control characters")]
    InvalidSecret,
    #[error("Zoom webhook maximum age must be between one second and one hour")]
    InvalidMaxAge,
    #[error("Zoom webhook URL validation token must contain 1-512 non-control characters")]
    InvalidPlainToken,
    #[error("Zoom webhook timestamp is invalid")]
    InvalidTimestamp,
    #[error("Zoom webhook timestamp is in the future")]
    TimestampInFuture,
    #[error("Zoom webhook timestamp is stale")]
    StaleTimestamp,
    #[error("Zoom webhook signature is malformed")]
    InvalidSignature,
    #[error("Zoom webhook signature does not match")]
    SignatureMismatch,
}

#[derive(Debug, Serialize)]
pub struct SignalingHandshake<'a> {
    msg_type: u8,
    protocol_version: u8,
    sequence: u64,
    meeting_uuid: &'a str,
    rtms_stream_id: &'a str,
    signature: String,
    buffer_data: bool,
}

impl<'a> SignalingHandshake<'a> {
    pub fn new(started: &'a ZoomRtmsStarted, credentials: &ZoomRtmsCredentials) -> Self {
        Self {
            msg_type: SIGNALING_HANDSHAKE_REQUEST,
            protocol_version: RTMS_PROTOCOL_VERSION,
            sequence: 0,
            meeting_uuid: &started.meeting_uuid,
            rtms_stream_id: &started.stream_id,
            signature: credentials.signature(&started.meeting_uuid, &started.stream_id),
            buffer_data: true,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MediaHandshake<'a> {
    msg_type: u8,
    protocol_version: u8,
    sequence: u64,
    meeting_uuid: &'a str,
    rtms_stream_id: &'a str,
    signature: String,
    media_type: u8,
}

impl<'a> MediaHandshake<'a> {
    pub fn transcripts(started: &'a ZoomRtmsStarted, credentials: &ZoomRtmsCredentials) -> Self {
        Self {
            msg_type: MEDIA_HANDSHAKE_REQUEST,
            protocol_version: RTMS_PROTOCOL_VERSION,
            sequence: 0,
            meeting_uuid: &started.meeting_uuid,
            rtms_stream_id: &started.stream_id,
            signature: credentials.signature(&started.meeting_uuid, &started.stream_id),
            media_type: TRANSCRIPT_MEDIA_TYPE,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SignalingMessage {
    HandshakeAccepted { transcript_url: Url },
    KeepAlive { timestamp: u64 },
    Other { message_type: u64 },
}

impl SignalingMessage {
    pub fn parse(text: &str) -> Result<Self, ZoomRtmsProtocolError> {
        let message: WireMessage = serde_json::from_str(text)?;
        match message.msg_type {
            value if value == u64::from(SIGNALING_HANDSHAKE_RESPONSE) => {
                if message.status_code != Some(0) {
                    return Err(ZoomRtmsProtocolError::HandshakeRejected {
                        scope: "signaling",
                        status_code: message.status_code,
                        reason: message.reason.unwrap_or_default(),
                    });
                }
                let url = message
                    .media_server
                    .and_then(|server| server.server_urls.transcript)
                    .ok_or(ZoomRtmsProtocolError::MissingTranscriptUrl)?;
                Ok(Self::HandshakeAccepted {
                    transcript_url: validate_websocket_url(&url)?,
                })
            }
            value if value == u64::from(KEEP_ALIVE_REQUEST) => Ok(Self::KeepAlive {
                timestamp: message
                    .timestamp
                    .ok_or(ZoomRtmsProtocolError::MissingKeepAliveTimestamp)?,
            }),
            message_type => Ok(Self::Other { message_type }),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum MediaMessage {
    HandshakeAccepted,
    KeepAlive { timestamp: u64 },
    Transcript(ZoomRtmsTranscript),
    Other { message_type: u64 },
}

impl MediaMessage {
    pub fn parse(text: &str) -> Result<Self, ZoomRtmsProtocolError> {
        let message: WireMessage = serde_json::from_str(text)?;
        match message.msg_type {
            value if value == u64::from(MEDIA_HANDSHAKE_RESPONSE) => {
                if message.status_code != Some(0) {
                    return Err(ZoomRtmsProtocolError::HandshakeRejected {
                        scope: "media",
                        status_code: message.status_code,
                        reason: message.reason.unwrap_or_default(),
                    });
                }
                Ok(Self::HandshakeAccepted)
            }
            value if value == u64::from(KEEP_ALIVE_REQUEST) => Ok(Self::KeepAlive {
                timestamp: message
                    .timestamp
                    .ok_or(ZoomRtmsProtocolError::MissingKeepAliveTimestamp)?,
            }),
            value if value == u64::from(TRANSCRIPT_DATA) => {
                let content = message
                    .content
                    .ok_or(ZoomRtmsProtocolError::MissingTranscriptContent)?;
                validate_transcript(&content)?;
                Ok(Self::Transcript(content))
            }
            message_type => Ok(Self::Other { message_type }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ZoomRtmsTranscript {
    pub user_id: u64,
    pub user_name: String,
    pub start_time: u64,
    pub end_time: u64,
    pub timestamp: u64,
    pub language: u16,
    pub data: String,
}

impl ZoomRtmsTranscript {
    pub fn into_segment(self, origin_ms: u64, sequence: u64) -> TranscriptSegment {
        TranscriptSegment {
            id: format!("zoom-rtms-segment-{sequence}"),
            sequence,
            start_ms: self.start_time.saturating_sub(origin_ms),
            end_ms: Some(self.end_time.saturating_sub(origin_ms)),
            text: self.data,
            speaker: Some(Speaker {
                id: format!("zoom-user-{}", self.user_id),
                display_name: Some(self.user_name),
                participant_id: Some(self.user_id.to_string()),
            }),
            is_final: true,
        }
    }
}

#[derive(Deserialize)]
struct WireMessage {
    msg_type: u64,
    status_code: Option<i64>,
    reason: Option<String>,
    timestamp: Option<u64>,
    media_server: Option<WireMediaServer>,
    content: Option<ZoomRtmsTranscript>,
}

#[derive(Deserialize)]
struct WireMediaServer {
    server_urls: WireMediaUrls,
}

#[derive(Deserialize)]
struct WireMediaUrls {
    transcript: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct KeepAliveResponse {
    msg_type: u8,
    timestamp: u64,
}

impl KeepAliveResponse {
    pub fn new(timestamp: u64) -> Self {
        Self {
            msg_type: KEEP_ALIVE_RESPONSE,
            timestamp,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ClientReady<'a> {
    msg_type: u8,
    rtms_stream_id: &'a str,
}

impl<'a> ClientReady<'a> {
    pub fn new(stream_id: &'a str) -> Self {
        Self {
            msg_type: CLIENT_READY_ACK,
            rtms_stream_id: stream_id,
        }
    }
}

fn validate_websocket_url(value: &str) -> Result<Url, ZoomRtmsProtocolError> {
    let url = Url::parse(value).map_err(ZoomRtmsProtocolError::InvalidWebsocketUrl)?;
    if url.scheme() != "wss"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(ZoomRtmsProtocolError::UnsafeWebsocketUrl);
    }
    Ok(url)
}

fn validate_token(value: &str, field: &'static str) -> Result<(), ZoomRtmsProtocolError> {
    if value.is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err(ZoomRtmsProtocolError::InvalidWebhookField(field));
    }
    Ok(())
}

fn validate_transcript(value: &ZoomRtmsTranscript) -> Result<(), ZoomRtmsProtocolError> {
    if value.user_name.is_empty()
        || value.user_name.len() > 1024
        || value.user_name.chars().any(char::is_control)
        || value.data.trim().is_empty()
        || value.data.len() > 64 * 1024
        || value.end_time < value.start_time
    {
        return Err(ZoomRtmsProtocolError::InvalidTranscript);
    }
    Ok(())
}

fn hmac_hex(key: &[u8], message: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts keys of every size");
    mac.update(message);
    let bytes = mac.finalize().into_bytes();
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing to a string cannot fail");
    }
    encoded
}

fn decode_hex_digest(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Some(output)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ZoomRtmsProtocolError {
    #[error("invalid Zoom RTMS JSON message")]
    Json(#[from] serde_json::Error),
    #[error("invalid Zoom RTMS WebSocket URL")]
    InvalidWebsocketUrl(#[source] url::ParseError),
    #[error("Zoom RTMS WebSocket URLs must use wss without credentials or fragments")]
    UnsafeWebsocketUrl,
    #[error("invalid Zoom RTMS webhook field: {0}")]
    InvalidWebhookField(&'static str),
    #[error("Zoom RTMS {scope} handshake was rejected with status {status_code:?}: {reason}")]
    HandshakeRejected {
        scope: &'static str,
        status_code: Option<i64>,
        reason: String,
    },
    #[error("Zoom RTMS signaling handshake omitted the transcript media URL")]
    MissingTranscriptUrl,
    #[error("Zoom RTMS keep-alive request omitted its timestamp")]
    MissingKeepAliveTimestamp,
    #[error("Zoom RTMS transcript message omitted its content")]
    MissingTranscriptContent,
    #[error("Zoom RTMS transcript content is invalid")]
    InvalidTranscript,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started_body(server_url: &str) -> Vec<u8> {
        started_body_with_meeting_id(server_url, serde_json::json!("123456789"))
    }

    fn started_body_with_meeting_id(server_url: &str, meeting_id: Value) -> Vec<u8> {
        serde_json::json!({
            "event": "meeting.rtms_started",
            "event_ts": 1_727_384_100_000_u64,
            "payload": {
                "account_id": "account-id",
                "meeting_uuid": "meeting-uuid",
                "meeting_id": meeting_id,
                "rtms_stream_id": "stream-id",
                "server_urls": server_url
            }
        })
        .to_string()
        .into_bytes()
    }

    #[test]
    fn verifies_webhooks_without_exposing_the_secret() {
        let verifier =
            ZoomWebhookVerifier::new("webhook-secret", Duration::from_secs(300)).unwrap();
        let body = started_body("wss://rtms.zoom.us/signaling");
        let timestamp = "1727384100";
        let signature = format!(
            "v0={}",
            hmac_hex(
                b"webhook-secret",
                [format!("v0:{timestamp}:").as_bytes(), body.as_slice()]
                    .concat()
                    .as_slice()
            )
        );

        verifier
            .verify(timestamp, &signature, &body, 1_727_384_101)
            .unwrap();
        assert!(matches!(
            verifier.verify(
                timestamp,
                "v0=0000000000000000000000000000000000000000000000000000000000000000",
                &body,
                1_727_384_101
            ),
            Err(ZoomWebhookVerificationError::SignatureMismatch)
        ));
        assert!(!format!("{verifier:?}").contains("webhook-secret"));
    }

    #[test]
    fn parses_and_answers_zoom_url_validation_challenges() {
        let body = serde_json::json!({
            "event": "endpoint.url_validation",
            "event_ts": 1_727_384_100_000_u64,
            "payload": { "plainToken": "plain-token" }
        })
        .to_string();
        assert!(matches!(
            ZoomRtmsWebhookEvent::parse(body.as_bytes()).unwrap(),
            ZoomRtmsWebhookEvent::UrlValidation { plain_token } if plain_token == "plain-token"
        ));

        let response = ZoomWebhookVerifier::new("webhook-secret", Duration::from_secs(300))
            .unwrap()
            .validation_response("plain-token")
            .unwrap();
        assert_eq!(response.plain_token, "plain-token");
        assert_eq!(response.encrypted_token.len(), 64);
        assert_eq!(
            serde_json::to_value(response).unwrap()["encryptedToken"]
                .as_str()
                .unwrap()
                .len(),
            64
        );
    }

    #[test]
    fn rejects_stale_webhooks_and_unsafe_server_urls() {
        let verifier = ZoomWebhookVerifier::new("webhook-secret", Duration::from_secs(60)).unwrap();
        assert!(matches!(
            verifier.verify(
                "100",
                "v0=0000000000000000000000000000000000000000000000000000000000000000",
                b"{}",
                1000
            ),
            Err(ZoomWebhookVerificationError::StaleTimestamp)
        ));
        assert!(matches!(
            ZoomRtmsWebhookEvent::parse(&started_body("ws://127.0.0.1/signaling")),
            Err(ZoomRtmsProtocolError::UnsafeWebsocketUrl)
        ));
    }

    #[test]
    fn builds_zoom_handshakes_and_parses_attributed_transcripts() {
        let ZoomRtmsWebhookEvent::Started(started) =
            ZoomRtmsWebhookEvent::parse(&started_body_with_meeting_id(
                "wss://rtms.zoom.us/signaling",
                serde_json::json!(123456789_u64),
            ))
            .unwrap()
        else {
            panic!("expected started event")
        };
        assert_eq!(started.meeting_id, "123456789");
        assert_eq!(started.account_id, "account-id");
        let credentials = ZoomRtmsCredentials::new("client-id", "client-secret").unwrap();
        let signaling =
            serde_json::to_value(SignalingHandshake::new(&started, &credentials)).unwrap();
        assert_eq!(signaling["msg_type"], SIGNALING_HANDSHAKE_REQUEST);
        assert_eq!(signaling["meeting_uuid"], "meeting-uuid");
        assert_eq!(signaling["signature"].as_str().unwrap().len(), 64);

        assert!(matches!(
            SignalingMessage::parse(
                r#"{"msg_type":2,"status_code":0,"media_server":{"server_urls":{"transcript":"wss://rtms.zoom.us/transcript"}}}"#
            )
            .unwrap(),
            SignalingMessage::HandshakeAccepted { .. }
        ));
        let MediaMessage::Transcript(transcript) = MediaMessage::parse(
            r#"{"msg_type":17,"content":{"user_id":19778240,"user_name":"John Smith","start_time":1727384100000,"end_time":1727384310000,"timestamp":1727384349000,"language":9,"data":"Hi, hello world!"}}"#,
        )
        .unwrap()
        else {
            panic!("expected transcript")
        };
        let segment = transcript.into_segment(started.event_timestamp_ms, 4);
        assert_eq!(segment.sequence, 4);
        assert_eq!(segment.start_ms, 0);
        assert_eq!(segment.end_ms, Some(210_000));
        assert_eq!(
            segment.speaker.unwrap().participant_id.as_deref(),
            Some("19778240")
        );
    }
}
