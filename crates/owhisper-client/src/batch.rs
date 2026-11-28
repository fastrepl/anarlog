use std::marker::PhantomData;
use std::path::Path;

use owhisper_interface::ListenParams;
use owhisper_interface::batch::Response as BatchResponse;
use reqwest_middleware::ClientWithMiddleware;

use crate::adapter::{BatchSttAdapter, append_provider_param, is_anarlog_proxy};
use crate::error::Error;
use crate::http_client::{create_client, create_client_with_headers};
use crate::{DeepgramAdapter, ListenClientBuilder, normalize_listen_params};

pub struct BatchClientBuilder<A: BatchSttAdapter = DeepgramAdapter> {
    api_base: Option<String>,
    api_key: Option<String>,
    params: Option<ListenParams>,
    extra_headers: Vec<(String, String)>,
    _marker: PhantomData<A>,
}

impl Default for BatchClientBuilder {
    fn default() -> Self {
        Self {
            api_base: None,
            api_key: None,
            params: None,
            extra_headers: Vec::new(),
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

    pub fn extra_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_headers.push((name.into(), value.into()));
        self
    }

    pub fn adapter<B: BatchSttAdapter>(self) -> BatchClientBuilder<B> {
        BatchClientBuilder {
            api_base: self.api_base,
            api_key: self.api_key,
            params: self.params,
            extra_headers: self.extra_headers,
            _marker: PhantomData,
        }
    }

    pub fn build(self) -> BatchClient<A> {
        BatchClient::new_with_headers(
            self.api_base.expect("api_base is required"),
            self.api_key.unwrap_or_default(),
            self.params.unwrap_or_default(),
            self.extra_headers,
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
        if !is_anarlog_proxy(&api_base) {
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
            extra_headers: Vec::new(),
            _marker: PhantomData,
        }
    }

    pub fn new(api_base: String, api_key: String, params: ListenParams) -> Self {
        Self::new_with_headers(api_base, api_key, params, Vec::new())
    }

    pub fn new_with_headers(
        api_base: String,
        api_key: String,
        params: ListenParams,
        extra_headers: Vec<(String, String)>,
    ) -> Self {
        let client = if !extra_headers.is_empty() && is_anarlog_proxy(&api_base) {
            create_client_with_headers(&extra_headers)
        } else {
            create_client()
        };
        let api_base = Self::normalize_api_base(api_base);
        let params = normalize_listen_params(params);

        Self {
            client,
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

impl<A: crate::RealtimeSttAdapter + BatchSttAdapter> ListenClientBuilder<A> {
    pub fn build_batch(self) -> BatchClient<A> {
        let params = self.normalized_params();
        let api_base = self.api_base.expect("api_base is required");
        BatchClient::new_with_headers(
            api_base,
            self.api_key.unwrap_or_default(),
            params,
            self.extra_headers,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AnarlogAdapter, DeepgramAdapter, OpenAIAdapter};
    use anlg_language::{ISO639, Language};

    #[test]
    fn injects_or_rewrites_provider_param_for_anarlog_proxy() {
        let client = BatchClient::<AnarlogAdapter>::builder()
            .api_base("https://api.anarlog.so/stt")
            .api_key("test")
            .build();
        assert!(client.api_base.contains("provider=hyprnote"));

        let client = BatchClient::<OpenAIAdapter>::builder()
            .api_base("https://api.openai.com/v1")
            .api_key("test")
            .build();
        assert_eq!(client.api_base, "https://api.openai.com/v1");

        let client = BatchClient::<DeepgramAdapter>::builder()
            .api_base("https://api.anarlog.so/stt")
            .api_key("test")
            .build();
        assert!(client.api_base.contains("provider=deepgram"));

        let client = BatchClient::<OpenAIAdapter>::builder()
            .api_base("https://api.anarlog.so/stt?provider=anarlog&model=whisper-1")
            .api_key("test")
            .build();
        assert!(client.api_base.contains("provider=openai"));
        assert!(!client.api_base.contains("provider=anarlog"));
        assert!(client.api_base.contains("model=whisper-1"));
    }

    #[test]
    fn normalizes_languages_when_constructed() {
        let client = BatchClient::<DeepgramAdapter>::builder()
            .api_base("https://api.deepgram.com/v1")
            .api_key("test")
            .params(ListenParams {
                languages: vec![
                    Language::with_region(ISO639::En, "US"),
                    Language::with_region(ISO639::En, "GB"),
                    ISO639::En.into(),
                    Language::with_region(ISO639::Ko, "KR"),
                ],
                ..Default::default()
            })
            .build();

        assert_eq!(client.params.languages.len(), 2);
        assert_eq!(client.params.languages[0].iso639(), ISO639::En);
        assert_eq!(client.params.languages[0].region(), None);
        assert_eq!(client.params.languages[1].iso639(), ISO639::Ko);
        assert_eq!(client.params.languages[1].region(), Some("KR"));
    }

    #[tokio::test]
    async fn extra_headers_only_sent_to_anarlog_proxy() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/stt/listen"))
            .and(header("x-device-fingerprint", "dev-fp-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "metadata": {},
                "results": { "channels": [] }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let proxy_client = BatchClient::<crate::AnarlogAdapter>::builder()
            .api_base(format!("{}/stt", server.uri()))
            .api_key("test-key")
            .extra_header("x-device-fingerprint", "dev-fp-1")
            .build();
        proxy_client
            .transcribe_file(anlg_data::english_1::AUDIO_PATH)
            .await
            .unwrap();

        Mock::given(method("POST"))
            .and(path("/v1/listen"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "metadata": {},
                "results": { "channels": [] }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let external_client = BatchClient::<DeepgramAdapter>::builder()
            .api_base(format!("{}/v1", server.uri()))
            .api_key("test-key")
            .extra_header("x-device-fingerprint", "dev-fp-1")
            .build();
        external_client
            .transcribe_file(anlg_data::english_1::AUDIO_PATH)
            .await
            .unwrap();

        for request in server.received_requests().await.unwrap() {
            if request.url.path() == "/v1/listen" {
                assert!(
                    !request.headers.contains_key("x-device-fingerprint"),
                    "third-party endpoint must not receive x-device-fingerprint"
                );
            }
        }
    }

    #[tokio::test]
    async fn proxy_redirects_to_nonproxy_do_not_forward_extra_headers() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let proxy_server = MockServer::start().await;
        let external_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/stt/listen"))
            .respond_with(
                ResponseTemplate::new(303)
                    .insert_header("Location", format!("{}/stt/listen", external_server.uri())),
            )
            .expect(1)
            .mount(&proxy_server)
            .await;

        Mock::given(path("/stt/listen"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "metadata": {},
                "results": { "channels": [] }
            })))
            .mount(&external_server)
            .await;

        let proxy_client = BatchClient::<crate::AnarlogAdapter>::builder()
            .api_base(format!("{}/stt", proxy_server.uri()))
            .api_key("test-key")
            .extra_header("x-device-fingerprint", "dev-fp-1")
            .build();
        let result = proxy_client
            .transcribe_file(anlg_data::english_1::AUDIO_PATH)
            .await;
        assert!(
            result.is_err(),
            "redirect from proxy to third-party host must be rejected even with /stt path"
        );
        assert!(
            external_server
                .received_requests()
                .await
                .unwrap()
                .is_empty(),
            "third-party redirect target must not receive any request"
        );

        let same_host_proxy = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/stt/listen"))
            .respond_with(
                ResponseTemplate::new(303)
                    .insert_header("Location", format!("{}/v1/result", same_host_proxy.uri())),
            )
            .expect(1)
            .mount(&same_host_proxy)
            .await;
        Mock::given(path("/v1/result"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "metadata": {},
                "results": { "channels": [] }
            })))
            .mount(&same_host_proxy)
            .await;

        let same_host_client = BatchClient::<crate::AnarlogAdapter>::builder()
            .api_base(format!("{}/stt", same_host_proxy.uri()))
            .api_key("test-key")
            .extra_header("x-device-fingerprint", "dev-fp-1")
            .build();
        let same_host_result = same_host_client
            .transcribe_file(anlg_data::english_1::AUDIO_PATH)
            .await;
        assert!(
            same_host_result.is_err(),
            "redirect from proxy to non-proxy path must be rejected"
        );
        let received = same_host_proxy.received_requests().await.unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].url.path(), "/stt/listen");

        let same_origin_proxy = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/stt/listen"))
            .respond_with(ResponseTemplate::new(303).insert_header(
                "Location",
                format!("{}/stt/result", same_origin_proxy.uri()),
            ))
            .expect(1)
            .mount(&same_origin_proxy)
            .await;
        Mock::given(method("GET"))
            .and(path("/stt/result"))
            .and(header("x-device-fingerprint", "dev-fp-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "metadata": {},
                "results": { "channels": [] }
            })))
            .expect(1)
            .mount(&same_origin_proxy)
            .await;

        let same_origin_client = BatchClient::<crate::AnarlogAdapter>::builder()
            .api_base(format!("{}/stt", same_origin_proxy.uri()))
            .api_key("test-key")
            .extra_header("x-device-fingerprint", "dev-fp-1")
            .build();
        same_origin_client
            .transcribe_file(anlg_data::english_1::AUDIO_PATH)
            .await
            .expect("same-origin proxy redirect should succeed");
    }
}
