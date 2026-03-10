use std::path::{Path, PathBuf};

use owhisper_interface::ListenParams;
use owhisper_interface::batch::{Alternatives, Channel, Response as BatchResponse, Results, Word};
use reqwest::multipart::{Form, Part};

use crate::adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware, append_path_if_missing};
use crate::error::Error;

use super::OpenAIAdapter;

use crate::providers::{Provider, is_meta_model};

const DEFAULT_API_BASE: &str = "https://api.openai.com/v1";
const RESPONSE_FORMAT_VERBOSE: &str = "verbose_json";
const RESPONSE_FORMAT_JSON: &str = "json";
const TIMESTAMP_GRANULARITY: &str = "word";

// Models that support verbose_json with word-level timestamps
fn supports_word_timestamps(model: &str) -> bool {
    model == "whisper-1"
}

impl BatchSttAdapter for OpenAIAdapter {
    fn provider_name(&self) -> &'static str {
        "openai"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        OpenAIAdapter::is_supported_languages_batch(languages)
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

#[derive(Debug, serde::Deserialize)]
struct OpenAIWord {
    word: String,
    start: f64,
    end: f64,
}

#[derive(Debug, serde::Deserialize)]
struct OpenAIVerboseResponse {
    #[allow(dead_code)]
    task: Option<String>,
    language: Option<String>,
    #[allow(dead_code)]
    duration: Option<f64>,
    text: String,
    #[serde(default)]
    words: Vec<OpenAIWord>,
}

async fn do_transcribe_file(
    client: &ClientWithMiddleware,
    api_base: &str,
    api_key: &str,
    params: &ListenParams,
    file_path: PathBuf,
) -> Result<BatchResponse, Error> {
    let fallback_name = match file_path.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("audio.{}", ext),
        None => "audio".to_string(),
    };

    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or(fallback_name);

    let file_bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|e| Error::AudioProcessing(e.to_string()))?;

    let mime_type = mime_type_from_extension(&file_path);

    let file_part = Part::bytes(file_bytes)
        .file_name(file_name)
        .mime_str(mime_type)
        .map_err(|e| Error::AudioProcessing(e.to_string()))?;

    let default = Provider::OpenAI.default_batch_model();
    let model = match params.model.as_deref() {
        Some(m) if is_meta_model(m) => default,
        Some(m) => m,
        None => default,
    };

    let mut form = Form::new()
        .part("file", file_part)
        .text("model", model.to_string());

    // whisper-1 supports verbose_json with word-level timestamps
    // gpt-4o-transcribe and gpt-4o-mini-transcribe only support json/text
    if supports_word_timestamps(model) {
        form = form
            .text("response_format", RESPONSE_FORMAT_VERBOSE)
            .text("timestamp_granularities[]", TIMESTAMP_GRANULARITY);
    } else {
        form = form.text("response_format", RESPONSE_FORMAT_JSON);
    }

    if let Some(lang) = params.languages.first() {
        form = form.text("language", lang.iso639().code().to_string());
    }

    let mut url: url::Url = if api_base.is_empty() {
        DEFAULT_API_BASE
            .parse()
            .expect("invalid_default_openai_api_base")
    } else {
        api_base.parse().map_err(|e: url::ParseError| {
            Error::AudioProcessing(format!("invalid api_base: {e}"))
        })?
    };
    append_path_if_missing(&mut url, "audio/transcriptions");

    let response = client
        .post(url.to_string())
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await?;

    let status = response.status();
    if status.is_success() {
        let openai_response: OpenAIVerboseResponse = response.json().await?;
        Ok(convert_response(openai_response))
    } else {
        Err(Error::UnexpectedStatus {
            status,
            body: response.text().await.unwrap_or_default(),
        })
    }
}

use crate::adapter::http::mime_type_from_extension;

fn strip_punctuation(s: &str) -> String {
    s.trim_matches(|c: char| c.is_ascii_punctuation())
        .to_string()
}

fn convert_response(response: OpenAIVerboseResponse) -> BatchResponse {
    let words: Vec<Word> = if !response.words.is_empty() {
        response
            .words
            .into_iter()
            .map(|w| {
                let normalized = strip_punctuation(&w.word);
                Word {
                    word: if normalized.is_empty() {
                        w.word.clone()
                    } else {
                        normalized
                    },
                    start: w.start,
                    end: w.end,
                    confidence: 1.0,
                    speaker: None,
                    punctuated_word: Some(w.word),
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    let alternatives = Alternatives {
        transcript: response.text.trim().to_string(),
        confidence: 1.0,
        words,
    };

    let channel = Channel {
        alternatives: vec![alternatives],
    };

    let metadata = serde_json::json!({
        "language": response.language,
    });

    BatchResponse {
        metadata,
        results: Results {
            channels: vec![channel],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::BatchSttAdapter;
    use crate::http_client::create_client;

    #[tokio::test]
    #[ignore]
    async fn test_openai_transcribe() {
        let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

        let adapter = OpenAIAdapter::default();
        let client = create_client();
        let api_base = "https://api.openai.com/v1";

        let params = ListenParams::default();

        let audio_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/data/src/english_1/audio.wav");

        let result = adapter
            .transcribe_file(&client, api_base, &api_key, &params, &audio_path)
            .await;

        let response = result.expect("transcription should succeed");

        assert!(!response.results.channels.is_empty());
        let channel = &response.results.channels[0];
        assert!(!channel.alternatives.is_empty());
        let alt = &channel.alternatives[0];
        assert!(!alt.transcript.is_empty());
        println!("Transcript: {}", alt.transcript);
        println!("Word count: {}", alt.words.len());
    }
}
