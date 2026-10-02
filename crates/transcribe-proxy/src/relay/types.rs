use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::WebSocket;
use futures_util::stream::{SplitSink, SplitStream};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub const DEFAULT_CLOSE_CODE: u16 = 1011;

pub type OnCloseCallback =
    Arc<dyn Fn(Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;
pub type ControlMessageTypes = Arc<HashSet<&'static str>>;
pub type FirstMessageTransformer = Arc<dyn Fn(String) -> String + Send + Sync>;
pub type InitialMessage = Arc<String>;
pub type ResponseTransformer = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

pub type ClientMessageFilter = Arc<dyn Fn(String) -> Option<String> + Send + Sync>;
pub type ClientBinaryMessageMapper =
    Arc<dyn Fn(Vec<u8>) -> Option<ClientBinaryMessage> + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientBinaryMessage {
    Text(String),
    Binary(Vec<u8>),
}

/// Matches an upstream JSON message by field value (readiness, completion, ...).
/// `field` is a top-level key or a JSON Pointer (leading `/`).
#[derive(Clone, Debug)]
pub struct UpstreamEvent {
    pub field: String,
    pub expected: String,
}

impl UpstreamEvent {
    pub fn new(field: impl Into<String>, expected: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            expected: expected.into(),
        }
    }

    pub fn matches(&self, text: &str) -> bool {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
            return false;
        };
        let actual = if self.field.starts_with('/') {
            value.pointer(&self.field)
        } else {
            value.get(&self.field)
        };
        actual.and_then(|v| v.as_str()) == Some(self.expected.as_str())
    }
}

pub type ReadyNotifier = tokio::sync::watch::Sender<bool>;
pub type ReadyWaiter = tokio::sync::watch::Receiver<bool>;

pub fn ready_channel(readiness: Option<&UpstreamEvent>) -> (ReadyNotifier, ReadyWaiter) {
    tokio::sync::watch::channel(readiness.is_none())
}

pub async fn wait_until_ready(waiter: &mut ReadyWaiter) {
    let _ = waiter.wait_for(|ready| *ready).await;
}

#[derive(Clone, Debug)]
pub enum ShutdownSignal {
    Close { code: u16, reason: String },
    Abort,
}

pub type UpstreamSender = SplitSink<
    WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    tokio_tungstenite::tungstenite::Message,
>;
pub type UpstreamReceiver = SplitStream<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>>;
pub type ClientSender = SplitSink<WebSocket, axum::extract::ws::Message>;
pub type ClientReceiver = SplitStream<WebSocket>;

#[derive(serde::Deserialize)]
struct TypeOnly<'a> {
    #[serde(borrow, rename = "type")]
    msg_type: Option<&'a str>,
}

pub fn is_control_message(data: &[u8], types: &HashSet<&str>) -> bool {
    if types.is_empty() {
        return false;
    }
    if data.first() != Some(&b'{') {
        return false;
    }
    let Ok(parsed) = serde_json::from_slice::<TypeOnly>(data) else {
        return false;
    };
    parsed.msg_type.is_some_and(|t| types.contains(t))
}

pub fn normalize_close_code(code: u16) -> u16 {
    if code == 1005 || code == 1006 || code == 1015 || code >= 5000 {
        DEFAULT_CLOSE_CODE
    } else {
        code
    }
}

pub mod convert {
    use super::{DEFAULT_CLOSE_CODE, normalize_close_code};
    use axum::extract::ws::{CloseFrame as AxumCloseFrame, Message as AxumMessage};
    use tokio_tungstenite::tungstenite::{
        Message as TungsteniteMessage,
        protocol::{CloseFrame as TungsteniteCloseFrame, frame::coding::CloseCode},
    };

    pub fn extract_axum_close(
        frame: Option<AxumCloseFrame>,
        default_reason: &str,
    ) -> (u16, String) {
        match frame {
            Some(f) => (normalize_close_code(f.code), f.reason.to_string()),
            None => (DEFAULT_CLOSE_CODE, default_reason.to_string()),
        }
    }

    pub fn extract_tungstenite_close(
        frame: Option<TungsteniteCloseFrame>,
        default_reason: &str,
    ) -> (u16, String) {
        match frame {
            Some(f) => (normalize_close_code(f.code.into()), f.reason.to_string()),
            None => (DEFAULT_CLOSE_CODE, default_reason.to_string()),
        }
    }

    pub fn to_axum_close(code: u16, reason: String) -> AxumMessage {
        AxumMessage::Close(Some(AxumCloseFrame {
            code,
            reason: reason.into(),
        }))
    }

    pub fn to_tungstenite_close(code: u16, reason: String) -> TungsteniteMessage {
        TungsteniteMessage::Close(Some(TungsteniteCloseFrame {
            code: CloseCode::from(code),
            reason: reason.into(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_readiness_matches_pointer_and_top_level_fields() {
        let nested = UpstreamEvent::new("/header/event", "task-started");
        assert!(nested.matches(r#"{"header":{"event":"task-started"}}"#));
        assert!(!nested.matches(r#"{"header":{"event":"result-generated"}}"#));
        assert!(!nested.matches("not json"));

        let flat = UpstreamEvent::new("type", "session.created");
        assert!(flat.matches(r#"{"type":"session.created"}"#));
        assert!(!flat.matches(r#"{"type":"other"}"#));
    }

    #[test]
    fn is_control_message_matches_only_configured_string_types() {
        let cases: &[(&[u8], &[&str], bool)] = &[
            (br#"{"type": "KeepAlive"}"#, &[], false),
            (
                br#"{"type": "KeepAlive"}"#,
                &["KeepAlive", "CloseStream"],
                true,
            ),
            (
                br#"{"type": "CloseStream"}"#,
                &["KeepAlive", "CloseStream"],
                true,
            ),
            (br#"{"type": "DataMessage"}"#, &["KeepAlive"], false),
            (b"not json", &["KeepAlive"], false),
            (br#"{"message": "hello"}"#, &["KeepAlive"], false),
            (br#"{"type": 123}"#, &["KeepAlive"], false),
            (b"", &["KeepAlive"], false),
            (br#"["type", "KeepAlive"]"#, &["KeepAlive"], false),
            (br#" {"type": "KeepAlive"}"#, &["KeepAlive"], false),
            (br#"{"data": {"type": "KeepAlive"}}"#, &["KeepAlive"], false),
            (br#"{"type": null}"#, &["KeepAlive"], false),
            (br#"{"type": ""}"#, &[""], true),
            (
                br#"{"type": "KeepAlive", "timestamp": 12345, "data": {"foo": "bar"}}"#,
                &["KeepAlive"],
                true,
            ),
        ];

        for (data, configured_types, expected) in cases {
            let types = configured_types.iter().copied().collect();
            assert_eq!(
                is_control_message(data, &types),
                *expected,
                "data: {}",
                String::from_utf8_lossy(data)
            );
        }
    }

    #[test]
    fn normalize_close_code_replaces_reserved_and_out_of_range_codes() {
        for (input, expected) in [
            (1000, 1000),
            (1001, 1001),
            (1002, 1002),
            (1003, 1003),
            (4999, 4999),
            (1005, DEFAULT_CLOSE_CODE),
            (1006, DEFAULT_CLOSE_CODE),
            (1015, DEFAULT_CLOSE_CODE),
            (5000, DEFAULT_CLOSE_CODE),
            (5001, DEFAULT_CLOSE_CODE),
            (9999, DEFAULT_CLOSE_CODE),
            (0, 0),
            (u16::MAX, DEFAULT_CLOSE_CODE),
        ] {
            assert_eq!(normalize_close_code(input), expected, "close code {input}");
        }
    }

    mod convert_tests {
        use super::super::DEFAULT_CLOSE_CODE;
        use super::super::convert::*;
        use axum::extract::ws::{CloseFrame as AxumCloseFrame, Message as AxumMessage};
        use tokio_tungstenite::tungstenite::{
            Message as TungsteniteMessage,
            protocol::{CloseFrame as TungsteniteCloseFrame, frame::coding::CloseCode},
        };

        #[test]
        fn extract_close_normalizes_code_and_defaults_missing_frames() {
            let frame = Some(AxumCloseFrame {
                code: 1000,
                reason: "normal closure".into(),
            });
            let (code, reason) = extract_axum_close(frame, "default");
            assert_eq!(code, 1000);
            assert_eq!(reason, "normal closure");

            let (code, reason) = extract_axum_close(None, "client_disconnected");
            assert_eq!(code, DEFAULT_CLOSE_CODE);
            assert_eq!(reason, "client_disconnected");

            let frame = Some(AxumCloseFrame {
                code: 1006,
                reason: "abnormal".into(),
            });
            let (code, reason) = extract_axum_close(frame, "default");
            assert_eq!(code, DEFAULT_CLOSE_CODE);
            assert_eq!(reason, "abnormal");

            let frame = Some(AxumCloseFrame {
                code: 1000,
                reason: "".into(),
            });
            let (code, reason) = extract_axum_close(frame, "default");
            assert_eq!(code, 1000);
            assert_eq!(reason, "");

            let frame = Some(TungsteniteCloseFrame {
                code: CloseCode::Normal,
                reason: "goodbye".into(),
            });
            let (code, reason) = extract_tungstenite_close(frame, "default");
            assert_eq!(code, 1000);
            assert_eq!(reason, "goodbye");

            let (code, reason) = extract_tungstenite_close(None, "upstream_closed");
            assert_eq!(code, DEFAULT_CLOSE_CODE);
            assert_eq!(reason, "upstream_closed");

            let frame = Some(TungsteniteCloseFrame {
                code: CloseCode::Abnormal,
                reason: "abnormal".into(),
            });
            let (code, reason) = extract_tungstenite_close(frame, "default");
            assert_eq!(code, DEFAULT_CLOSE_CODE);
            assert_eq!(reason, "abnormal");

            let frame = Some(TungsteniteCloseFrame {
                code: CloseCode::from(4001),
                reason: "custom error".into(),
            });
            let (code, reason) = extract_tungstenite_close(frame, "default");
            assert_eq!(code, 4001);
            assert_eq!(reason, "custom error");
        }

        #[test]
        fn to_close_messages_preserve_code_and_reason() {
            let msg = to_axum_close(1000, "normal".to_string());
            match msg {
                AxumMessage::Close(Some(frame)) => {
                    assert_eq!(frame.code, 1000);
                    assert_eq!(&*frame.reason, "normal");
                }
                _ => panic!("expected Close message"),
            }

            let msg = to_axum_close(4400, "bad request".to_string());
            match msg {
                AxumMessage::Close(Some(frame)) => {
                    assert_eq!(frame.code, 4400);
                    assert_eq!(&*frame.reason, "bad request");
                }
                _ => panic!("expected Close message"),
            }

            let msg = to_axum_close(1000, "".to_string());
            match msg {
                AxumMessage::Close(Some(frame)) => {
                    assert_eq!(frame.code, 1000);
                    assert_eq!(&*frame.reason, "");
                }
                _ => panic!("expected Close message"),
            }

            let msg = to_tungstenite_close(1000, "normal".to_string());
            match msg {
                TungsteniteMessage::Close(Some(frame)) => {
                    assert_eq!(u16::from(frame.code), 1000);
                    assert_eq!(&*frame.reason, "normal");
                }
                _ => panic!("expected Close message"),
            }

            let msg = to_tungstenite_close(4429, "rate limited".to_string());
            match msg {
                TungsteniteMessage::Close(Some(frame)) => {
                    assert_eq!(u16::from(frame.code), 4429);
                    assert_eq!(&*frame.reason, "rate limited");
                }
                _ => panic!("expected Close message"),
            }

            let msg = to_tungstenite_close(1001, "".to_string());
            match msg {
                TungsteniteMessage::Close(Some(frame)) => {
                    assert_eq!(u16::from(frame.code), 1001);
                    assert_eq!(&*frame.reason, "");
                }
                _ => panic!("expected Close message"),
            }

            let long_reason = "a".repeat(1000);
            let msg = to_tungstenite_close(1000, long_reason.clone());
            match msg {
                TungsteniteMessage::Close(Some(frame)) => {
                    assert_eq!(u16::from(frame.code), 1000);
                    assert_eq!(&*frame.reason, long_reason.as_str());
                }
                _ => panic!("expected Close message"),
            }
        }
    }
}
