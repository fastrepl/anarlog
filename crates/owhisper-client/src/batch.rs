use std::marker::PhantomData;
use std::path::Path;

use owhisper_interface::ListenParams;
use owhisper_interface::batch::Response as BatchResponse;
use reqwest_middleware::ClientWithMiddleware;

use crate::DeepgramAdapter;
use crate::adapter::{BatchSttAdapter, append_provider_param, is_hyprnote_proxy};
use crate::error::Error;
use crate::http_client::create_client;

pub struct BatchClientBuilder<A: BatchSttAdapter = DeepgramAdapter> {
    api_base: Option<String>,
    api_key: Option<String>,
    params: Option<ListenParams>,
    _marker: PhantomData<A>,
}

impl Default for BatchClientBuilder {
    fn default() -> Self {
        Self {
            api_base: None,
            api_key: None,
            params: None,
            _marker: PhantomData,
        }
    }
}

impl<A: BatchSttAdapter> BatchClientBuilder<A> {
    pub fn api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = Some(api_base.into());
        self
    }

    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn params(mut self, params: ListenParams) -> Self {
        self.params = Some(params);
        self
    }

    pub fn adapter<B: BatchSttAdapter>(self) -> BatchClientBuilder<B> {
        BatchClientBuilder {
            api_base: self.api_base,
            api_key: self.api_key,
            params: self.params,
            _marker: PhantomData,
        }
    }

    pub fn build(self) -> BatchClient<A> {
        BatchClient::new(
            self.api_base.expect("api_base is required"),
            self.api_key.unwrap_or_default(),
            self.params.unwrap_or_default(),
        )
    }
}

#[derive(Clone)]
pub struct BatchClient<A: BatchSttAdapter = DeepgramAdapter> {
    client: ClientWithMiddleware,
    api_base: String,
    api_key: String,
    params: ListenParams,
    _marker: PhantomData<A>,
}

impl<A: BatchSttAdapter> BatchClient<A> {
    fn normalize_api_base(api_base: String) -> String {
        if !is_hyprnote_proxy(&api_base) {
            return api_base;
        }
        let provider_name = A::default().provider_name();
        if provider_name == "unknown" {
            return api_base;
        }
        append_provider_param(&api_base, provider_name)
    }

    pub fn builder() -> BatchClientBuilder<A> {
        BatchClientBuilder {
            api_base: None,
            api_key: None,
            params: None,
            _marker: PhantomData,
        }
    }

    pub fn new(api_base: String, api_key: String, params: ListenParams) -> Self {
        let api_base = Self::normalize_api_base(api_base);
        Self {
            client: create_client(),
            api_base,
            api_key,
            params,
            _marker: PhantomData,
        }
    }

    pub async fn transcribe_file<P: AsRef<Path> + Send>(
        &self,
        file_path: P,
    ) -> Result<BatchResponse, Error> {
        A::default()
            .transcribe_file(
                &self.client,
                &self.api_base,
                &self.api_key,
                &self.params,
                file_path,
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeepgramAdapter, HyprnoteAdapter, OpenAIAdapter};

    #[test]
    fn injects_provider_for_hyprnote_proxy() {
        let client = BatchClient::<HyprnoteAdapter>::builder()
            .api_base("https://api.hyprnote.com/stt")
            .api_key("test")
            .build();

        assert!(client.api_base.contains("provider=hyprnote"));
    }

    #[test]
    fn does_not_inject_provider_for_direct_provider_url() {
        let client = BatchClient::<OpenAIAdapter>::builder()
            .api_base("https://api.openai.com/v1")
            .api_key("test")
            .build();

        assert_eq!(client.api_base, "https://api.openai.com/v1");
    }

    #[test]
    fn injects_provider_for_direct_provider_adapter_on_hyprnote_proxy() {
        let client = BatchClient::<DeepgramAdapter>::builder()
            .api_base("https://api.hyprnote.com/stt")
            .api_key("test")
            .build();

        assert!(client.api_base.contains("provider=deepgram"));
    }

    #[test]
    fn rewrites_existing_provider_for_direct_provider_adapter() {
        let client = BatchClient::<OpenAIAdapter>::builder()
            .api_base("https://api.hyprnote.com/stt?provider=hyprnote&model=whisper-1")
            .api_key("test")
            .build();

        assert!(client.api_base.contains("provider=openai"));
        assert!(!client.api_base.contains("provider=hyprnote"));
        assert!(client.api_base.contains("model=whisper-1"));
    }
}
