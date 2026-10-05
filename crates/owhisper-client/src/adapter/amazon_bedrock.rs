use super::openai_compatible_batch::{OpenAICompatibleBatchConfig, transcribe};
use super::{BatchFuture, BatchSttAdapter, ClientWithMiddleware};
use owhisper_interface::ListenParams;
use std::path::Path;

#[derive(Clone, Default)]
pub struct AmazonBedrockAdapter;

impl BatchSttAdapter for AmazonBedrockAdapter {
    fn provider_name(&self) -> &'static str {
        "amazon_bedrock"
    }
    fn is_supported_languages(&self, _: &[anlg_language::Language], _: Option<&str>) -> bool {
        true
    }
    fn transcribe_file<'a, P: AsRef<Path> + Send + 'a>(
        &'a self,
        client: &'a ClientWithMiddleware,
        base: &'a str,
        key: &'a str,
        params: &'a ListenParams,
        path: P,
    ) -> BatchFuture<'a> {
        let path = path.as_ref().to_path_buf();
        Box::pin(async move {
            let mut url = url::Url::parse(base).map_err(|_| {
                crate::Error::provider_configuration(
                    "amazon_bedrock",
                    "Enter a transcription gateway URL.",
                )
            })?;
            if url.host_str().is_some_and(|host| {
                host.ends_with(".amazonaws.com")
                    && (host.starts_with("bedrock-runtime.")
                        || host.starts_with("bedrock.")
                        || host.starts_with("bedrock-mantle."))
            }) {
                return Err(crate::Error::provider_configuration(
                    "amazon_bedrock",
                    "Native Bedrock endpoints require AWS bidirectional streaming. Enter an OpenAI-compatible transcription gateway URL.",
                ));
            }
            let query: Vec<_> = url
                .query_pairs()
                .filter(|(key, _)| key != "provider")
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect();
            url.set_query(None);
            if !query.is_empty() {
                url.query_pairs_mut().extend_pairs(query);
            }
            transcribe(
                client,
                url.as_str(),
                key,
                params,
                &path,
                OpenAICompatibleBatchConfig {
                    provider: "amazon_bedrock",
                    default_api_base: "",
                    default_model: "amazon.nova-2-sonic-v1:0",
                    transcription_path: "audio/transcriptions",
                    response_format: Some("verbose_json"),
                    timestamp_field: Some("timestamp_granularities[]"),
                    include_language: true,
                },
            )
            .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn gateway_keeps_nova_model_without_leaking_routing_query() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/audio/transcriptions"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "text": "Hello from Nova.",
                "words": [{"word":"Hello", "start":0.1, "end":0.5}]
            })))
            .mount(&server)
            .await;
        let params = ListenParams {
            model: Some("amazon.nova-2-sonic-v1:0".into()),
            ..Default::default()
        };
        let response = AmazonBedrockAdapter
            .transcribe_file(
                &crate::http_client::create_client(),
                &format!("{}/v1?provider=amazon_bedrock", server.uri()),
                "test-key",
                &params,
                anlg_data::english_1::AUDIO_PATH,
            )
            .await
            .unwrap();
        let requests = server.received_requests().await.unwrap();
        assert!(requests[0].url.query().is_none());
        let body = String::from_utf8_lossy(&requests[0].body);
        assert!(body.contains("amazon.nova-2-sonic-v1:0"));
        assert!(!body.contains("gpt-4o"));
        let alternative = &response.results.channels[0].alternatives[0];
        assert_eq!(alternative.transcript, "Hello from Nova.");
        assert_eq!(alternative.words[0].start, 0.1);
    }
}
