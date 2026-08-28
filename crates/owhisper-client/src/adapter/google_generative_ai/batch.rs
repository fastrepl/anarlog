use std::path::{Path, PathBuf};

use base64::Engine;
use owhisper_interface::ListenParams;
use owhisper_interface::batch::{Alternatives, Channel, Response, Results, Word};

use super::{GoogleGenerativeAiAdapter, parse_duration_secs};
use crate::adapter::http::{ensure_success, mime_type_from_extension};
use crate::adapter::parsing::parse_speaker_id;
use crate::adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware};
use crate::error::Error;
use crate::providers::Provider;

const INLINE_AUDIO_LIMIT_BYTES: u64 = 20 * 1024 * 1024;

impl BatchSttAdapter for GoogleGenerativeAiAdapter {
    fn provider_name(&self) -> &'static str {
        "google_generative_ai"
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
    let metadata = tokio::fs::metadata(&file_path)
        .await
        .map_err(|error| Error::AudioProcessing(error.to_string()))?;
    if metadata.len() > INLINE_AUDIO_LIMIT_BYTES {
        return Err(Error::AudioProcessing(
            "Gemini 3.5 Transcribe accepts inline audio up to 20 MB; longer recordings are split automatically"
                .to_string(),
        ));
    }

    let audio = tokio::fs::read(&file_path)
        .await
        .map_err(|error| Error::AudioProcessing(error.to_string()))?;
    let model = GoogleGenerativeAiAdapter::resolve_batch_model(params.model.as_deref());

    let request = serde_json::json!({
        "model": model,
        "input": [{
            "type": "audio",
            "data": base64::engine::general_purpose::STANDARD.encode(audio),
            "mime_type": mime_type_from_extension(&file_path),
        }],
        "generation_config": {
            "transcription_config": transcription_config(params),
        }
    });

    let url = interactions_url(api_base)?;
    let response = client
        .post(url)
        .header("x-goog-api-key", api_key)
        .json(&request)
        .send()
        .await?;
    let payload: serde_json::Value = ensure_success(response).await?.json().await?;

    Ok(convert_response(&payload))
}

fn transcription_config(params: &ListenParams) -> serde_json::Value {
    // Word timestamps require verbatim mode. Google rejects custom vocabulary
    // when timestamp_granularities is set, so keywords stay on the live path.
    let mut config = serde_json::json!({
        "mode": {
            "type": "verbatim",
            "diarization_mode": "speaker",
            "timestamp_granularities": ["word"],
        }
    });
    let language_codes = GoogleGenerativeAiAdapter::language_codes(params);
    if !language_codes.is_empty() {
        let codes = serde_json::Value::Array(
            language_codes
                .into_iter()
                .map(serde_json::Value::String)
                .collect(),
        );
        config["language_hints"] = codes.clone();
        config["language_codes"] = codes;
    }
    config
}

fn interactions_url(api_base: &str) -> Result<String, Error> {
    let mut url: url::Url = if api_base.is_empty() {
        Provider::GoogleGenerativeAi
            .default_api_base()
            .parse()
            .expect("invalid_default_google_generative_ai_api_base")
    } else {
        api_base.parse().map_err(|error: url::ParseError| {
            Error::AudioProcessing(format!("invalid api_base: {error}"))
        })?
    };
    let path = url.path().trim_end_matches('/').to_string();
    if !path.ends_with("/interactions") {
        url.set_path(&format!("{path}/interactions"));
    }
    url.set_query(None);
    Ok(url.to_string())
}

fn convert_response(payload: &serde_json::Value) -> Response {
    let mut transcript_parts = Vec::new();
    let mut words = Vec::new();

    for content in content_blocks(payload) {
        let text = content
            .get("text")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim();
        if !text.is_empty() {
            transcript_parts.push(text.to_string());
        }

        let annotations = content
            .get("annotations")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut found_words = false;
        for annotation in annotations {
            if annotation.get("type").and_then(serde_json::Value::as_str) != Some("word_info") {
                continue;
            }
            let token = annotation
                .get("text")
                .or_else(|| annotation.get("word"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string();
            if token.is_empty() {
                continue;
            }
            found_words = true;
            let speaker = annotation
                .get("speaker")
                .or_else(|| annotation.get("speakerLabel"))
                .or_else(|| annotation.get("speaker_label"))
                .and_then(serde_json::Value::as_str)
                .and_then(parse_speaker_id);
            let start = annotation
                .get("start_offset")
                .or_else(|| annotation.get("startOffset"))
                .map(parse_duration_secs)
                .unwrap_or_default();
            let end = annotation
                .get("end_offset")
                .or_else(|| annotation.get("endOffset"))
                .map(parse_duration_secs)
                .unwrap_or(start);
            words.push(Word {
                punctuated_word: Some(token.clone()),
                word: token,
                start,
                end,
                confidence: 1.0,
                channel: 0,
                speaker,
            });
        }

        if !found_words && !text.is_empty() {
            let speaker = content
                .get("audioTranscription")
                .or_else(|| content.get("audio_transcription"))
                .and_then(|value| {
                    value
                        .get("speakerLabel")
                        .or_else(|| value.get("speaker_label"))
                })
                .and_then(serde_json::Value::as_str)
                .and_then(parse_speaker_id);
            words.push(Word {
                punctuated_word: Some(text.to_string()),
                word: text.to_string(),
                start: 0.0,
                end: 0.0,
                confidence: 1.0,
                channel: 0,
                speaker,
            });
        }
    }

    Response {
        metadata: serde_json::json!({ "provider": "google_generative_ai" }),
        results: Results {
            channels: vec![Channel {
                alternatives: vec![Alternatives {
                    transcript: transcript_parts.join(" "),
                    confidence: 1.0,
                    words,
                }],
            }],
        },
    }
}

fn content_blocks(payload: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut blocks = Vec::new();
    for key in ["steps", "outputs"] {
        let Some(items) = payload.get(key).and_then(serde_json::Value::as_array) else {
            continue;
        };
        for item in items {
            if let Some(content) = item.get("content").and_then(serde_json::Value::as_array) {
                blocks.extend(content.iter().cloned());
            } else if item.get("text").is_some() || item.get("type").is_some() {
                blocks.push(item.clone());
            }
        }
    }
    if blocks.is_empty() {
        if let Some(parts) = payload
            .pointer("/candidates/0/content/parts")
            .and_then(serde_json::Value::as_array)
        {
            blocks.extend(parts.iter().cloned());
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omits_language_hints_when_none_are_selected() {
        let config = transcription_config(&ListenParams::default());
        assert!(config.get("language_hints").is_none());
        assert!(config.get("language_codes").is_none());
        assert_eq!(config["mode"]["type"], "verbatim");
    }

    #[test]
    fn includes_language_hints_when_selected() {
        let config = transcription_config(&ListenParams {
            languages: vec!["en-US".parse().unwrap()],
            ..Default::default()
        });
        assert_eq!(config["language_hints"][0], "en-US");
        assert_eq!(config["language_codes"][0], "en-US");
    }

    #[test]
    fn builds_interactions_url() {
        assert_eq!(
            interactions_url("https://generativelanguage.googleapis.com/v1beta").unwrap(),
            "https://generativelanguage.googleapis.com/v1beta/interactions"
        );
        assert_eq!(
            interactions_url("https://generativelanguage.googleapis.com/v1beta/interactions")
                .unwrap(),
            "https://generativelanguage.googleapis.com/v1beta/interactions"
        );
    }

    #[test]
    fn converts_word_info_annotations() {
        let payload = serde_json::json!({
            "steps": [{
                "content": [{
                    "type": "text",
                    "text": "Hello there. Hi.",
                    "annotations": [
                        {
                            "type": "word_info",
                            "text": "Hello",
                            "speaker": "spk_2",
                            "start_offset": "0.10s",
                            "end_offset": "0.40s"
                        },
                        {
                            "type": "word_info",
                            "text": "there.",
                            "speaker": "spk_2",
                            "start_offset": "0.40s",
                            "end_offset": "0.90s"
                        },
                        {
                            "type": "word_info",
                            "text": "Hi.",
                            "speaker": "spk_1",
                            "start_offset": "1.00s",
                            "end_offset": "1.20s"
                        }
                    ]
                }]
            }]
        });

        let response = convert_response(&payload);
        let alternative = &response.results.channels[0].alternatives[0];

        assert_eq!(alternative.transcript, "Hello there. Hi.");
        assert_eq!(alternative.words[0].speaker, Some(2));
        assert_eq!(alternative.words[0].start, 0.1);
        assert_eq!(alternative.words[2].speaker, Some(1));
        assert_eq!(response.metadata["provider"], "google_generative_ai");
    }
}
