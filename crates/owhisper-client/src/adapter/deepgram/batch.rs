use std::path::{Path, PathBuf};

use owhisper_interface::ListenParams;
use owhisper_interface::batch::Response as BatchResponse;

use crate::adapter::deepgram_compat::build_batch_url;
use crate::adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware};
use crate::error::Error;

use super::{
    DeepgramAdapter, keywords::DeepgramKeywordStrategy, language::DeepgramLanguageStrategy,
};

impl BatchSttAdapter for DeepgramAdapter {
    fn provider_name(&self) -> &'static str {
        "deepgram"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        DeepgramAdapter::is_supported_languages_batch(languages, model)
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

use crate::adapter::http::mime_type_from_extension;

async fn do_transcribe_file(
    client: &ClientWithMiddleware,
    api_base: &str,
    api_key: &str,
    params: &ListenParams,
    file_path: PathBuf,
) -> Result<BatchResponse, Error> {
    let audio_data = tokio::fs::read(&file_path)
        .await
        .map_err(|e| Error::AudioProcessing(format!("failed to read file: {}", e)))?;

    let content_type = mime_type_from_extension(&file_path);

    let url = build_batch_url(
        api_base,
        params,
        &DeepgramLanguageStrategy,
        &DeepgramKeywordStrategy,
    );

    let response = client
        .post(url)
        .header("Authorization", format!("Token {}", api_key))
        .header("Accept", "application/json")
        .header("Content-Type", content_type)
        .body(audio_data)
        .send()
        .await?;

    let status = response.status();
    if status.is_success() {
        Ok(response.json().await?)
    } else {
        Err(Error::UnexpectedStatus {
            status,
            body: response.text().await.unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_client::create_client;

    #[tokio::test]
    #[ignore]
    async fn test_deepgram_batch_transcription() {
        let api_key = std::env::var("DEEPGRAM_API_KEY").expect("DEEPGRAM_API_KEY not set");
        let client = create_client();
        let adapter = DeepgramAdapter::default();
        let params = ListenParams {
            model: Some("nova-2".to_string()),
            ..Default::default()
        };

        let audio_path = std::path::PathBuf::from(hypr_data::english_1::AUDIO_PATH);

        let result = adapter
            .transcribe_file(
                &client,
                "https://api.deepgram.com/v1",
                &api_key,
                &params,
                &audio_path,
            )
            .await
            .expect("transcription failed");

        assert!(!result.results.channels.is_empty());
        assert!(!result.results.channels[0].alternatives.is_empty());
        assert!(
            !result.results.channels[0].alternatives[0]
                .transcript
                .is_empty()
        );
    }
}
