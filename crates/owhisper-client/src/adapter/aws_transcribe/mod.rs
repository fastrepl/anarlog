use std::path::Path;

use owhisper_interface::ListenParams;

use crate::adapter::openai_compatible_batch::{OpenAICompatibleBatchConfig, transcribe};
use crate::adapter::{
    BatchFuture, BatchSttAdapter, ClientWithMiddleware, LanguageQuality, LanguageSupport,
};
use crate::error::Error;

#[derive(Clone, Default)]
pub struct AwsTranscribeAdapter;

impl AwsTranscribeAdapter {
    pub fn language_support_batch(_languages: &[anlg_language::Language]) -> LanguageSupport {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    }
}

impl BatchSttAdapter for AwsTranscribeAdapter {
    fn provider_name(&self) -> &'static str {
        "aws_transcribe"
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
            if api_base.is_empty() {
                return Err(Error::AudioProcessing(
                    "Amazon Transcribe requires an OpenAI-compatible gateway URL because the native API requires SigV4 and S3"
                        .to_string(),
                ));
            }

            transcribe(
                client,
                api_base,
                api_key,
                params,
                &path,
                OpenAICompatibleBatchConfig {
                    provider: "aws_transcribe",
                    default_api_base: "",
                    default_model: "amazon-transcribe",
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
