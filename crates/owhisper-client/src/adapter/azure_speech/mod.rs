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

    let locales = params
        .languages
        .iter()
        .map(anlg_language::Language::bcp47_code)
        .collect::<Vec<_>>();
    let definition = if locales.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::json!({ "locales": locales })
    };
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
}
