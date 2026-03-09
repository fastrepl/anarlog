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
    fn test_is_control_message_empty_types() {
        let types = HashSet::new();
        let data = br#"{"type": "KeepAlive"}"#;
        assert!(!is_control_message(data, &types));
    }

    #[test]
    fn test_is_control_message_matching_type() {
        let mut types = HashSet::new();
        types.insert("KeepAlive");
        types.insert("CloseStream");

        let data = br#"{"type": "KeepAlive"}"#;
        assert!(is_control_message(data, &types));

        let data = br#"{"type": "CloseStream"}"#;
        assert!(is_control_message(data, &types));
    }

    #[test]
    fn test_is_control_message_non_matching_type() {
        let mut types = HashSet::new();
        types.insert("KeepAlive");

        let data = br#"{"type": "DataMessage"}"#;
        assert!(!is_control_message(data, &types));
    }

    #[test]
    fn test_is_control_message_invalid_json() {
        let mut types = HashSet::new();
        types.insert("KeepAlive");

        let data = b"not json";
        assert!(!is_control_message(data, &types));
    }

    #[test]
    fn test_is_control_message_no_type_field() {
        let mut types = HashSet::new();
        types.insert("KeepAlive");

        let data = br#"{"message": "hello"}"#;
        assert!(!is_control_message(data, &types));
    }

    #[test]
    fn test_is_control_message_type_not_string() {
        let mut types = HashSet::new();
        types.insert("KeepAlive");

        let data = br#"{"type": 123}"#;
        assert!(!is_control_message(data, &types));
    }

    #[test]
    fn test_normalize_close_code_valid_codes() {
        assert_eq!(normalize_close_code(1000), 1000);
        assert_eq!(normalize_close_code(1001), 1001);
        assert_eq!(normalize_close_code(1002), 1002);
        assert_eq!(normalize_close_code(1003), 1003);
        assert_eq!(normalize_close_code(4999), 4999);
    }

    #[test]
    fn test_normalize_close_code_reserved_codes() {
        assert_eq!(normalize_close_code(1005), DEFAULT_CLOSE_CODE);
        assert_eq!(normalize_close_code(1006), DEFAULT_CLOSE_CODE);
        assert_eq!(normalize_close_code(1015), DEFAULT_CLOSE_CODE);
    }

    #[test]
    fn test_normalize_close_code_high_codes() {
        assert_eq!(normalize_close_code(5000), DEFAULT_CLOSE_CODE);
        assert_eq!(normalize_close_code(5001), DEFAULT_CLOSE_CODE);
        assert_eq!(normalize_close_code(9999), DEFAULT_CLOSE_CODE);
    }

    #[test]
    fn test_is_control_message_empty_data() {
        let mut types = HashSet::new();
        types.insert("KeepAlive");

        let data = b"";
        assert!(!is_control_message(data, &types));
    }

    #[test]
    fn test_is_control_message_not_starting_with_brace() {
        let mut types = HashSet::new();
        types.insert("KeepAlive");

        let data = br#"["type", "KeepAlive"]"#;
        assert!(!is_control_message(data, &types));

        let data = br#" {"type": "KeepAlive"}"#;
        assert!(!is_control_message(data, &types));
    }

    #[test]
    fn test_is_control_message_nested_type() {
        let mut types = HashSet::new();
        types.insert("KeepAlive");

        let data = br#"{"data": {"type": "KeepAlive"}}"#;
        assert!(!is_control_message(data, &types));
    }

    #[test]
    fn test_is_control_message_type_null() {
        let mut types = HashSet::new();
        types.insert("KeepAlive");

        let data = br#"{"type": null}"#;
        assert!(!is_control_message(data, &types));
    }

    #[test]
    fn test_is_control_message_type_empty_string() {
        let mut types = HashSet::new();
        types.insert("");

        let data = br#"{"type": ""}"#;
        assert!(is_control_message(data, &types));
    }

    #[test]
    fn test_is_control_message_with_extra_fields() {
        let mut types = HashSet::new();
        types.insert("KeepAlive");

        let data = br#"{"type": "KeepAlive", "timestamp": 12345, "data": {"foo": "bar"}}"#;
        assert!(is_control_message(data, &types));
    }

    #[test]
    fn test_normalize_close_code_boundary_values() {
        assert_eq!(normalize_close_code(4999), 4999);
        assert_eq!(normalize_close_code(5000), DEFAULT_CLOSE_CODE);
        assert_eq!(normalize_close_code(0), 0);
        assert_eq!(normalize_close_code(u16::MAX), DEFAULT_CLOSE_CODE);
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
        fn test_extract_axum_close_with_frame() {
            let frame = Some(AxumCloseFrame {
                code: 1000,
                reason: "normal closure".into(),
            });
            let (code, reason) = extract_axum_close(frame, "default");
            assert_eq!(code, 1000);
            assert_eq!(reason, "normal closure");
        }

        #[test]
        fn test_extract_axum_close_without_frame() {
            let (code, reason) = extract_axum_close(None, "client_disconnected");
            assert_eq!(code, DEFAULT_CLOSE_CODE);
            assert_eq!(reason, "client_disconnected");
        }

        #[test]
        fn test_extract_axum_close_normalizes_reserved_code() {
            let frame = Some(AxumCloseFrame {
                code: 1006,
                reason: "abnormal".into(),
            });
            let (code, reason) = extract_axum_close(frame, "default");
            assert_eq!(code, DEFAULT_CLOSE_CODE);
            assert_eq!(reason, "abnormal");
        }

        #[test]
        fn test_extract_axum_close_empty_reason() {
            let frame = Some(AxumCloseFrame {
                code: 1000,
                reason: "".into(),
            });
            let (code, reason) = extract_axum_close(frame, "default");
            assert_eq!(code, 1000);
            assert_eq!(reason, "");
        }

        #[test]
        fn test_extract_tungstenite_close_with_frame() {
            let frame = Some(TungsteniteCloseFrame {
                code: CloseCode::Normal,
                reason: "goodbye".into(),
            });
            let (code, reason) = extract_tungstenite_close(frame, "default");
            assert_eq!(code, 1000);
            assert_eq!(reason, "goodbye");
        }

        #[test]
        fn test_extract_tungstenite_close_without_frame() {
            let (code, reason) = extract_tungstenite_close(None, "upstream_closed");
            assert_eq!(code, DEFAULT_CLOSE_CODE);
            assert_eq!(reason, "upstream_closed");
        }

        #[test]
        fn test_extract_tungstenite_close_normalizes_reserved_code() {
            let frame = Some(TungsteniteCloseFrame {
                code: CloseCode::Abnormal,
                reason: "abnormal".into(),
            });
            let (code, reason) = extract_tungstenite_close(frame, "default");
            assert_eq!(code, DEFAULT_CLOSE_CODE);
            assert_eq!(reason, "abnormal");
        }

        #[test]
        fn test_extract_tungstenite_close_with_custom_code() {
            let frame = Some(TungsteniteCloseFrame {
                code: CloseCode::from(4001),
                reason: "custom error".into(),
            });
            let (code, reason) = extract_tungstenite_close(frame, "default");
            assert_eq!(code, 4001);
            assert_eq!(reason, "custom error");
        }

        #[test]
        fn test_to_axum_close() {
            let msg = to_axum_close(1000, "normal".to_string());
            match msg {
                AxumMessage::Close(Some(frame)) => {
                    assert_eq!(frame.code, 1000);
                    assert_eq!(&*frame.reason, "normal");
                }
                _ => panic!("expected Close message"),
            }
        }

        #[test]
        fn test_to_axum_close_with_custom_code() {
            let msg = to_axum_close(4400, "bad request".to_string());
            match msg {
                AxumMessage::Close(Some(frame)) => {
                    assert_eq!(frame.code, 4400);
                    assert_eq!(&*frame.reason, "bad request");
                }
                _ => panic!("expected Close message"),
            }
        }

        #[test]
        fn test_to_axum_close_empty_reason() {
            let msg = to_axum_close(1000, "".to_string());
            match msg {
                AxumMessage::Close(Some(frame)) => {
                    assert_eq!(frame.code, 1000);
                    assert_eq!(&*frame.reason, "");
                }
                _ => panic!("expected Close message"),
            }
        }

        #[test]
        fn test_to_tungstenite_close() {
            let msg = to_tungstenite_close(1000, "normal".to_string());
            match msg {
                TungsteniteMessage::Close(Some(frame)) => {
                    assert_eq!(u16::from(frame.code), 1000);
                    assert_eq!(&*frame.reason, "normal");
                }
                _ => panic!("expected Close message"),
            }
        }

        #[test]
        fn test_to_tungstenite_close_with_custom_code() {
            let msg = to_tungstenite_close(4429, "rate limited".to_string());
            match msg {
                TungsteniteMessage::Close(Some(frame)) => {
                    assert_eq!(u16::from(frame.code), 4429);
                    assert_eq!(&*frame.reason, "rate limited");
                }
                _ => panic!("expected Close message"),
            }
        }

        #[test]
        fn test_to_tungstenite_close_empty_reason() {
            let msg = to_tungstenite_close(1001, "".to_string());
            match msg {
                TungsteniteMessage::Close(Some(frame)) => {
                    assert_eq!(u16::from(frame.code), 1001);
                    assert_eq!(&*frame.reason, "");
                }
                _ => panic!("expected Close message"),
            }
        }

        #[test]
        fn test_to_tungstenite_close_long_reason() {
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
