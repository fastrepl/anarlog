use std::{net::IpAddr, time::Duration};

use futures_util::{SinkExt, StreamExt};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Error as WebSocketError, Message},
};
use url::Url;

use crate::{ADMISSION_PROBE_EXPRESSION, AdmissionSnapshot};

const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleMeetUrl(Url);

impl GoogleMeetUrl {
    pub fn parse(value: &str) -> Result<Self, CdpError> {
        let url = Url::parse(value).map_err(CdpError::InvalidMeetingUrl)?;
        let valid_origin = url.scheme() == "https"
            && url.host_str() == Some("meet.google.com")
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none();
        let valid_code = url
            .path_segments()
            .and_then(|mut segments| {
                let code = segments.next()?;
                (segments.next().is_none()).then_some(code)
            })
            .is_some_and(is_google_meet_code);

        if !valid_origin || !valid_code || url.fragment().is_some() {
            return Err(CdpError::UnsupportedMeetingUrl(value.into()));
        }
        Ok(Self(url))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

pub struct CdpPage {
    websocket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    next_command_id: u64,
    command_timeout: Duration,
}

impl CdpPage {
    pub async fn open(
        browser_websocket_url: &str,
        meeting_url: &GoogleMeetUrl,
    ) -> Result<Self, CdpError> {
        let browser_url = validate_loopback_websocket_url(browser_websocket_url)?;
        let browser_port = browser_url
            .port()
            .ok_or_else(|| CdpError::UnsafeDevToolsEndpoint(browser_websocket_url.into()))?;
        let discovery_url = discovery_url(browser_websocket_url, meeting_url)?;
        let client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .map_err(CdpError::DiscoveryClient)?;
        let response = client
            .put(discovery_url)
            .send()
            .await
            .map_err(CdpError::DiscoveryRequest)?;
        if !response.status().is_success() {
            return Err(CdpError::DiscoveryStatus(response.status().as_u16()));
        }
        let target: CdpTarget = response.json().await.map_err(CdpError::DiscoveryResponse)?;
        let page_url =
            validate_target_websocket_url(&target.web_socket_debugger_url, browser_port)?;
        let (websocket, _) = connect_async(page_url.as_str())
            .await
            .map_err(CdpError::WebSocket)?;
        Ok(Self {
            websocket,
            next_command_id: 1,
            command_timeout: DEFAULT_COMMAND_TIMEOUT,
        })
    }

    pub async fn probe_admission(&mut self) -> Result<AdmissionSnapshot, CdpError> {
        self.evaluate(ADMISSION_PROBE_EXPRESSION).await
    }

    pub async fn evaluate<T: DeserializeOwned>(&mut self, expression: &str) -> Result<T, CdpError> {
        let command_id = self.next_command_id;
        self.next_command_id = self.next_command_id.saturating_add(1);
        let command = json!({
            "id": command_id,
            "method": "Runtime.evaluate",
            "params": {
                "expression": expression,
                "returnByValue": true,
                "awaitPromise": true
            }
        });
        self.websocket
            .send(Message::Text(command.to_string().into()))
            .await
            .map_err(CdpError::WebSocket)?;

        let result = tokio::time::timeout(
            self.command_timeout,
            receive_command_result(&mut self.websocket, command_id),
        )
        .await
        .map_err(|_| CdpError::CommandTimeout(self.command_timeout))??;
        let value = result
            .pointer("/result/value")
            .cloned()
            .ok_or(CdpError::MissingEvaluationValue)?;
        serde_json::from_value(value).map_err(CdpError::EvaluationValue)
    }

    pub async fn close(mut self) -> Result<(), CdpError> {
        self.websocket
            .close(None)
            .await
            .map_err(CdpError::WebSocket)
    }

    #[cfg(test)]
    pub(crate) fn from_test_websocket(
        websocket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    ) -> Self {
        Self {
            websocket,
            next_command_id: 1,
            command_timeout: Duration::from_secs(1),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct CdpTarget {
    #[serde(rename = "webSocketDebuggerUrl")]
    web_socket_debugger_url: String,
}

#[derive(Debug, serde::Deserialize)]
struct CdpEnvelope {
    id: Option<u64>,
    result: Option<Value>,
    error: Option<CdpProtocolError>,
}

#[derive(Debug, serde::Deserialize)]
struct CdpProtocolError {
    code: i64,
    message: String,
}

async fn receive_command_result(
    websocket: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    command_id: u64,
) -> Result<Value, CdpError> {
    while let Some(message) = websocket.next().await {
        let message = message.map_err(CdpError::WebSocket)?;
        let Message::Text(message) = message else {
            if matches!(message, Message::Close(_)) {
                return Err(CdpError::ConnectionClosed);
            }
            continue;
        };
        let envelope: CdpEnvelope =
            serde_json::from_str(message.as_ref()).map_err(CdpError::ProtocolMessage)?;
        if envelope.id != Some(command_id) {
            continue;
        }
        if let Some(error) = envelope.error {
            return Err(CdpError::Protocol {
                code: error.code,
                message: error.message,
            });
        }
        let result = envelope.result.ok_or(CdpError::MissingCommandResult)?;
        if result.get("exceptionDetails").is_some() {
            return Err(CdpError::EvaluationException(result));
        }
        return Ok(result);
    }
    Err(CdpError::ConnectionClosed)
}

fn discovery_url(
    browser_websocket_url: &str,
    meeting_url: &GoogleMeetUrl,
) -> Result<Url, CdpError> {
    let browser_url = validate_loopback_websocket_url(browser_websocket_url)?;
    let port = browser_url
        .port()
        .ok_or_else(|| CdpError::UnsafeDevToolsEndpoint(browser_websocket_url.into()))?;
    let encoded_meeting_url: String =
        url::form_urlencoded::byte_serialize(meeting_url.as_str().as_bytes()).collect();
    Url::parse(&format!(
        "http://127.0.0.1:{port}/json/new?{encoded_meeting_url}"
    ))
    .map_err(CdpError::InvalidDiscoveryUrl)
}

fn validate_loopback_websocket_url(value: &str) -> Result<Url, CdpError> {
    let url = Url::parse(value).map_err(CdpError::InvalidDevToolsUrl)?;
    let is_loopback = url
        .host()
        .and_then(|host| match host {
            url::Host::Ipv4(address) => Some(IpAddr::V4(address).is_loopback()),
            url::Host::Ipv6(address) => Some(IpAddr::V6(address).is_loopback()),
            url::Host::Domain(_) => None,
        })
        .unwrap_or(false);
    if url.scheme() != "ws"
        || !is_loopback
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_none()
    {
        return Err(CdpError::UnsafeDevToolsEndpoint(value.into()));
    }
    Ok(url)
}

fn validate_target_websocket_url(value: &str, browser_port: u16) -> Result<Url, CdpError> {
    let url = validate_loopback_websocket_url(value)?;
    if url.port() != Some(browser_port) {
        return Err(CdpError::UnexpectedTargetPort {
            expected: browser_port,
            actual: url.port(),
        });
    }
    Ok(url)
}

fn is_google_meet_code(code: &str) -> bool {
    let mut groups = code.split('-');
    let valid = [3, 4, 3].into_iter().all(|expected_length| {
        groups.next().is_some_and(|group| {
            group.len() == expected_length && group.bytes().all(|byte| byte.is_ascii_lowercase())
        })
    });
    valid && groups.next().is_none()
}

#[derive(Debug, thiserror::Error)]
pub enum CdpError {
    #[error("invalid Google Meet URL")]
    InvalidMeetingUrl(#[source] url::ParseError),
    #[error("only canonical https://meet.google.com/xxx-xxxx-xxx URLs are supported: {0}")]
    UnsupportedMeetingUrl(String),
    #[error("invalid Chromium DevTools URL")]
    InvalidDevToolsUrl(#[source] url::ParseError),
    #[error("Chromium DevTools must use an explicit loopback WebSocket endpoint: {0}")]
    UnsafeDevToolsEndpoint(String),
    #[error("failed to construct Chromium target discovery URL")]
    InvalidDiscoveryUrl(#[source] url::ParseError),
    #[error("failed to ask Chromium to open the meeting")]
    DiscoveryRequest(#[source] reqwest::Error),
    #[error("failed to create the Chromium discovery client")]
    DiscoveryClient(#[source] reqwest::Error),
    #[error("Chromium target discovery returned HTTP {0}")]
    DiscoveryStatus(u16),
    #[error("invalid Chromium target discovery response")]
    DiscoveryResponse(#[source] reqwest::Error),
    #[error("Chromium target WebSocket used port {actual:?}, expected {expected}")]
    UnexpectedTargetPort { expected: u16, actual: Option<u16> },
    #[error("Chromium DevTools WebSocket failed")]
    WebSocket(#[source] WebSocketError),
    #[error("invalid Chromium DevTools protocol message")]
    ProtocolMessage(#[source] serde_json::Error),
    #[error("Chromium DevTools protocol error {code}: {message}")]
    Protocol { code: i64, message: String },
    #[error("Chromium DevTools connection closed")]
    ConnectionClosed,
    #[error("Chromium DevTools command timed out after {0:?}")]
    CommandTimeout(Duration),
    #[error("Chromium DevTools response did not include a result")]
    MissingCommandResult,
    #[error("Chromium JavaScript evaluation failed: {0}")]
    EvaluationException(Value),
    #[error("Chromium JavaScript evaluation returned no value")]
    MissingEvaluationValue,
    #[error("Chromium JavaScript evaluation returned an unexpected value")]
    EvaluationValue(#[source] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    #[test]
    fn accepts_only_canonical_google_meet_urls() {
        assert!(GoogleMeetUrl::parse("https://meet.google.com/abc-defg-hij").is_ok());
        assert!(GoogleMeetUrl::parse("https://meet.google.com/abc-defg-hij?authuser=1").is_ok());

        for value in [
            "http://meet.google.com/abc-defg-hij",
            "https://example.com/abc-defg-hij",
            "https://meet.google.com/not-a-code",
            "https://meet.google.com/abc-defg-hij/extra",
            "https://meet.google.com/abc-defg-hij#fragment",
        ] {
            assert!(GoogleMeetUrl::parse(value).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn target_discovery_is_pinned_to_the_loopback_browser_port() {
        let meeting = GoogleMeetUrl::parse("https://meet.google.com/abc-defg-hij").unwrap();
        let url = discovery_url("ws://127.0.0.1:49231/devtools/browser/test-id", &meeting).unwrap();

        assert_eq!(url.host_str(), Some("127.0.0.1"));
        assert_eq!(url.port(), Some(49231));
        assert_eq!(
            url.as_str(),
            "http://127.0.0.1:49231/json/new?https%3A%2F%2Fmeet.google.com%2Fabc-defg-hij"
        );
    }

    #[test]
    fn rejects_non_loopback_devtools_endpoints() {
        let meeting = GoogleMeetUrl::parse("https://meet.google.com/abc-defg-hij").unwrap();

        for value in [
            "ws://example.com:9222/devtools/browser/id",
            "wss://127.0.0.1:9222/devtools/browser/id",
            "ws://localhost:9222/devtools/browser/id",
            "ws://127.0.0.1/devtools/browser/id",
        ] {
            assert!(discovery_url(value, &meeting).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn target_websocket_cannot_pivot_to_another_loopback_port() {
        assert!(matches!(
            validate_target_websocket_url("ws://127.0.0.1:49232/devtools/page/id", 49231),
            Err(CdpError::UnexpectedTargetPort {
                expected: 49231,
                actual: Some(49232)
            })
        ));
    }

    #[tokio::test]
    async fn evaluates_the_probe_and_ignores_unrelated_devtools_events() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut websocket = accept_async(stream).await.unwrap();
            let Message::Text(command) = websocket.next().await.unwrap().unwrap() else {
                panic!("expected text command")
            };
            let command: Value = serde_json::from_str(command.as_ref()).unwrap();
            assert_eq!(command["method"], "Runtime.evaluate");
            assert!(
                command["params"]["expression"]
                    .as_str()
                    .unwrap()
                    .contains("waiting_room_visible")
            );

            websocket
                .send(Message::Text(
                    json!({"method": "Runtime.consoleAPICalled", "params": {}})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            websocket
                .send(Message::Text(
                    json!({
                        "id": command["id"],
                        "result": {
                            "result": {
                                "type": "object",
                                "value": {
                                    "waiting_room_visible": true,
                                    "consent_prompt_visible": false,
                                    "explicit_denial_indicator": null,
                                    "ambiguous_error_indicator": null,
                                    "visible_recaptcha_challenge": false,
                                    "participant_tile_labels": [],
                                    "self_name_nodes": 0,
                                    "visible_admission_controls": 0
                                }
                            }
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .unwrap();
        });

        let (websocket, _) = connect_async(format!("ws://{address}")).await.unwrap();
        let mut page = CdpPage {
            websocket,
            next_command_id: 1,
            command_timeout: Duration::from_secs(1),
        };

        assert!(page.probe_admission().await.unwrap().waiting_room_visible);
        server.await.unwrap();
    }
}
