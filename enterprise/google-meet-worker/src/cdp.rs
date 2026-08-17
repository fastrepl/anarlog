use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::{
    net::TcpStream,
    sync::{mpsc, mpsc::error::TrySendError, oneshot},
    task::JoinHandle,
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async_with_config,
    tungstenite::{Error as WebSocketError, Message, protocol::WebSocketConfig},
};
use url::Url;

#[cfg(test)]
use tokio_tungstenite::connect_async;

use crate::{ADMISSION_PROBE_EXPRESSION, AdmissionSnapshot};

const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const CDP_EVENT_CAPACITY: usize = 2048;
const CDP_MAX_MESSAGE_BYTES: usize = 256 * 1024;

type PageWebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

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
    commands: mpsc::Sender<CdpRequest>,
    binding_events: Option<mpsc::Receiver<Value>>,
    bindings: HashSet<String>,
    actor: JoinHandle<()>,
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
        let websocket_config = WebSocketConfig::default()
            .max_message_size(Some(CDP_MAX_MESSAGE_BYTES))
            .max_frame_size(Some(CDP_MAX_MESSAGE_BYTES));
        let (websocket, _) =
            connect_async_with_config(page_url.as_str(), Some(websocket_config), false)
                .await
                .map_err(CdpError::WebSocket)?;
        Ok(Self::from_websocket(websocket, DEFAULT_COMMAND_TIMEOUT))
    }

    pub async fn probe_admission(&mut self) -> Result<AdmissionSnapshot, CdpError> {
        self.evaluate(ADMISSION_PROBE_EXPRESSION).await
    }

    pub async fn evaluate<T: DeserializeOwned>(&mut self, expression: &str) -> Result<T, CdpError> {
        let result = self
            .command(
                "Runtime.evaluate",
                json!({
                "expression": expression,
                "returnByValue": true,
                "awaitPromise": true
                }),
            )
            .await?;
        if result.get("exceptionDetails").is_some() {
            return Err(CdpError::EvaluationException(result));
        }
        let value = result
            .pointer("/result/value")
            .cloned()
            .ok_or(CdpError::MissingEvaluationValue)?;
        serde_json::from_value(value).map_err(CdpError::EvaluationValue)
    }

    pub async fn add_binding(&mut self, name: &str) -> Result<(), CdpError> {
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(CdpError::InvalidBindingName(name.into()));
        }
        if self.bindings.contains(name) {
            return Ok(());
        }
        self.command("Runtime.addBinding", json!({ "name": name }))
            .await?;
        self.bindings.insert(name.into());
        Ok(())
    }

    pub fn take_binding_events(&mut self) -> Result<CdpBindingEventStream, CdpError> {
        self.binding_events
            .take()
            .map(|receiver| CdpBindingEventStream { receiver })
            .ok_or(CdpError::BindingEventsAlreadyTaken)
    }

    pub fn ensure_binding_events_available(&self) -> Result<(), CdpError> {
        self.binding_events
            .is_some()
            .then_some(())
            .ok_or(CdpError::BindingEventsAlreadyTaken)
    }

    pub async fn close_binding_events(&mut self) -> Result<(), CdpError> {
        self.binding_events.take();
        let (response, result) = oneshot::channel();
        self.commands
            .send(CdpRequest::CloseBindingEvents { response })
            .await
            .map_err(|_| CdpError::ActorUnavailable)?;
        tokio::time::timeout(self.command_timeout, result)
            .await
            .map_err(|_| CdpError::CommandTimeout(self.command_timeout))?
            .map_err(|_| CdpError::ActorUnavailable)
    }

    pub async fn close(self) -> Result<(), CdpError> {
        let Self {
            commands,
            binding_events,
            bindings: _,
            actor,
            command_timeout,
        } = self;
        drop(binding_events);
        let (response, result) = oneshot::channel();
        commands
            .send(CdpRequest::Close { response })
            .await
            .map_err(|_| CdpError::ActorUnavailable)?;
        tokio::time::timeout(command_timeout, result)
            .await
            .map_err(|_| CdpError::CommandTimeout(command_timeout))?
            .map_err(|_| CdpError::ActorUnavailable)??;
        actor.await.map_err(CdpError::ActorJoin)?;
        Ok(())
    }

    async fn command(&mut self, method: &'static str, params: Value) -> Result<Value, CdpError> {
        let (response, result) = oneshot::channel();
        self.commands
            .send(CdpRequest::Command {
                method,
                params,
                response,
            })
            .await
            .map_err(|_| CdpError::ActorUnavailable)?;
        tokio::time::timeout(self.command_timeout, result)
            .await
            .map_err(|_| CdpError::CommandTimeout(self.command_timeout))?
            .map_err(|_| CdpError::ActorUnavailable)?
            .map_err(Into::into)
    }

    fn from_websocket(websocket: PageWebSocket, command_timeout: Duration) -> Self {
        let (commands, command_receiver) = mpsc::channel(32);
        let (event_sender, binding_events) = mpsc::channel(CDP_EVENT_CAPACITY);
        let actor = tokio::spawn(run_cdp_actor(websocket, command_receiver, event_sender));
        Self {
            commands,
            binding_events: Some(binding_events),
            bindings: HashSet::new(),
            actor,
            command_timeout,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_test_websocket(websocket: PageWebSocket) -> Self {
        Self::from_websocket(websocket, Duration::from_secs(1))
    }
}

pub struct CdpBindingEventStream {
    receiver: mpsc::Receiver<Value>,
}

impl CdpBindingEventStream {
    pub async fn next_payload(&mut self, binding_name: &str) -> Result<String, CdpError> {
        while let Some(event) = self.receiver.recv().await {
            let Some(params) = event.get("params") else {
                continue;
            };
            if params.get("name").and_then(Value::as_str) != Some(binding_name) {
                continue;
            }
            return params
                .get("payload")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or(CdpError::InvalidBindingEvent);
        }
        Err(CdpError::BindingEventStreamClosed)
    }

    pub fn try_next_payload(&mut self, binding_name: &str) -> Result<Option<String>, CdpError> {
        loop {
            let event = match self.receiver.try_recv() {
                Ok(event) => event,
                Err(mpsc::error::TryRecvError::Empty) => return Ok(None),
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    return Err(CdpError::BindingEventStreamClosed);
                }
            };
            let Some(params) = event.get("params") else {
                continue;
            };
            if params.get("name").and_then(Value::as_str) != Some(binding_name) {
                continue;
            }
            return params
                .get("payload")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .map(Some)
                .ok_or(CdpError::InvalidBindingEvent);
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct CdpTarget {
    #[serde(rename = "webSocketDebuggerUrl")]
    web_socket_debugger_url: String,
}

enum CdpRequest {
    Command {
        method: &'static str,
        params: Value,
        response: oneshot::Sender<Result<Value, CdpActorError>>,
    },
    Close {
        response: oneshot::Sender<Result<(), CdpActorError>>,
    },
    CloseBindingEvents {
        response: oneshot::Sender<()>,
    },
}

async fn run_cdp_actor(
    mut websocket: PageWebSocket,
    mut commands: mpsc::Receiver<CdpRequest>,
    binding_events: mpsc::Sender<Value>,
) {
    let mut binding_events = Some(binding_events);
    let mut next_command_id = 1_u64;
    let mut pending = HashMap::<u64, oneshot::Sender<Result<Value, CdpActorError>>>::new();
    loop {
        tokio::select! {
            request = commands.recv() => {
                match request {
                    Some(CdpRequest::Command { method, params, response }) => {
                        let command_id = next_command_id;
                        next_command_id = next_command_id.saturating_add(1);
                        let command = json!({ "id": command_id, "method": method, "params": params });
                        if let Err(error) = websocket.send(Message::Text(command.to_string().into())).await {
                            let error = CdpActorError::WebSocket(error.to_string());
                            let _ = response.send(Err(error.clone()));
                            fail_pending(&mut pending, error);
                            return;
                        }
                        pending.insert(command_id, response);
                    }
                    Some(CdpRequest::Close { response }) => {
                        let result = websocket
                            .close(None)
                            .await
                            .map_err(|error| CdpActorError::WebSocket(error.to_string()));
                        let _ = response.send(result);
                        fail_pending(&mut pending, CdpActorError::ConnectionClosed);
                        return;
                    }
                    Some(CdpRequest::CloseBindingEvents { response }) => {
                        binding_events.take();
                        let _ = response.send(());
                    }
                    None => {
                        let _ = websocket.close(None).await;
                        fail_pending(&mut pending, CdpActorError::ConnectionClosed);
                        return;
                    }
                }
            }
            message = websocket.next() => {
                let value = match message {
                    Some(Ok(Message::Text(message))) => match serde_json::from_str::<Value>(message.as_ref()) {
                        Ok(value) => value,
                        Err(error) => {
                            fail_pending(&mut pending, CdpActorError::InvalidMessage(error.to_string()));
                            return;
                        }
                    },
                    Some(Ok(Message::Close(_))) | None => {
                        fail_pending(&mut pending, CdpActorError::ConnectionClosed);
                        return;
                    }
                    Some(Ok(_)) => continue,
                    Some(Err(error)) => {
                        fail_pending(&mut pending, CdpActorError::WebSocket(error.to_string()));
                        return;
                    }
                };
                if let Some(command_id) = value.get("id").and_then(Value::as_u64) {
                    let Some(response) = pending.remove(&command_id) else {
                        continue;
                    };
                    let result = if let Some(error) = value.get("error") {
                        Err(CdpActorError::Protocol {
                            code: error.get("code").and_then(Value::as_i64).unwrap_or_default(),
                            message: error
                                .get("message")
                                .and_then(Value::as_str)
                                .unwrap_or("unknown protocol error")
                                .to_owned(),
                        })
                    } else {
                        value
                            .get("result")
                            .cloned()
                            .ok_or(CdpActorError::MissingCommandResult)
                    };
                    let _ = response.send(result);
                } else if value.get("method").and_then(Value::as_str)
                    == Some("Runtime.bindingCalled")
                    && let Some(binding_events) = binding_events.as_ref()
                {
                    match binding_events.try_send(value) {
                        Ok(()) | Err(TrySendError::Closed(_)) => {}
                        Err(TrySendError::Full(_)) => {
                            fail_pending(&mut pending, CdpActorError::BindingEventOverflow);
                            return;
                        }
                    }
                }
            }
        }
    }
}

fn fail_pending(
    pending: &mut HashMap<u64, oneshot::Sender<Result<Value, CdpActorError>>>,
    error: CdpActorError,
) {
    for (_, response) in pending.drain() {
        let _ = response.send(Err(error.clone()));
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum CdpActorError {
    #[error("Chromium DevTools WebSocket failed: {0}")]
    WebSocket(String),
    #[error("invalid Chromium DevTools protocol message: {0}")]
    InvalidMessage(String),
    #[error("Chromium DevTools protocol error {code}: {message}")]
    Protocol { code: i64, message: String },
    #[error("Chromium DevTools response did not include a result")]
    MissingCommandResult,
    #[error("Chromium DevTools connection closed")]
    ConnectionClosed,
    #[error("Chromium DevTools binding event buffer overflowed")]
    BindingEventOverflow,
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
    #[error(transparent)]
    Actor(#[from] CdpActorError),
    #[error("Chromium DevTools actor is unavailable")]
    ActorUnavailable,
    #[error("Chromium DevTools actor task failed")]
    ActorJoin(#[source] tokio::task::JoinError),
    #[error("Chromium DevTools command timed out after {0:?}")]
    CommandTimeout(Duration),
    #[error("Chromium JavaScript evaluation failed: {0}")]
    EvaluationException(Value),
    #[error("Chromium JavaScript evaluation returned no value")]
    MissingEvaluationValue,
    #[error("Chromium JavaScript evaluation returned an unexpected value")]
    EvaluationValue(#[source] serde_json::Error),
    #[error("Chromium binding name must contain only ASCII letters, numbers, and underscores: {0}")]
    InvalidBindingName(String),
    #[error("Chromium binding events were already taken")]
    BindingEventsAlreadyTaken,
    #[error("invalid Chromium binding event")]
    InvalidBindingEvent,
    #[error("Chromium binding event stream closed")]
    BindingEventStreamClosed,
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
        let mut page = CdpPage::from_test_websocket(websocket);

        assert!(page.probe_admission().await.unwrap().waiting_room_visible);
        server.await.unwrap();
    }

    #[tokio::test]
    async fn routes_binding_events_without_blocking_commands() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut websocket = accept_async(stream).await.unwrap();
            let Message::Text(command) = websocket.next().await.unwrap().unwrap() else {
                panic!("expected text command")
            };
            let command: Value = serde_json::from_str(command.as_ref()).unwrap();
            assert_eq!(command["method"], "Runtime.addBinding");
            websocket
                .send(Message::Text(
                    json!({"id": command["id"], "result": {}})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            websocket
                .send(Message::Text(
                    json!({
                        "method": "Runtime.bindingCalled",
                        "params": {"name": "anlgCapture", "payload": "frame-1"}
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
        page.add_binding("anlgCapture").await.unwrap();
        page.add_binding("anlgCapture").await.unwrap();
        let mut events = page.take_binding_events().unwrap();

        assert_eq!(events.next_payload("anlgCapture").await.unwrap(), "frame-1");
        page.close().await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn rejects_script_like_binding_names_before_devtools() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut websocket = accept_async(stream).await.unwrap();
            assert!(matches!(
                websocket.next().await,
                Some(Ok(Message::Close(_)))
            ));
        });
        let (websocket, _) = connect_async(format!("ws://{address}")).await.unwrap();
        let mut page = CdpPage::from_test_websocket(websocket);

        assert!(matches!(
            page.add_binding("capture);alert(1)").await,
            Err(CdpError::InvalidBindingName(_))
        ));
        page.close().await.unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn dropping_the_binding_consumer_keeps_page_commands_available() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut websocket = accept_async(stream).await.unwrap();

            let Message::Text(binding) = websocket.next().await.unwrap().unwrap() else {
                panic!("expected binding command")
            };
            let binding: Value = serde_json::from_str(binding.as_ref()).unwrap();
            websocket
                .send(Message::Text(
                    json!({"id": binding["id"], "result": {}})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            websocket
                .send(Message::Text(
                    json!({
                        "method": "Runtime.bindingCalled",
                        "params": {"name": "anlgCapture", "payload": "ignored"}
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .unwrap();

            let Message::Text(evaluate) = websocket.next().await.unwrap().unwrap() else {
                panic!("expected evaluate command")
            };
            let evaluate: Value = serde_json::from_str(evaluate.as_ref()).unwrap();
            websocket
                .send(Message::Text(
                    json!({
                        "id": evaluate["id"],
                        "result": {"result": {"value": {
                            "waiting_room_visible": false,
                            "consent_prompt_visible": false,
                            "explicit_denial_indicator": null,
                            "ambiguous_error_indicator": null,
                            "visible_recaptcha_challenge": false,
                            "participant_tile_labels": [],
                            "self_name_nodes": 0,
                            "visible_admission_controls": 0
                        }}}
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
        page.add_binding("anlgCapture").await.unwrap();
        drop(page.take_binding_events().unwrap());

        assert_eq!(
            page.probe_admission().await.unwrap(),
            AdmissionSnapshot::default()
        );
        page.close().await.unwrap();
        server.await.unwrap();
    }
}
