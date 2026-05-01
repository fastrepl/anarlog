mod builder;
mod channel_split;
mod handler;
mod pending;
mod types;
mod upstream_error;

use std::collections::HashMap;
use std::time::Duration;

use axum::body::Body;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::Response;
use owhisper_client::Auth;

pub use builder::ClientRequestBuilder;
pub use handler::WebSocketProxy;
pub use types::{
    ClientMessageFilter, FirstMessageTransformer, InitialMessage, OnCloseCallback,
    ResponseTransformer,
};
pub use upstream_error::{UpstreamError, detect_upstream_error};

use crate::provider_selector::SelectedProvider;
use channel_split::ChannelSplitProxy;

#[derive(Clone, Copy)]
pub enum StreamingTransport {
    Single,
    SplitStereo,
}

impl StreamingTransport {
    pub fn for_channels(
        channels: u8,
        supports_native_multichannel: bool,
    ) -> Result<Self, crate::ProxyError> {
        if channels > 2 {
            return Err(crate::ProxyError::InvalidRequest(
                "channels must be 1 or 2".to_string(),
            ));
        }

        Ok(if channels > 1 && !supports_native_multichannel {
            Self::SplitStereo
        } else {
            Self::Single
        })
    }

    pub fn upstream_count(self) -> usize {
        match self {
            Self::Single => 1,
            Self::SplitStereo => 2,
        }
    }

    pub fn upstream_request_channels(self, channels: u8) -> u8 {
        match self {
            Self::Single => channels,
            Self::SplitStereo => 1,
        }
    }
}

pub struct StreamingProxyPlan {
    transport: StreamingTransport,
    headers: HashMap<String, String>,
    control_message_types: Vec<&'static str>,
    transform_first_message: Option<FirstMessageTransformer>,
    initial_message: Option<InitialMessage>,
    response_transformer: Option<ResponseTransformer>,
    connect_timeout: Duration,
    on_close: Option<OnCloseCallback>,
    client_message_filter: Option<ClientMessageFilter>,
}

pub enum StreamingProxy {
    Single(WebSocketProxy),
    ChannelSplit(ChannelSplitProxy),
}

impl StreamingProxyPlan {
    pub fn new(transport: StreamingTransport) -> Self {
        Self {
            transport,
            headers: HashMap::new(),
            control_message_types: Vec::new(),
            transform_first_message: None,
            initial_message: None,
            response_transformer: None,
            connect_timeout: Duration::default(),
            on_close: None,
            client_message_filter: None,
        }
    }

    pub fn upstream_count(&self) -> usize {
        self.transport.upstream_count()
    }

    pub fn upstream_request_channels(&self, channels: u8) -> u8 {
        self.transport.upstream_request_channels(channels)
    }

    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    pub fn control_message_types(mut self, types: &[&'static str]) -> Self {
        self.control_message_types = types.to_vec();
        self
    }

    pub fn initial_message(mut self, message: impl Into<String>) -> Self {
        self.initial_message = Some(std::sync::Arc::new(message.into()));
        self
    }

    pub fn response_transformer<F>(mut self, transformer: F) -> Self
    where
        F: Fn(&str) -> Option<String> + Send + Sync + 'static,
    {
        self.response_transformer = Some(std::sync::Arc::new(transformer));
        self
    }

    pub fn on_close(mut self, callback: OnCloseCallback) -> Self {
        self.on_close = Some(callback);
        self
    }

    pub fn client_message_filter(mut self, filter: ClientMessageFilter) -> Self {
        self.client_message_filter = Some(filter);
        self
    }

    pub fn apply_auth(mut self, selected: &SelectedProvider) -> Self {
        let provider = selected.provider();
        let api_key = selected.api_key();

        match provider.auth() {
            Auth::Header { .. } => {
                if let Some((name, value)) = provider.build_auth_header(api_key) {
                    self.headers.insert(name.to_string(), value.to_string());
                }
            }
            Auth::FirstMessage { .. } => {
                let auth = provider.auth();
                let api_key = api_key.to_string();
                self.transform_first_message = Some(std::sync::Arc::new(move |msg| {
                    auth.transform_first_message(msg, &api_key)
                }));
            }
            Auth::SessionInit { .. } => {}
        }

        self
    }

    pub fn build_from_upstream_url(
        self,
        upstream_url: &str,
    ) -> Result<StreamingProxy, crate::ProxyError> {
        let request = request_from_url(upstream_url)?;
        Ok(self.build_from_request(request))
    }

    pub fn build_from_upstream_urls(
        self,
        upstream_urls: Vec<String>,
    ) -> Result<StreamingProxy, crate::ProxyError> {
        match self.transport {
            StreamingTransport::Single => {
                let [url] = upstream_urls.try_into().map_err(|_| {
                    crate::ProxyError::InvalidRequest("expected 1 upstream url".to_string())
                })?;
                self.build_from_upstream_url(&url)
            }
            StreamingTransport::SplitStereo => {
                let [mic_url, spk_url] = upstream_urls.try_into().map_err(|_| {
                    crate::ProxyError::InvalidRequest("expected 2 upstream urls".to_string())
                })?;
                let mic_request = request_from_url(&mic_url)?;
                let spk_request = request_from_url(&spk_url)?;
                Ok(self.build_from_requests(mic_request, spk_request))
            }
        }
    }

    pub fn build_from_request(self, request: ClientRequestBuilder) -> StreamingProxy {
        match self.transport {
            StreamingTransport::Single => StreamingProxy::Single(WebSocketProxy::new(
                apply_headers(request, self.headers),
                control_message_types(self.control_message_types),
                self.transform_first_message,
                self.initial_message,
                self.response_transformer,
                self.connect_timeout,
                self.on_close,
                self.client_message_filter,
            )),
            StreamingTransport::SplitStereo => StreamingProxy::split(
                apply_headers(request, self.headers),
                self.initial_message,
                self.response_transformer,
                self.connect_timeout,
                self.on_close,
                self.client_message_filter,
            ),
        }
    }

    pub fn build_from_requests(
        self,
        mic_request: ClientRequestBuilder,
        spk_request: ClientRequestBuilder,
    ) -> StreamingProxy {
        match self.transport {
            StreamingTransport::Single => self.build_from_request(mic_request),
            StreamingTransport::SplitStereo => StreamingProxy::split_with_requests(
                apply_headers(mic_request, self.headers.clone()),
                apply_headers(spk_request, self.headers),
                self.initial_message,
                self.response_transformer,
                self.connect_timeout,
                self.on_close,
                self.client_message_filter,
            ),
        }
    }
}

impl StreamingProxy {
    pub fn single(proxy: WebSocketProxy) -> Self {
        Self::Single(proxy)
    }

    pub fn split(
        upstream_request: ClientRequestBuilder,
        initial_message: Option<InitialMessage>,
        response_transformer: Option<ResponseTransformer>,
        connect_timeout: Duration,
        on_close: Option<OnCloseCallback>,
        client_message_filter: Option<ClientMessageFilter>,
    ) -> Self {
        let proxy = ChannelSplitProxy::new(
            upstream_request,
            initial_message,
            response_transformer,
            connect_timeout,
            on_close,
        );

        match client_message_filter {
            Some(filter) => Self::ChannelSplit(proxy.with_client_message_filter(filter)),
            None => Self::ChannelSplit(proxy),
        }
    }

    pub fn split_with_requests(
        mic_request: ClientRequestBuilder,
        spk_request: ClientRequestBuilder,
        initial_message: Option<InitialMessage>,
        response_transformer: Option<ResponseTransformer>,
        connect_timeout: Duration,
        on_close: Option<OnCloseCallback>,
        client_message_filter: Option<ClientMessageFilter>,
    ) -> Self {
        let proxy = ChannelSplitProxy::with_split_requests(
            mic_request,
            spk_request,
            initial_message,
            response_transformer,
            connect_timeout,
            on_close,
        );

        match client_message_filter {
            Some(filter) => Self::ChannelSplit(proxy.with_client_message_filter(filter)),
            None => Self::ChannelSplit(proxy),
        }
    }

    pub async fn handle_upgrade(&self, ws: WebSocketUpgrade) -> Response<Body> {
        match self {
            Self::Single(proxy) => proxy.handle_upgrade(ws).await,
            Self::ChannelSplit(proxy) => proxy.handle_upgrade(ws).await,
        }
    }
}

fn control_message_types(
    types: Vec<&'static str>,
) -> Option<std::sync::Arc<std::collections::HashSet<&'static str>>> {
    if types.is_empty() {
        None
    } else {
        Some(std::sync::Arc::new(types.into_iter().collect()))
    }
}

fn request_from_url(upstream_url: &str) -> Result<ClientRequestBuilder, crate::ProxyError> {
    let uri = upstream_url
        .parse()
        .map_err(|error| crate::ProxyError::InvalidRequest(format!("{error}")))?;

    Ok(ClientRequestBuilder::new(uri))
}

fn apply_headers(
    mut request: ClientRequestBuilder,
    headers: HashMap<String, String>,
) -> ClientRequestBuilder {
    for (key, value) in headers {
        request = request.with_header(key, value);
    }

    request
}
