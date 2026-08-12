use std::path::Path;

use owhisper_interface::ListenParams;

use crate::adapter::openai_compatible_batch::{OpenAICompatibleBatchConfig, transcribe};
use crate::adapter::{
    BatchFuture, BatchSttAdapter, ClientWithMiddleware, LanguageQuality, LanguageSupport,
};

#[derive(Clone, Default)]
pub struct SiliconFlowAdapter;

impl SiliconFlowAdapter {
    pub fn language_support_batch(_languages: &[anlg_language::Language]) -> LanguageSupport {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    }
}

impl BatchSttAdapter for SiliconFlowAdapter {
    fn provider_name(&self) -> &'static str {
        "siliconflow"
    }

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
        _model: Option<&str>,
    ) -> bool {
        Self::language_support_batch(languages).is_supported()
    }

    fn transcribe_file<'a, P: AsRef<Path> + Send + 'a>(
        &'a self,
        client: &'a ClientWithMiddleware,
        api_base: &'a str,
        api_key: &'a str,
        params: &'a ListenParams,
        file_path: P,
    ) -> BatchFuture<'a> {
        let path = file_path.as_ref().to_path_buf();
        Box::pin(async move {
            transcribe(
                client,
                api_base,
                api_key,
                params,
                &path,
                OpenAICompatibleBatchConfig {
                    provider: "siliconflow",
                    default_api_base: "https://api.siliconflow.com/v1",
                    default_model: "FunAudioLLM/SenseVoiceSmall",
                    transcription_path: "audio/transcriptions",
                    response_format: None,
                    timestamp_field: None,
                    include_language: false,
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
    async fn uses_siliconflow_transcription_contract() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/audio/transcriptions"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "text": "Hello from SiliconFlow."
            })))
            .mount(&server)
            .await;
        let params = ListenParams {
            model: Some("FunAudioLLM/SenseVoiceSmall".to_string()),
            languages: vec![anlg_language::ISO639::En.into()],
            ..Default::default()
        };

        let response = SiliconFlowAdapter
            .transcribe_file(
                &crate::http_client::create_client(),
                &server.uri(),
                "test-key",
                &params,
                anlg_data::english_1::AUDIO_PATH,
            )
            .await
            .unwrap();

        let requests = server.received_requests().await.unwrap();
        let body = String::from_utf8_lossy(&requests[0].body);
        assert!(body.contains("FunAudioLLM/SenseVoiceSmall"));
        assert!(!body.contains("name=\"language\""));
        assert_eq!(
            response.results.channels[0].alternatives[0].transcript,
            "Hello from SiliconFlow."
        );
    }
}
