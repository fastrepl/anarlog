use std::path::{Path, PathBuf};

use owhisper_interface::ListenParams;
use owhisper_interface::batch::{Alternatives, Channel, Response, Results, Word};
use reqwest::multipart::Form;

use crate::adapter::http::{ensure_success, streaming_file_part};
use crate::adapter::{
    BatchFuture, BatchSttAdapter, ClientWithMiddleware, LanguageQuality, LanguageSupport,
    append_path_if_missing,
};
use crate::error::Error;

const AZURE_MIN_SPEAKERS: u32 = 2;
const AZURE_MAX_SPEAKERS: u32 = 35;
const AZURE_DEFAULT_MAX_SPEAKERS: u32 = 8;

#[derive(Clone, Default)]
pub struct AzureSpeechAdapter;

impl AzureSpeechAdapter {
    pub fn language_support_batch(_languages: &[anlg_language::Language]) -> LanguageSupport {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    }
}

impl BatchSttAdapter for AzureSpeechAdapter {
    fn provider_name(&self) -> &'static str {
        "azure_speech"
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
    if api_base.is_empty() {
        return Err(Error::AudioProcessing(
            "Azure AI Speech requires a regional resource endpoint".to_string(),
        ));
    }

    let definition = transcription_definition(params);
    let form = Form::new()
        .text("definition", definition.to_string())
        .part("audio", streaming_file_part(&file_path).await?);
    let mut url: url::Url = api_base.parse().map_err(|error: url::ParseError| {
        Error::AudioProcessing(format!("invalid api_base: {error}"))
    })?;
    append_path_if_missing(&mut url, "speechtotext/transcriptions:transcribe");
    url.query_pairs_mut()
        .append_pair("api-version", "2025-10-15");

    let response = client
        .post(url.to_string())
        .header("Ocp-Apim-Subscription-Key", api_key)
        .multipart(form)
        .send()
        .await?;
    let payload: AzureSpeechResponse = ensure_success(response).await?.json().await?;

    Ok(convert_response(payload))
}

fn transcription_definition(params: &ListenParams) -> serde_json::Value {
    let locales = params
        .languages
        .iter()
        .map(anlg_language::Language::bcp47_code)
        .collect::<Vec<_>>();
    let mut definition = if locales.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::json!({ "locales": locales })
    };
    // Fast transcription leaves diarization off unless the definition asks for it,
    // which collapses every meeting participant onto a single speaker label.
    definition["diarization"] = serde_json::json!({
        "enabled": true,
        "maxSpeakers": azure_max_speakers(params),
    });
    definition
}

fn azure_max_speakers(params: &ListenParams) -> u32 {
    params
        .max_speakers
        .or(params.num_speakers)
        .unwrap_or(AZURE_DEFAULT_MAX_SPEAKERS)
        .clamp(AZURE_MIN_SPEAKERS, AZURE_MAX_SPEAKERS)
}

#[derive(serde::Deserialize)]
struct AzureSpeechResponse {
    #[serde(default, rename = "durationMilliseconds")]
    duration_ms: u64,
    #[serde(default, rename = "combinedPhrases")]
    combined_phrases: Vec<AzureCombinedPhrase>,
    #[serde(default)]
    phrases: Vec<AzurePhrase>,
}

#[derive(serde::Deserialize)]
struct AzureCombinedPhrase {
    #[serde(default)]
    text: String,
    #[serde(default)]
    channel: Option<i32>,
}

#[derive(serde::Deserialize)]
struct AzurePhrase {
    #[serde(default)]
    confidence: f64,
    #[serde(default)]
    speaker: Option<usize>,
    #[serde(default)]
    channel: Option<i32>,
    #[serde(default)]
    words: Vec<AzureWord>,
}

#[derive(serde::Deserialize)]
struct AzureWord {
    text: String,
    #[serde(rename = "offsetMilliseconds")]
    offset_ms: u64,
    #[serde(rename = "durationMilliseconds")]
    duration_ms: u64,
}

fn convert_response(payload: AzureSpeechResponse) -> Response {
    let channel_count = payload
        .combined_phrases
        .iter()
        .filter_map(|phrase| phrase.channel)
        .max()
        .map(|channel| channel + 1)
        .unwrap_or(1)
        .max(1);
    let mut channels = (0..channel_count)
        .map(|channel_index| {
            let transcript = payload
                .combined_phrases
                .iter()
                .find(|phrase| phrase.channel.unwrap_or(0) == channel_index)
                .map(|phrase| phrase.text.trim().to_string())
                .unwrap_or_default();
            let words = payload
                .phrases
                .iter()
                .filter(|phrase| phrase.channel.unwrap_or(0) == channel_index)
                .flat_map(|phrase| {
                    phrase.words.iter().map(|word| Word {
                        punctuated_word: Some(word.text.clone()),
                        word: word.text.clone(),
                        start: word.offset_ms as f64 / 1_000.0,
                        end: (word.offset_ms + word.duration_ms) as f64 / 1_000.0,
                        confidence: phrase.confidence,
                        channel: channel_index,
                        speaker: phrase.speaker,
                    })
                })
                .collect();

            Channel {
                alternatives: vec![Alternatives {
                    transcript,
                    confidence: 1.0,
                    words,
                }],
            }
        })
        .collect::<Vec<_>>();
    if channels.is_empty() {
        channels.push(Channel {
            alternatives: vec![Alternatives {
                transcript: String::new(),
                confidence: 1.0,
                words: Vec::new(),
            }],
        });
    }

    Response {
        metadata: serde_json::json!({
            "provider": "azure_speech",
            "duration_ms": payload.duration_ms,
        }),
        results: Results { channels },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_fast_transcription_response() {
        let payload: AzureSpeechResponse = serde_json::from_value(serde_json::json!({
            "durationMilliseconds": 2000,
            "combinedPhrases": [{ "text": "Weather" }],
            "phrases": [{
                "confidence": 0.8,
                "speaker": 1,
                "words": [{
                    "text": "weather",
                    "offsetMilliseconds": 40,
                    "durationMilliseconds": 320
                }]
            }]
        }))
        .unwrap();

        let response = convert_response(payload);
        let alternative = &response.results.channels[0].alternatives[0];

        assert_eq!(alternative.transcript, "Weather");
        assert_eq!(alternative.words[0].start, 0.04);
        assert_eq!(alternative.words[0].speaker, Some(1));
    }

    #[test]
    fn converts_diarized_phrases_to_per_word_speakers() {
        let payload: AzureSpeechResponse = serde_json::from_value(serde_json::json!({
            "durationMilliseconds": 1800,
            "combinedPhrases": [{ "text": "Hello there" }],
            "phrases": [
                {
                    "confidence": 0.9,
                    "speaker": 0,
                    "words": [{
                        "text": "Hello",
                        "offsetMilliseconds": 0,
                        "durationMilliseconds": 400
                    }]
                },
                {
                    "confidence": 0.8,
                    "speaker": 1,
                    "words": [{
                        "text": "there",
                        "offsetMilliseconds": 500,
                        "durationMilliseconds": 300
                    }]
                }
            ]
        }))
        .unwrap();

        let words = &convert_response(payload).results.channels[0].alternatives[0].words;
        assert_eq!(words[0].speaker, Some(0));
        assert_eq!(words[1].speaker, Some(1));
    }

    #[test]
    fn definition_enables_diarization_by_default() {
        let definition = transcription_definition(&ListenParams::default());

        assert_eq!(
            definition,
            serde_json::json!({
                "diarization": {
                    "enabled": true,
                    "maxSpeakers": AZURE_DEFAULT_MAX_SPEAKERS
                }
            })
        );
    }

    #[test]
    fn definition_uses_speaker_hint_and_locales() {
        let definition = transcription_definition(&ListenParams {
            languages: vec![anlg_language::ISO639::En.into()],
            num_speakers: Some(3),
            max_speakers: Some(5),
            ..ListenParams::default()
        });

        assert_eq!(
            definition,
            serde_json::json!({
                "locales": ["en"],
                "diarization": {
                    "enabled": true,
                    "maxSpeakers": 5
                }
            })
        );
    }

    #[test]
    fn max_speakers_clamps_to_azure_range() {
        assert_eq!(
            azure_max_speakers(&ListenParams {
                num_speakers: Some(1),
                ..ListenParams::default()
            }),
            AZURE_MIN_SPEAKERS
        );
        assert_eq!(
            azure_max_speakers(&ListenParams {
                max_speakers: Some(100),
                ..ListenParams::default()
            }),
            AZURE_MAX_SPEAKERS
        );
        assert_eq!(
            azure_max_speakers(&ListenParams {
                num_speakers: Some(4),
                ..ListenParams::default()
            }),
            4
        );
    }
}
