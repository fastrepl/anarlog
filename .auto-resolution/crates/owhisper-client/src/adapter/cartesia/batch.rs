use std::path::Path;

use owhisper_interface::ListenParams;
use owhisper_interface::batch::{
    Alternatives, Channel, Response as BatchResponse, Results, Word as BatchWord,
};
use reqwest::multipart::Form;

use crate::adapter::http::{ensure_success, streaming_file_part};
use crate::adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware, MIXED_CAPTURE_CHANNEL};
use crate::error::Error;

use super::{API_VERSION, CartesiaAdapter, DEFAULT_MODEL};

impl BatchSttAdapter for CartesiaAdapter {
    fn provider_name(&self) -> &'static str {
        "cartesia"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        CartesiaAdapter::is_supported_languages_batch(languages)
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
        Box::pin(async move { transcribe_file(client, api_base, api_key, params, &path).await })
    }
}

#[derive(Debug, serde::Deserialize)]
struct CartesiaTranscriptionResponse {
    #[serde(rename = "type")]
    response_type: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    request_id: Option<String>,
    #[serde(default)]
    is_final: Option<bool>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    words: Vec<CartesiaWord>,
}

#[derive(Debug, serde::Deserialize)]
struct CartesiaWord {
    #[serde(default)]
    word: String,
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
}

async fn transcribe_file(
    client: &ClientWithMiddleware,
    api_base: &str,
    api_key: &str,
    params: &ListenParams,
    file_path: &Path,
) -> Result<BatchResponse, Error> {
    let file_part = streaming_file_part(file_path).await?;
    let model = resolve_model(params.model.as_deref());

    let mut form = Form::new()
        .part("file", file_part)
        .text("model", model.to_string())
        .text("timestamp_granularities[]", "word");

    if let Some(language) = params.languages.first() {
        form = form.text("language", language.iso639().code().to_string());
    }

    let mut request = client.post(transcription_url(api_base)?.to_string());
    for (name, value) in request_headers(api_key) {
        request = request.header(name, value);
    }

    let response = request.multipart(form).send().await?;

    let response = ensure_success(response).await?;
    let transcript = response.json::<CartesiaTranscriptionResponse>().await?;

    Ok(convert_response(transcript))
}

fn resolve_model(model: Option<&str>) -> &str {
    match model {
        Some(model) if crate::providers::is_meta_model(model) => DEFAULT_MODEL,
        Some("ink-2") => DEFAULT_MODEL,
        Some(model) => model,
        None => DEFAULT_MODEL,
    }
}

fn transcription_url(api_base: &str) -> Result<url::Url, Error> {
    let mut url: url::Url = if api_base.is_empty() {
        crate::providers::Provider::Cartesia
            .default_api_base()
            .parse()
            .expect("invalid_default_cartesia_api_base")
    } else {
        api_base
            .parse()
            .map_err(|e: url::ParseError| Error::AudioProcessing(e.to_string()))?
    };

    crate::adapter::append_path_if_missing(&mut url, "stt");
    Ok(url)
}

fn request_headers(api_key: &str) -> [(&'static str, String); 2] {
    [
        ("X-API-Key", api_key.to_string()),
        ("Cartesia-Version", API_VERSION.to_string()),
    ]
}

fn convert_response(response: CartesiaTranscriptionResponse) -> BatchResponse {
    let words = response
        .words
        .into_iter()
        .map(|word| BatchWord {
            word: word.word.clone(),
            start: word.start,
            end: word.end,
            confidence: 1.0,
            channel: MIXED_CAPTURE_CHANNEL,
            speaker: None,
            punctuated_word: Some(word.word),
        })
        .collect();

    BatchResponse {
        metadata: serde_json::json!({
            "provider": "cartesia",
            "type": response.response_type,
            "request_id": response.request_id,
            "is_final": response.is_final,
            "language": response.language,
            "duration": response.duration,
        }),
        results: Results {
            channels: vec![Channel {
                alternatives: vec![Alternatives {
                    transcript: response.text,
                    confidence: 1.0,
                    words,
                }],
            }],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_model_uses_default_for_meta_models() {
        assert_eq!(resolve_model(None), DEFAULT_MODEL);
        assert_eq!(resolve_model(Some("cloud")), DEFAULT_MODEL);
        assert_eq!(resolve_model(Some("auto")), DEFAULT_MODEL);
        assert_eq!(resolve_model(Some("ink-2")), DEFAULT_MODEL);
        assert_eq!(resolve_model(Some("ink-whisper")), "ink-whisper");
    }

    #[test]
    fn transcription_url_appends_stt() {
        assert_eq!(
            transcription_url("").unwrap().as_str(),
            "https://api.cartesia.ai/stt"
        );
        assert_eq!(
            transcription_url("https://api.cartesia.ai")
                .unwrap()
                .as_str(),
            "https://api.cartesia.ai/stt"
        );
        assert_eq!(
            transcription_url("https://api.cartesia.ai/stt")
                .unwrap()
                .as_str(),
            "https://api.cartesia.ai/stt"
        );
    }

    #[test]
    fn request_headers_use_cartesia_api_key_header() {
        let headers = request_headers("cartesia-key");

        assert_eq!(headers[0], ("X-API-Key", "cartesia-key".to_string()));
        assert_eq!(headers[1], ("Cartesia-Version", API_VERSION.to_string()));
        assert!(!headers.iter().any(|(name, _)| *name == "Authorization"));
    }

    #[test]
    fn convert_response_preserves_word_timestamps() {
        let batch = convert_response(CartesiaTranscriptionResponse {
            response_type: "transcript".to_string(),
            text: "hello world".to_string(),
            request_id: Some("req_123".to_string()),
            is_final: Some(true),
            language: Some("en".to_string()),
            duration: Some(1.2),
            words: vec![
                CartesiaWord {
                    word: "hello".to_string(),
                    start: 0.0,
                    end: 0.4,
                },
                CartesiaWord {
                    word: "world".to_string(),
                    start: 0.5,
                    end: 0.9,
                },
            ],
        });

        let alternative = &batch.results.channels[0].alternatives[0];
        assert_eq!(alternative.transcript, "hello world");
        assert_eq!(alternative.words.len(), 2);
        assert_eq!(alternative.words[0].word, "hello");
        assert_eq!(alternative.words[0].channel, MIXED_CAPTURE_CHANNEL);
        assert_eq!(alternative.words[0].speaker, None);
        assert_eq!(batch.metadata["provider"], "cartesia");
    }
}
