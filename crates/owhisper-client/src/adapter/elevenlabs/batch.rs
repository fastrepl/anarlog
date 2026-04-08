use std::path::Path;

use owhisper_interface::ListenParams;
use owhisper_interface::batch::{
    Alternatives as BatchAlternatives, Channel as BatchChannel, Response as BatchResponse,
    Results as BatchResults, Word as BatchWord,
};
use serde::Deserialize;

use super::{ElevenLabsAdapter, ElevenLabsWord};
use crate::adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware, MIXED_CAPTURE_CHANNEL};
use crate::error::Error;

impl BatchSttAdapter for ElevenLabsAdapter {
    fn provider_name(&self) -> &'static str {
        "elevenlabs"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        ElevenLabsAdapter::is_supported_languages_batch(languages)
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
        Box::pin(
            async move { Self::do_transcribe_file(client, api_base, api_key, params, &path).await },
        )
    }
}

#[derive(Debug, Deserialize)]
struct TranscriptResponse {
    #[serde(default)]
    language_code: Option<String>,
    #[serde(default)]
    text: String,
    #[serde(default)]
    words: Vec<ElevenLabsWord>,
}

impl ElevenLabsAdapter {
    async fn do_transcribe_file(
        client: &ClientWithMiddleware,
        api_base: &str,
        api_key: &str,
        params: &ListenParams,
        file_path: &Path,
    ) -> Result<BatchResponse, Error> {
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.wav")
            .to_string();

        let file_bytes = tokio::fs::read(file_path).await.map_err(|e| {
            Error::AudioProcessing(format!(
                "failed to read file {}: {}",
                file_path.display(),
                e
            ))
        })?;

        let default = crate::providers::Provider::ElevenLabs.default_batch_model();
        let model = match params.model.as_deref() {
            Some(m) if crate::providers::is_meta_model(m) => default,
            Some("scribe_v2") => default,
            Some(m) => m,
            None => default,
        };

        let part = reqwest::multipart::Part::bytes(file_bytes).file_name(file_name);
        let mut form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model_id", model.to_string())
            .text("diarize", "true")
            .text("timestamps_granularity", "word");

        if let Some(lang) = params.languages.first() {
            form = form.text("language_code", lang.iso639().code().to_string());
        }

        let url = Self::batch_api_url(api_base);
        tracing::info!(
            hyprnote.file.path = %file_path.display(),
            url.full = %url,
            "uploading_file_to_elevenlabs"
        );

        let response = client
            .post(&url)
            .header("xi-api-key", api_key)
            .multipart(form)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::UnexpectedStatus { status, body });
        }

        let transcript: TranscriptResponse = response.json().await?;
        tracing::info!("transcript fetched successfully from ElevenLabs");

        Ok(Self::convert_to_batch_response(transcript))
    }

    fn convert_to_batch_response(response: TranscriptResponse) -> BatchResponse {
        let words: Vec<BatchWord> = response
            .words
            .iter()
            .filter(|w| w.word_type.as_deref() == Some("word"))
            .map(|w| {
                let speaker = w
                    .speaker_id
                    .as_ref()
                    .and_then(|s| s.trim_start_matches("speaker_").parse::<usize>().ok());
                BatchWord {
                    word: w.text.clone(),
                    start: w.start,
                    end: w.end,
                    confidence: 1.0,
                    channel: MIXED_CAPTURE_CHANNEL,
                    speaker,
                    punctuated_word: Some(w.text.clone()),
                }
            })
            .collect();

        let alternatives = BatchAlternatives {
            transcript: response.text,
            confidence: 1.0,
            words,
        };

        let channel = BatchChannel {
            alternatives: vec![alternatives],
        };

        BatchResponse {
            metadata: serde_json::json!({
                "language_code": response.language_code,
            }),
            results: BatchResults {
                channels: vec![channel],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_client::create_client;

    #[test]
    fn speaker_labeled_words_use_mixed_capture_channel() {
        let response = TranscriptResponse {
            language_code: Some("en".to_string()),
            text: "hello there".to_string(),
            words: vec![
                ElevenLabsWord {
                    text: "hello".to_string(),
                    start: 0.0,
                    end: 0.5,
                    word_type: Some("word".to_string()),
                    speaker_id: Some("speaker_0".to_string()),
                },
                ElevenLabsWord {
                    text: "there".to_string(),
                    start: 0.5,
                    end: 1.0,
                    word_type: Some("word".to_string()),
                    speaker_id: Some("speaker_1".to_string()),
                },
            ],
        };

        let batch = ElevenLabsAdapter::convert_to_batch_response(response);
        let words = &batch.results.channels[0].alternatives[0].words;

        assert_eq!(words[0].channel, MIXED_CAPTURE_CHANNEL);
        assert_eq!(words[0].speaker, Some(0));
        assert_eq!(words[1].channel, MIXED_CAPTURE_CHANNEL);
        assert_eq!(words[1].speaker, Some(1));
    }

    #[tokio::test]
    #[ignore]
    async fn test_elevenlabs_batch_transcription() {
        let api_key = std::env::var("ELEVENLABS_API_KEY").expect("ELEVENLABS_API_KEY not set");
        let client = create_client();
        let adapter = ElevenLabsAdapter::default();
        let params = ListenParams::default();

        let audio_path = std::path::PathBuf::from(hypr_data::english_1::AUDIO_PATH);

        let result = adapter
            .transcribe_file(&client, "", &api_key, &params, &audio_path)
            .await
            .expect("transcription failed");

        assert!(!result.results.channels.is_empty());
        assert!(!result.results.channels[0].alternatives.is_empty());
        assert!(
            !result.results.channels[0].alternatives[0]
                .transcript
                .is_empty()
        );
        assert!(!result.results.channels[0].alternatives[0].words.is_empty());
    }
}
