mod adapter;
mod batch;
mod error;
mod error_detection;
mod http_client;
mod live;
pub(crate) mod polling;
mod providers;

#[cfg(test)]
pub(crate) mod test_utils;

pub use error_detection::ProviderError;
pub use providers::{Auth, Provider, is_meta_model};

use std::marker::PhantomData;

#[cfg(feature = "argmax")]
pub use adapter::StreamingBatchConfig;
pub use adapter::deepgram::DeepgramModel;
pub use adapter::{
    AdapterKind, ArgmaxAdapter, AssemblyAIAdapter, BatchSttAdapter, CactusAdapter, CallbackResult,
    CallbackSttAdapter, DashScopeAdapter, DeepgramAdapter, ElevenLabsAdapter, FireworksAdapter,
    GladiaAdapter, HyprnoteAdapter, LanguageQuality, LanguageSupport, MistralAdapter,
    OpenAIAdapter, RealtimeSttAdapter, SonioxAdapter, append_provider_param,
    documented_language_codes_batch, documented_language_codes_live, is_hyprnote_proxy,
    is_local_host, normalize_languages,
};
pub use adapter::{StreamingBatchEvent, StreamingBatchStream};

pub use batch::{BatchClient, BatchClientBuilder};
pub use error::Error;
pub use hypr_ws_client;
pub use live::{DualHandle, FinalizeHandle, ListenClient, ListenClientDual};

pub struct ListenClientBuilder<A: RealtimeSttAdapter = DeepgramAdapter> {
    api_base: Option<String>,
    api_key: Option<String>,
    params: Option<owhisper_interface::ListenParams>,
    extra_headers: Vec<(String, String)>,
    connect_policy: Option<hypr_ws_client::client::WebSocketConnectPolicy>,
    _marker: PhantomData<A>,
}

impl Default for ListenClientBuilder {
    fn default() -> Self {
        Self {
            api_base: None,
            api_key: None,
            params: None,
            extra_headers: Vec::new(),
            connect_policy: None,
            _marker: PhantomData,
        }
    }
}

impl<A: RealtimeSttAdapter> ListenClientBuilder<A> {
    pub fn api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = Some(api_base.into());
        self
    }

    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn params(mut self, params: owhisper_interface::ListenParams) -> Self {
        self.params = Some(params);
        self
    }

    pub fn extra_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_headers.push((name.into(), value.into()));
        self
    }

    pub fn connect_policy(
        mut self,
        policy: hypr_ws_client::client::WebSocketConnectPolicy,
    ) -> Self {
        self.connect_policy = Some(policy);
        self
    }

    pub fn adapter<B: RealtimeSttAdapter>(self) -> ListenClientBuilder<B> {
        ListenClientBuilder {
            api_base: self.api_base,
            api_key: self.api_key,
            params: self.params,
            extra_headers: self.extra_headers,
            connect_policy: self.connect_policy,
            _marker: PhantomData,
        }
    }

    fn get_api_base(&self) -> &str {
        self.api_base.as_ref().expect("api_base is required")
    }

    fn get_params(&self) -> owhisper_interface::ListenParams {
        let mut params = self.params.clone().unwrap_or_default();
        params.languages = adapter::normalize_languages(&params.languages);
        params
    }

    async fn build_request(
        &self,
        adapter: &A,
        channels: u8,
    ) -> hypr_ws_client::client::ClientRequestBuilder {
        let params = self.get_params();
        let original_api_base = self.get_api_base();
        let api_base = append_provider_param(original_api_base, adapter.provider_name());
        let url = adapter
            .build_ws_url_with_api_key(&api_base, &params, channels, self.api_key.as_deref())
            .await
            .unwrap_or_else(|| adapter.build_ws_url(&api_base, &params, channels));
        let uri = url.to_string().parse().unwrap();

        let mut request = hypr_ws_client::client::ClientRequestBuilder::new(uri);

        if is_hyprnote_proxy(original_api_base) {
            if let Some(api_key) = self.api_key.as_deref() {
                request = request.with_header("Authorization", format!("Bearer {}", api_key));
            }
            for (name, value) in &self.extra_headers {
                request = request.with_header(name, value);
            }
        } else if let Some((header_name, header_value)) =
            adapter.build_auth_header(self.api_key.as_deref())
        {
            request = request.with_header(header_name, header_value);
        }

        request
    }

    pub async fn build_with_channels(self, channels: u8) -> ListenClient<A> {
        let adapter = A::default();
        let params = self.get_params();
        let request = self.build_request(&adapter, channels).await;
        let initial_message = adapter.initial_message(self.api_key.as_deref(), &params, channels);

        ListenClient {
            adapter,
            request,
            initial_message,
            connect_policy: self.connect_policy,
        }
    }

    pub async fn build_single(self) -> ListenClient<A> {
        self.build_with_channels(1).await
    }

    pub async fn build_dual(self) -> ListenClientDual<A> {
        let adapter = A::default();
        let channels = if adapter.supports_native_multichannel() {
            2
        } else {
            1
        };
        let params = self.get_params();
        let request = self.build_request(&adapter, channels).await;
        let initial_message = adapter.initial_message(self.api_key.as_deref(), &params, channels);

        ListenClientDual {
            adapter,
            request,
            initial_message,
            connect_policy: self.connect_policy,
        }
    }
}

impl<A: RealtimeSttAdapter + BatchSttAdapter> ListenClientBuilder<A> {
    pub fn build_batch(self) -> BatchClient<A> {
        let params = self.get_params();
        let api_base = self.get_api_base().to_string();
        BatchClient::new(api_base, self.api_key.unwrap_or_default(), params)
    }
}
