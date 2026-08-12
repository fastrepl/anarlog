use std::path::Path;

use owhisper_interface::ListenParams;
use owhisper_interface::batch::{Alternatives, Channel, Response, Results, Word};
use reqwest::multipart::Form;
use serde::Deserialize;

use crate::adapter::http::{ensure_success, streaming_file_part};
use crate::adapter::{ClientWithMiddleware, append_path_if_missing};
use crate::error::Error;

pub(crate) struct OpenAICompatibleBatchConfig<'a> {
    pub provider: &'a str,
    pub default_api_base: &'a str,
    pub default_model: &'a str,
    pub transcription_path: &'a str,
    pub response_format: Option<&'a str>,
    pub timestamp_field: Option<&'a str>,
    pub include_language: bool,
}

pub(crate) async fn transcribe(
    client: &ClientWithMiddleware,
    api_base: &str,
    api_key: &str,
    params: &ListenParams,
    file_path: &Path,
    config: OpenAICompatibleBatchConfig<'_>,
) -> Result<Response, Error> {
    let model = match params.model.as_deref() {
        Some(model) if !crate::providers::is_meta_model(model) => model,
        _ => config.default_model,
    };

    let mut form = Form::new().text("model", model.to_string());

    if let Some(response_format) = config.response_format {
        form = form.text("response_format", response_format.to_string());
    }
    if let Some(field) = config.timestamp_field {
        form = form.text(field.to_string(), "word");
    }
    if config.include_language
        && let Some(language) = params.languages.first()
    {
        form = form.text("language", language.iso639().code().to_string());
    }
    form = form.part("file", streaming_file_part(file_path).await?);

    let mut url: url::Url = if api_base.is_empty() {
        config
            .default_api_base
            .parse()
            .expect("invalid_default_openai_compatible_api_base")
    } else {
        api_base.parse().map_err(|error: url::ParseError| {
            Error::AudioProcessing(format!("invalid api_base: {error}"))
        })?
    };
    append_path_if_missing(&mut url, config.transcription_path);

    let response = client
        .post(url.to_string())
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await?;
    let payload: CompatibleResponse = ensure_success(response).await?.json().await?;

    Ok(convert_response(config.provider, payload))
}

#[derive(Debug, serde::Deserialize)]
struct CompatibleResponse {
    #[serde(default)]
    text: String,
    #[serde(default)]
    words: Vec<CompatibleWord>,
    #[serde(default)]
    segments: Vec<CompatibleSegment>,
}

#[derive(Debug, serde::Deserialize)]
struct CompatibleSegment {
    #[serde(default)]
    text: String,
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
    #[serde(default)]
    words: Vec<CompatibleWord>,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct CompatibleWord {
    #[serde(default, alias = "text")]
    word: String,
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
    #[serde(default = "default_confidence")]
    confidence: f64,
    #[serde(default, deserialize_with = "deserialize_optional_speaker")]
    speaker: Option<usize>,
}

fn default_confidence() -> f64 {
    1.0
}

fn deserialize_optional_speaker<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| match value {
        serde_json::Value::Number(number) => number
            .as_u64()
            .and_then(|speaker| usize::try_from(speaker).ok()),
        serde_json::Value::String(value) => value
            .trim_start_matches(|character: char| !character.is_ascii_digit())
            .parse()
            .ok(),
        _ => None,
    }))
}

fn convert_response(provider: &str, payload: CompatibleResponse) -> Response {
    let words = if payload.words.is_empty() {
        payload
            .segments
            .iter()
            .flat_map(|segment| {
                if segment.words.is_empty() {
                    vec![CompatibleWord {
                        word: segment.text.clone(),
                        start: segment.start,
                        end: segment.end,
                        confidence: 1.0,
                        speaker: None,
                    }]
                } else {
                    segment.words.clone()
                }
            })
            .collect()
    } else {
        payload.words
    };
    let transcript = if payload.text.trim().is_empty() {
        payload
            .segments
            .iter()
            .map(|segment| segment.text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        payload.text.trim().to_string()
    };

    Response {
        metadata: serde_json::json!({ "provider": provider }),
        results: Results {
            channels: vec![Channel {
                alternatives: vec![Alternatives {
                    transcript,
                    confidence: 1.0,
                    words: words
                        .into_iter()
                        .filter(|word| !word.word.trim().is_empty())
                        .map(|word| Word {
                            punctuated_word: Some(word.word.clone()),
                            word: word.word,
                            start: word.start,
                            end: word.end,
                            confidence: word.confidence,
                            channel: 0,
                            speaker: word.speaker,
                        })
                        .collect(),
                }],
            }],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_word_and_segment_response_shapes() {
        let word_payload: CompatibleResponse = serde_json::from_value(serde_json::json!({
            "text": "Hello world.",
            "words": [
                { "word": "Hello", "start": 0.1, "end": 0.4 },
                { "text": "world.", "start": 0.4, "end": 0.8, "speaker": "speaker_2" }
            ]
        }))
        .unwrap();
        let segment_payload: CompatibleResponse = serde_json::from_value(serde_json::json!({
            "segments": [{ "text": "Fallback segment.", "start": 1.0, "end": 2.0 }]
        }))
        .unwrap();

        let word_response = convert_response("groq", word_payload);
        let segment_response = convert_response("together", segment_payload);
        let word_alternative = &word_response.results.channels[0].alternatives[0];
        let segment_alternative = &segment_response.results.channels[0].alternatives[0];

        assert_eq!(word_alternative.transcript, "Hello world.");
        assert_eq!(word_alternative.words[1].speaker, Some(2));
        assert_eq!(segment_alternative.transcript, "Fallback segment.");
        assert_eq!(segment_alternative.words[0].start, 1.0);
    }
}
