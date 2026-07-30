use std::path::{Path, PathBuf};

use owhisper_interface::ListenParams;
use owhisper_interface::batch::{Alternatives, Channel, Response as BatchResponse, Results};
use reqwest::multipart::Form;

use crate::adapter::http::{ensure_success, streaming_file_part};
use crate::adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware, append_path_if_missing};
use crate::error::Error;

use super::CohereAdapter;

const DEFAULT_API_BASE: &str = "https://api.cohere.com/v2";
const DEFAULT_MODEL: &str = "cohere-transcribe-03-2026";

impl BatchSttAdapter for CohereAdapter {
    fn provider_name(&self) -> &'static str {
        "cohere"
    }

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
        _model: Option<&str>,
    ) -> bool {
        CohereAdapter::language_support_batch(languages).is_supported()
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
        Box::pin(do_transcribe_file(client, api_base, api_key, params, path))
    }
}

async fn do_transcribe_file(
    client: &ClientWithMiddleware,
    api_base: &str,
    api_key: &str,
    params: &ListenParams,
    file_path: PathBuf,
) -> Result<BatchResponse, Error> {
    let language = language_code(params)?;
    let form = Form::new()
        .text(
            "model",
            params.model.as_deref().unwrap_or(DEFAULT_MODEL).to_string(),
        )
        .text("language", language.to_string())
        .part("file", streaming_file_part(&file_path).await?);

    let response = client
        .post(transcription_url(api_base)?.to_string())
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await?;
    let response: CohereResponse = ensure_success(response).await?.json().await?;

    Ok(build_batch_response(response.text, language))
}

#[derive(serde::Deserialize)]
struct CohereResponse {
    text: String,
}

fn language_code(params: &ListenParams) -> Result<&str, Error> {
    if !CohereAdapter::language_support_batch(&params.languages).is_supported() {
        return Err(Error::AudioProcessing(
            "Cohere Transcribe requires one supported language".to_string(),
        ));
    }

    Ok(params
        .languages
        .first()
        .map(anlg_language::Language::iso639_code)
        .unwrap_or("en"))
}

fn transcription_url(api_base: &str) -> Result<url::Url, Error> {
    let mut url: url::Url = if api_base.is_empty() {
        DEFAULT_API_BASE
            .parse()
            .expect("invalid_default_cohere_api_base")
    } else {
        api_base.parse().map_err(|e: url::ParseError| {
            Error::AudioProcessing(format!("invalid api_base: {e}"))
        })?
    };
    append_path_if_missing(&mut url, "audio/transcriptions");
    Ok(url)
}

fn build_batch_response(transcript: String, language: &str) -> BatchResponse {
    BatchResponse {
        metadata: serde_json::json!({
            "provider": "cohere",
            "language": language,
            "timing_source": "synthetic_text",
        }),
        results: Results {
            channels: vec![Channel {
                alternatives: vec![Alternatives {
                    transcript: transcript.trim().to_string(),
                    confidence: 1.0,
                    words: Vec::new(),
                }],
            }],
        },
    }
}

#[cfg(test)]
mod tests {
    use anlg_language::ISO639;

    use super::*;

    #[test]
    fn builds_default_and_prefixed_urls() {
        assert_eq!(
            transcription_url("").unwrap().as_str(),
            "https://api.cohere.com/v2/audio/transcriptions"
        );
        assert_eq!(
            transcription_url("https://example.com/cohere/v2")
                .unwrap()
                .as_str(),
            "https://example.com/cohere/v2/audio/transcriptions"
        );
    }

    #[test]
    fn defaults_to_english_and_uses_selected_language() {
        assert_eq!(language_code(&ListenParams::default()).unwrap(), "en");
        assert_eq!(
            language_code(&ListenParams {
                languages: vec![ISO639::Ko.into()],
                ..Default::default()
            })
            .unwrap(),
            "ko"
        );
    }

    #[test]
    fn rejects_multiple_or_unsupported_languages() {
        assert!(
            language_code(&ListenParams {
                languages: vec![ISO639::En.into(), ISO639::Ko.into()],
                ..Default::default()
            })
            .is_err()
        );
        assert!(
            language_code(&ListenParams {
                languages: vec![ISO639::Ru.into()],
                ..Default::default()
            })
            .is_err()
        );
    }

    #[test]
    fn normalizes_text_only_response() {
        let response = build_batch_response(" hello world \n".to_string(), "en");
        let alternative = &response.results.channels[0].alternatives[0];

        assert_eq!(alternative.transcript, "hello world");
        assert!(alternative.words.is_empty());
        assert_eq!(response.metadata["provider"], "cohere");
        assert_eq!(response.metadata["language"], "en");
        assert_eq!(response.metadata["timing_source"], "synthetic_text");
    }
}
