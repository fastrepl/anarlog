use std::path::{Path, PathBuf};

use owhisper_interface::ListenParams;
use owhisper_interface::batch::{Alternatives, Channel, Response, Results, Word};
use reqwest::multipart::Form;

use crate::adapter::http::{ensure_success, streaming_file_part};
use crate::adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware, append_path_if_missing};
use crate::error::Error;

use super::XaiAdapter;

impl BatchSttAdapter for XaiAdapter {
    fn provider_name(&self) -> &'static str {
        "xai"
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
        Box::pin(do_transcribe_file(client, api_base, api_key, params, path))
    }
}

async fn do_transcribe_file(
    client: &ClientWithMiddleware,
    api_base: &str,
    api_key: &str,
    params: &ListenParams,
    file_path: PathBuf,
) -> Result<Response, Error> {
    let mut form = Form::new();
    if let Some(language) = params.languages.first() {
        form = form
            .text("format", "true")
            .text("language", language.iso639().code().to_string());
    }
    if params.num_speakers.is_some()
        || params.min_speakers.is_some()
        || params.max_speakers.is_some()
    {
        form = form.text("diarize", "true");
    }
    for keyword in &params.keywords {
        form = form.text("keyterm", keyword.clone());
    }

    // xAI requires every option to precede the file part.
    form = form.part("file", streaming_file_part(&file_path).await?);

    let mut url: url::Url = if api_base.is_empty() {
        "https://api.x.ai/v1"
            .parse()
            .expect("invalid_default_xai_api_base")
    } else {
        api_base.parse().map_err(|error: url::ParseError| {
            Error::AudioProcessing(format!("invalid api_base: {error}"))
        })?
    };
    append_path_if_missing(&mut url, "stt");

    let response = client
        .post(url.to_string())
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await?;
    let payload: XaiBatchResponse = ensure_success(response).await?.json().await?;

    Ok(convert_response(payload))
}

#[derive(serde::Deserialize)]
struct XaiBatchResponse {
    text: String,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    words: Vec<XaiBatchWord>,
    #[serde(default)]
    channels: Vec<XaiBatchChannel>,
}

#[derive(serde::Deserialize)]
struct XaiBatchChannel {
    #[serde(default)]
    index: i32,
    #[serde(default)]
    text: String,
    #[serde(default)]
    words: Vec<XaiBatchWord>,
}

#[derive(serde::Deserialize)]
struct XaiBatchWord {
    text: String,
    start: f64,
    end: f64,
    #[serde(default)]
    speaker: Option<usize>,
}

fn convert_words(words: Vec<XaiBatchWord>, channel: i32) -> Vec<Word> {
    words
        .into_iter()
        .map(|word| Word {
            punctuated_word: Some(word.text.clone()),
            word: word.text,
            start: word.start,
            end: word.end,
            confidence: 1.0,
            channel,
            speaker: word.speaker,
        })
        .collect()
}

fn convert_response(payload: XaiBatchResponse) -> Response {
    let channels = if payload.channels.is_empty() {
        vec![Channel {
            alternatives: vec![Alternatives {
                transcript: payload.text.trim().to_string(),
                confidence: 1.0,
                words: convert_words(payload.words, 0),
            }],
        }]
    } else {
        payload
            .channels
            .into_iter()
            .map(|channel| Channel {
                alternatives: vec![Alternatives {
                    transcript: channel.text.trim().to_string(),
                    confidence: 1.0,
                    words: convert_words(channel.words, channel.index),
                }],
            })
            .collect()
    };

    Response {
        metadata: serde_json::json!({
            "provider": "xai",
            "language": payload.language,
            "duration": payload.duration,
        }),
        results: Results { channels },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_diarized_words() {
        let payload: XaiBatchResponse = serde_json::from_value(serde_json::json!({
            "text": "Hello there.",
            "duration": 1.5,
            "words": [
                { "text": "Hello", "start": 0.1, "end": 0.5, "speaker": 2 },
                { "text": "there.", "start": 0.5, "end": 1.0, "speaker": 2 }
            ]
        }))
        .unwrap();

        let response = convert_response(payload);
        let alternative = &response.results.channels[0].alternatives[0];

        assert_eq!(alternative.transcript, "Hello there.");
        assert_eq!(alternative.words[0].speaker, Some(2));
        assert_eq!(response.metadata["provider"], "xai");
    }
}
