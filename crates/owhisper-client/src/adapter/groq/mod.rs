use std::path::Path;

use owhisper_interface::ListenParams;

use crate::adapter::openai_compatible_batch::{OpenAICompatibleBatchConfig, transcribe};
use crate::adapter::{
    BatchFuture, BatchSttAdapter, ClientWithMiddleware, LanguageQuality, LanguageSupport,
};

#[derive(Clone, Default)]
pub struct GroqAdapter;

impl GroqAdapter {
    pub fn language_support_batch(_languages: &[anlg_language::Language]) -> LanguageSupport {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    }
}

impl BatchSttAdapter for GroqAdapter {
    fn provider_name(&self) -> &'static str {
        "groq"
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
                    provider: "groq",
                    default_api_base: "https://api.groq.com/openai/v1",
                    default_model: "whisper-large-v3-turbo",
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
