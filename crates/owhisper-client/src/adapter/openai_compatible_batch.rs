use std::path::Path;

use owhisper_interface::ListenParams;
use owhisper_interface::batch::{Alternatives, Channel, Response, Results, Word};
use reqwest::multipart::Form;
use serde::Deserialize;

use crate::adapter::http::{ensure_success, streaming_file_part};
use crate::adapter::{ClientWithMiddleware, MIXED_CAPTURE_CHANNEL, append_path_if_missing};
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

pub(crate) fn resolve_model<'a>(params: &'a ListenParams, default_model: &'a str) -> &'a str {
    match params.model.as_deref() {
        Some(model) if !crate::providers::is_meta_model(model) => model,
        _ => default_model,
    }
}

pub(crate) async fn transcribe(
    client: &ClientWithMiddleware,
    api_base: &str,
    api_key: &str,
    params: &ListenParams,
    file_path: &Path,
    config: OpenAICompatibleBatchConfig<'_>,
) -> Result<Response, Error> {
    let model = resolve_model(params, config.default_model);

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

    let mut request = client.post(url.to_string()).bearer_auth(api_key);
    if config.provider == "openrouter" {
        request = super::openrouter::with_attribution_headers(request);
    }
    let response = request.multipart(form).send().await?;

    parse_response(config.provider, response).await
}

/// Reads and converts a provider response, shared by the multipart path above
/// and OpenRouter's JSON `input_audio` + `provider.options` path, which needs
/// the same response shape but a different request encoding.
pub(crate) async fn parse_response(
    provider: &str,
    response: reqwest::Response,
) -> Result<Response, Error> {
    let payload: CompatibleResponse = ensure_success(response).await?.json().await?;
    Ok(convert_response(provider, payload))
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
    /// Some providers diarize at segment/phrase granularity rather than per
    /// word (e.g. Azure, whose native diarization is phrase-scoped — see
    /// `adapter::azure_speech`, which propagates `phrase.speaker` onto every
    /// word in that phrase). `convert_response` backfills word-level speaker
    /// from this when a word has none of its own.
    #[serde(default, deserialize_with = "deserialize_optional_speaker")]
    speaker: Option<usize>,
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
    let CompatibleResponse {
        text,
        words: top_level_words,
        segments,
    } = payload;

    let mut words = if top_level_words.is_empty() {
        segments
            .iter()
            .flat_map(|segment| {
                if segment.words.is_empty() {
                    vec![CompatibleWord {
                        word: segment.text.clone(),
                        start: segment.start,
                        end: segment.end,
                        confidence: 1.0,
                        speaker: segment.speaker,
                    }]
                } else {
                    segment.words.clone()
                }
            })
            .collect()
    } else {
        top_level_words
    };

    // A flat top-level `words` array (needed for word timestamps) commonly
    // carries no speaker of its own even when segments are diarized — backfill
    // each word's speaker from whichever segment it temporally falls within.
    for word in &mut words {
        if word.speaker.is_some() {
            continue;
        }
        if let Some(segment) = segments.iter().find(|segment| {
            segment.speaker.is_some() && word.start >= segment.start && word.start < segment.end
        }) {
            word.speaker = segment.speaker;
        }
    }

    let transcript = if text.trim().is_empty() {
        segments
            .iter()
            .map(|segment| segment.text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        text.trim().to_string()
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
                            // Channel 0 (DirectMic) is treated downstream as
                            // always exactly one speaker (a real single mic),
                            // so a diarized word from a single mixed-audio
                            // batch upload must use the mixed-capture channel
                            // instead, or the render pipeline collapses every
                            // provider speaker back into one label.
                            channel: if word.speaker.is_some() {
                                MIXED_CAPTURE_CHANNEL
                            } else {
                                0
                            },
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

    #[test]
    fn backfills_word_speaker_from_segment_when_only_segments_are_diarized() {
        // Mirrors Azure's real diarization shape (phrase/segment-scoped, not
        // per-word) as routed through OpenRouter: a flat top-level `words`
        // array with no speaker of its own, alongside diarized `segments`.
        let payload: CompatibleResponse = serde_json::from_value(serde_json::json!({
            "text": "Hello there. Hi, how are you?",
            "segments": [
                { "id": 0, "start": 0.0, "end": 1.2, "text": "Hello there.", "speaker": 0 },
                { "id": 1, "start": 1.5, "end": 3.1, "text": "Hi, how are you?", "speaker": 1 }
            ],
            "words": [
                { "word": "Hello", "start": 0.0, "end": 0.4 },
                { "word": "there.", "start": 0.4, "end": 1.2 },
                { "word": "Hi,", "start": 1.5, "end": 1.9 },
                { "word": "how", "start": 1.9, "end": 2.2 }
            ]
        }))
        .unwrap();

        let response = convert_response("openrouter", payload);
        let words = &response.results.channels[0].alternatives[0].words;

        assert_eq!(words[0].speaker, Some(0));
        assert_eq!(words[1].speaker, Some(0));
        assert_eq!(words[2].speaker, Some(1));
        assert_eq!(words[3].speaker, Some(1));
        assert!(
            words
                .iter()
                .all(|word| word.channel == MIXED_CAPTURE_CHANNEL)
        );
    }

    #[test]
    fn leaves_word_speaker_untouched_when_the_provider_already_set_it() {
        let payload: CompatibleResponse = serde_json::from_value(serde_json::json!({
            "text": "Hello world.",
            "segments": [{ "start": 0.0, "end": 1.0, "text": "Hello world.", "speaker": 5 }],
            "words": [{ "word": "Hello", "start": 0.0, "end": 0.4, "speaker": 1 }]
        }))
        .unwrap();

        let response = convert_response("openrouter", payload);
        let words = &response.results.channels[0].alternatives[0].words;

        assert_eq!(words[0].speaker, Some(1));
        assert_eq!(words[0].channel, MIXED_CAPTURE_CHANNEL);
    }

    #[test]
    fn undiarized_words_stay_on_channel_zero() {
        let payload: CompatibleResponse = serde_json::from_value(serde_json::json!({
            "text": "Hello world.",
            "words": [{ "word": "Hello", "start": 0.0, "end": 0.4 }]
        }))
        .unwrap();

        let response = convert_response("groq", payload);
        let words = &response.results.channels[0].alternatives[0].words;

        assert_eq!(words[0].speaker, None);
        assert_eq!(words[0].channel, 0);
    }
}
