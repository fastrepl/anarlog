use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use owhisper_client::Auth;
pub use tokio_tungstenite::tungstenite::ClientRequestBuilder;

use super::handler::WebSocketProxy;
use super::types::{
    ClientMessageFilter, FirstMessageTransformer, InitialMessage, OnCloseCallback,
    ResponseTransformer,
};
use crate::config::DEFAULT_CONNECT_TIMEOUT_MS;
use crate::provider_selector::SelectedProvider;

pub struct NoUpstream;
pub struct WithUrl {
    url: String,
    headers: HashMap<String, String>,
}

pub(crate) trait HasHeaders {
    fn headers_mut(&mut self) -> &mut HashMap<String, String>;
}

impl HasHeaders for WithUrl {
    fn headers_mut(&mut self) -> &mut HashMap<String, String> {
        &mut self.headers
    }
}

pub struct WebSocketProxyBuilder<S = NoUpstream> {
    state: S,
    control_message_types: HashSet<&'static str>,
    transform_first_message: Option<FirstMessageTransformer>,
    initial_message: Option<InitialMessage>,
    response_transformer: Option<ResponseTransformer>,
    connect_timeout: Duration,
    on_close: Option<OnCloseCallback>,
    client_message_filter: Option<ClientMessageFilter>,
}

impl Default for WebSocketProxyBuilder<NoUpstream> {
    fn default() -> Self {
        Self {
            state: NoUpstream,
            control_message_types: HashSet::new(),
            transform_first_message: None,
            initial_message: None,
            response_transformer: None,
            connect_timeout: Duration::from_millis(DEFAULT_CONNECT_TIMEOUT_MS),
            on_close: None,
            client_message_filter: None,
        }
    }
}

impl<S> WebSocketProxyBuilder<S> {
    fn with_state<T>(self, state: T) -> WebSocketProxyBuilder<T> {
        WebSocketProxyBuilder {
            state,
            control_message_types: self.control_message_types,
            transform_first_message: self.transform_first_message,
            initial_message: self.initial_message,
            response_transformer: self.response_transformer,
            connect_timeout: self.connect_timeout,
            on_close: self.on_close,
            client_message_filter: self.client_message_filter,
        }
    }

    fn build_from(
        request: ClientRequestBuilder,
        control_message_types: HashSet<&'static str>,
        transform_first_message: Option<FirstMessageTransformer>,
        initial_message: Option<InitialMessage>,
        response_transformer: Option<ResponseTransformer>,
        connect_timeout: Duration,
        on_close: Option<OnCloseCallback>,
        client_message_filter: Option<ClientMessageFilter>,
    ) -> WebSocketProxy {
        let control_message_types = if control_message_types.is_empty() {
            None
        } else {
            Some(Arc::new(control_message_types))
        };

        WebSocketProxy::new(
            request,
            control_message_types,
            transform_first_message,
            initial_message,
            response_transformer,
            connect_timeout,
            on_close,
            client_message_filter,
        )
    }

    pub fn control_message_types(mut self, types: &[&'static str]) -> Self {
        self.control_message_types = types.iter().copied().collect();
        self
    }

    pub fn transform_first_message<F>(mut self, transformer: F) -> Self
    where
        F: Fn(String) -> String + Send + Sync + 'static,
    {
        self.transform_first_message = Some(Arc::new(transformer));
        self
    }

    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    pub fn initial_message(mut self, message: impl Into<String>) -> Self {
        self.initial_message = Some(Arc::new(message.into()));
        self
    }

    pub fn response_transformer<F>(mut self, transformer: F) -> Self
    where
        F: Fn(&str) -> Option<String> + Send + Sync + 'static,
    {
        self.response_transformer = Some(Arc::new(transformer));
        self
    }

    pub fn on_close<F, Fut>(mut self, callback: F) -> Self
    where
        F: Fn(Duration) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        self.on_close = Some(Arc::new(move |duration| {
            Box::pin(callback(duration))
                as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        }));
        self
    }

    pub fn client_message_filter(mut self, filter: ClientMessageFilter) -> Self {
        self.client_message_filter = Some(filter);
        self
    }
}

impl WebSocketProxyBuilder<NoUpstream> {
    pub fn upstream_url(self, url: impl Into<String>) -> WebSocketProxyBuilder<WithUrl> {
        self.with_state(WithUrl {
            url: url.into(),
            headers: HashMap::new(),
        })
    }
}

#[allow(private_bounds)]
impl<S: HasHeaders> WebSocketProxyBuilder<S> {
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.state.headers_mut().insert(key.into(), value.into());
        self
    }

    pub fn headers(mut self, new_headers: HashMap<String, String>) -> Self {
        self.state.headers_mut().extend(new_headers);
        self
    }

    pub fn apply_auth(self, selected: &SelectedProvider) -> Self {
        let provider = selected.provider();
        let api_key = selected.api_key();

        match provider.auth() {
            Auth::Header { .. } => match provider.build_auth_header(api_key) {
                Some((name, value)) => self.header(name, value),
                None => self,
            },
            Auth::FirstMessage { .. } => {
                let auth = provider.auth();
                let api_key = api_key.to_string();
                self.transform_first_message(move |msg| auth.transform_first_message(msg, &api_key))
            }
            Auth::SessionInit { .. } => self,
        }
    }
}

impl WebSocketProxyBuilder<WithUrl> {
    pub fn build(self) -> Result<WebSocketProxy, crate::ProxyError> {
        let uri = self
            .state
            .url
            .parse()
            .map_err(|e| crate::ProxyError::InvalidRequest(format!("{}", e)))?;

        let mut request = ClientRequestBuilder::new(uri);
        for (key, value) in self.state.headers {
            request = request.with_header(&key, &value);
        }

        Ok(Self::build_from(
            request,
            self.control_message_types,
            self.transform_first_message,
            self.initial_message,
            self.response_transformer,
            self.connect_timeout,
            self.on_close,
            self.client_message_filter,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_builder() {
        let builder = WebSocketProxyBuilder::default();
        assert!(builder.control_message_types.is_empty());
        assert!(builder.transform_first_message.is_none());
        assert_eq!(
            builder.connect_timeout,
            Duration::from_millis(DEFAULT_CONNECT_TIMEOUT_MS)
        );
        assert!(builder.on_close.is_none());
    }

    #[test]
    fn test_upstream_url_transition() {
        let builder = WebSocketProxyBuilder::default().upstream_url("wss://api.example.com/listen");

        assert_eq!(builder.state.url, "wss://api.example.com/listen");
        assert!(builder.state.headers.is_empty());
    }

    #[test]
    fn test_header_with_url() {
        let builder = WebSocketProxyBuilder::default()
            .upstream_url("wss://api.example.com/listen")
            .header("Authorization", "Bearer token123");

        assert_eq!(
            builder.state.headers.get("Authorization"),
            Some(&"Bearer token123".to_string())
        );
    }

    #[test]
    fn test_multiple_headers() {
        let builder = WebSocketProxyBuilder::default()
            .upstream_url("wss://api.example.com/listen")
            .header("Authorization", "Bearer token123")
            .header("X-Custom-Header", "custom-value");

        assert_eq!(builder.state.headers.len(), 2);
        assert_eq!(
            builder.state.headers.get("Authorization"),
            Some(&"Bearer token123".to_string())
        );
        assert_eq!(
            builder.state.headers.get("X-Custom-Header"),
            Some(&"custom-value".to_string())
        );
    }

    #[test]
    fn test_headers_batch() {
        let mut headers = HashMap::new();
        headers.insert("Header1".to_string(), "Value1".to_string());
        headers.insert("Header2".to_string(), "Value2".to_string());

        let builder = WebSocketProxyBuilder::default()
            .upstream_url("wss://api.example.com/listen")
            .headers(headers);

        assert_eq!(builder.state.headers.len(), 2);
        assert_eq!(
            builder.state.headers.get("Header1"),
            Some(&"Value1".to_string())
        );
        assert_eq!(
            builder.state.headers.get("Header2"),
            Some(&"Value2".to_string())
        );
    }

    #[test]
    fn test_headers_extend_existing() {
        let mut headers = HashMap::new();
        headers.insert("Header2".to_string(), "Value2".to_string());

        let builder = WebSocketProxyBuilder::default()
            .upstream_url("wss://api.example.com/listen")
            .header("Header1", "Value1")
            .headers(headers);

        assert_eq!(builder.state.headers.len(), 2);
        assert_eq!(
            builder.state.headers.get("Header1"),
            Some(&"Value1".to_string())
        );
        assert_eq!(
            builder.state.headers.get("Header2"),
            Some(&"Value2".to_string())
        );
    }

    #[test]
    fn test_control_message_types() {
        let builder = WebSocketProxyBuilder::default()
            .control_message_types(&["KeepAlive", "CloseStream"])
            .upstream_url("wss://api.example.com/listen");

        assert_eq!(builder.control_message_types.len(), 2);
        assert!(builder.control_message_types.contains("KeepAlive"));
        assert!(builder.control_message_types.contains("CloseStream"));
    }

    #[test]
    fn test_connect_timeout() {
        let builder = WebSocketProxyBuilder::default()
            .connect_timeout(Duration::from_secs(10))
            .upstream_url("wss://api.example.com/listen");

        assert_eq!(builder.connect_timeout, Duration::from_secs(10));
    }

    #[test]
    fn test_transform_first_message() {
        let builder = WebSocketProxyBuilder::default()
            .transform_first_message(|msg| format!("transformed: {}", msg))
            .upstream_url("wss://api.example.com/listen");

        assert!(builder.transform_first_message.is_some());
        let transformer = builder.transform_first_message.unwrap();
        assert_eq!(transformer("hello".to_string()), "transformed: hello");
    }

    #[test]
    fn test_build_with_url_success() {
        let result = WebSocketProxyBuilder::default()
            .upstream_url("wss://api.example.com/listen")
            .build();

        assert!(result.is_ok());
    }

    #[test]
    fn test_build_with_url_invalid_url() {
        let result = WebSocketProxyBuilder::default()
            .upstream_url("not a valid url ::::")
            .build();

        assert!(result.is_err());
        match result {
            Err(crate::ProxyError::InvalidRequest(_)) => {}
            _ => panic!("expected InvalidRequest error"),
        }
    }

    #[test]
    fn test_chaining_preserves_settings() {
        let builder = WebSocketProxyBuilder::default()
            .control_message_types(&["KeepAlive"])
            .connect_timeout(Duration::from_secs(15))
            .upstream_url("wss://api.example.com/listen")
            .header("Authorization", "Bearer token");

        assert_eq!(builder.control_message_types.len(), 1);
        assert!(builder.control_message_types.contains("KeepAlive"));
        assert_eq!(builder.connect_timeout, Duration::from_secs(15));
        assert_eq!(
            builder.state.headers.get("Authorization"),
            Some(&"Bearer token".to_string())
        );
    }

    #[test]
    fn test_header_overwrites_existing() {
        let builder = WebSocketProxyBuilder::default()
            .upstream_url("wss://api.example.com/listen")
            .header("Authorization", "Bearer old")
            .header("Authorization", "Bearer new");

        assert_eq!(builder.state.headers.len(), 1);
        assert_eq!(
            builder.state.headers.get("Authorization"),
            Some(&"Bearer new".to_string())
        );
    }

    #[test]
    fn test_empty_control_message_types() {
        let builder = WebSocketProxyBuilder::default()
            .control_message_types(&[])
            .upstream_url("wss://api.example.com/listen");

        assert!(builder.control_message_types.is_empty());
    }

    #[test]
    fn test_zero_timeout() {
        let builder = WebSocketProxyBuilder::default()
            .connect_timeout(Duration::ZERO)
            .upstream_url("wss://api.example.com/listen");

        assert_eq!(builder.connect_timeout, Duration::ZERO);
    }

    #[test]
    fn test_headers_with_empty_values() {
        let builder = WebSocketProxyBuilder::default()
            .upstream_url("wss://api.example.com/listen")
            .header("Empty-Header", "");

        assert_eq!(
            builder.state.headers.get("Empty-Header"),
            Some(&"".to_string())
        );
    }

    #[test]
    fn test_headers_with_special_characters() {
        let builder = WebSocketProxyBuilder::default()
            .upstream_url("wss://api.example.com/listen")
            .header("X-Special", "value with spaces and !@#$%");

        assert_eq!(
            builder.state.headers.get("X-Special"),
            Some(&"value with spaces and !@#$%".to_string())
        );
    }

    #[test]
    fn test_url_with_query_params() {
        let result = WebSocketProxyBuilder::default()
            .upstream_url("wss://api.example.com/listen?model=nova-3&encoding=linear16")
            .build();

        assert!(result.is_ok());
    }

    #[test]
    fn test_url_with_port() {
        let result = WebSocketProxyBuilder::default()
            .upstream_url("wss://api.example.com:8080/listen")
            .build();

        assert!(result.is_ok());
    }

    #[test]
    fn test_url_with_path() {
        let result = WebSocketProxyBuilder::default()
            .upstream_url("wss://api.example.com/v1/listen/stream")
            .build();

        assert!(result.is_ok());
    }

    #[test]
    fn test_initial_message() {
        let builder = WebSocketProxyBuilder::default()
            .initial_message(r#"{"api_key":"test"}"#)
            .upstream_url("wss://api.example.com/listen");

        assert!(builder.initial_message.is_some());
        assert_eq!(
            builder.initial_message.as_ref().map(|m| m.as_str()),
            Some(r#"{"api_key":"test"}"#)
        );
    }

    #[test]
    fn test_response_transformer() {
        let builder = WebSocketProxyBuilder::default()
            .response_transformer(|raw| Some(format!("transformed: {}", raw)))
            .upstream_url("wss://api.example.com/listen");

        assert!(builder.response_transformer.is_some());

        let transformer = builder.response_transformer.as_ref().unwrap();
        assert_eq!(transformer("test"), Some("transformed: test".to_string()));
    }

    #[test]
    fn test_response_transformer_returns_none() {
        let builder = WebSocketProxyBuilder::default()
            .response_transformer(|_| None)
            .upstream_url("wss://api.example.com/listen");

        let transformer = builder.response_transformer.as_ref().unwrap();
        assert_eq!(transformer("test"), None);
    }

    #[test]
    fn test_chaining_new_options() {
        let builder = WebSocketProxyBuilder::default()
            .initial_message(r#"{"init":true}"#)
            .response_transformer(|s| Some(s.to_uppercase()))
            .connect_timeout(Duration::from_secs(10))
            .upstream_url("wss://api.example.com/listen");

        assert!(builder.initial_message.is_some());
        assert!(builder.response_transformer.is_some());
        assert_eq!(builder.connect_timeout, Duration::from_secs(10));
    }
}
