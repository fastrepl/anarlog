use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use owhisper_client::Auth;
pub use tokio_tungstenite::tungstenite::ClientRequestBuilder;

use super::handler::WebSocketProxy;
use super::types::{
    ClientBinaryMessageMapper, ClientMessageFilter, FirstMessageTransformer, InitialMessage,
    OnCloseCallback, ResponseTransformer,
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
    client_binary_message_mapper: Option<ClientBinaryMessageMapper>,
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
            client_binary_message_mapper: None,
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
            client_binary_message_mapper: self.client_binary_message_mapper,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_from(
        request: ClientRequestBuilder,
        control_message_types: HashSet<&'static str>,
        transform_first_message: Option<FirstMessageTransformer>,
        initial_message: Option<InitialMessage>,
        response_transformer: Option<ResponseTransformer>,
        connect_timeout: Duration,
        on_close: Option<OnCloseCallback>,
        client_message_filter: Option<ClientMessageFilter>,
        client_binary_message_mapper: Option<ClientBinaryMessageMapper>,
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
            client_binary_message_mapper,
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

    pub fn client_binary_message_mapper(mut self, mapper: ClientBinaryMessageMapper) -> Self {
        self.client_binary_message_mapper = Some(mapper);
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
            self.client_binary_message_mapper,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_validates_upstream_url() {
        assert!(
            WebSocketProxyBuilder::default()
                .upstream_url("wss://api.example.com/listen")
                .build()
                .is_ok()
        );

        let invalid = WebSocketProxyBuilder::default()
            .upstream_url("not a valid url ::::")
            .build();
        assert!(invalid.is_err());
        match invalid {
            Err(crate::ProxyError::InvalidRequest(_)) => {}
            _ => panic!("expected InvalidRequest error"),
        }

        assert!(
            WebSocketProxyBuilder::default()
                .upstream_url(
                    "wss://api.example.com:8080/v1/listen/stream?model=nova-3&encoding=linear16"
                )
                .build()
                .is_ok()
        );
    }

    #[test]
    fn headers_merge_and_later_values_override() {
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
}
