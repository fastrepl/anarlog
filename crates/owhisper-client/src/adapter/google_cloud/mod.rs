use std::path::{Path, PathBuf};

use base64::Engine;
use owhisper_interface::ListenParams;
use owhisper_interface::batch::{Alternatives, Channel, Response, Results, Word};

use crate::adapter::http::ensure_success;
use crate::adapter::{
    BatchFuture, BatchSttAdapter, ClientWithMiddleware, LanguageQuality, LanguageSupport,
    append_path_if_missing,
};
use crate::error::Error;

const INLINE_AUDIO_LIMIT_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Clone, Default)]
pub struct GoogleCloudAdapter;

impl GoogleCloudAdapter {
    pub fn language_support_batch(_languages: &[anlg_language::Language]) -> LanguageSupport {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    }
}

impl BatchSttAdapter for GoogleCloudAdapter {
    fn provider_name(&self) -> &'static str {
        "google_cloud"
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
    access_token: &str,
    params: &ListenParams,
    file_path: PathBuf,
) -> Result<Response, Error> {
    let metadata = tokio::fs::metadata(&file_path)
        .await
        .map_err(|error| Error::AudioProcessing(error.to_string()))?;
    if metadata.len() > INLINE_AUDIO_LIMIT_BYTES {
        return Err(Error::AudioProcessing(
            "Google Cloud synchronous transcription accepts inline audio up to 10 MB and one minute"
                .to_string(),
        ));
    }
    let audio = tokio::fs::read(&file_path)
        .await
        .map_err(|error| Error::AudioProcessing(error.to_string()))?;
    let language_code = params
        .languages
        .first()
        .map(anlg_language::Language::bcp47_code)
        .filter(|code| !code.is_empty())
        .unwrap_or_else(|| "en-US".to_string());
    let model = match params.model.as_deref() {
        Some(model) if !crate::providers::is_meta_model(model) => model,
        _ => "latest_long",
    };
    let mut config = serde_json::json!({
        "languageCode": language_code,
        "model": model,
        "enableAutomaticPunctuation": true,
        "enableWordTimeOffsets": true,
        "enableWordConfidence": true,
        "audioChannelCount": params.channels,
        "enableSeparateRecognitionPerChannel": params.channels > 1,
    });
    if params.num_speakers.is_some()
        || params.min_speakers.is_some()
        || params.max_speakers.is_some()
    {
        config["diarizationConfig"] = serde_json::json!({
            "enableSpeakerDiarization": true,
            "minSpeakerCount": params.min_speakers.or(params.num_speakers),
            "maxSpeakerCount": params.max_speakers.or(params.num_speakers),
        });
    }
    if !params.keywords.is_empty() {
        config["speechContexts"] = serde_json::json!([{
            "phrases": params.keywords,
        }]);
    }
    let request = serde_json::json!({
        "config": config,
        "audio": {
            "content": base64::engine::general_purpose::STANDARD.encode(audio),
        },
    });
    let mut url: url::Url = if api_base.is_empty() {
        "https://speech.googleapis.com/v1"
            .parse()
            .expect("invalid_default_google_cloud_api_base")
    } else {
        api_base.parse().map_err(|error: url::ParseError| {
            Error::AudioProcessing(format!("invalid api_base: {error}"))
        })?
    };
    append_path_if_missing(&mut url, "speech:recognize");

    let response = client
        .post(url.to_string())
        .bearer_auth(access_token)
        .json(&request)
        .send()
        .await?;
    let payload: GoogleCloudResponse = ensure_success(response).await?.json().await?;

    Ok(convert_response(payload))
}

#[derive(serde::Deserialize)]
struct GoogleCloudResponse {
    #[serde(default)]
    results: Vec<GoogleCloudResult>,
}

#[derive(serde::Deserialize)]
struct GoogleCloudResult {
    #[serde(default)]
    alternatives: Vec<GoogleCloudAlternative>,
    #[serde(default, rename = "channelTag")]
    channel_tag: Option<i32>,
}

#[derive(serde::Deserialize)]
struct GoogleCloudAlternative {
    #[serde(default)]
    transcript: String,
    #[serde(default)]
    confidence: f64,
    #[serde(default)]
    words: Vec<GoogleCloudWord>,
}

#[derive(serde::Deserialize)]
struct GoogleCloudWord {
    word: String,
    #[serde(default, rename = "startTime")]
    start_time: String,
    #[serde(default, rename = "endTime")]
    end_time: String,
    #[serde(default)]
    confidence: f64,
    #[serde(default, rename = "speakerTag")]
    speaker_tag: Option<usize>,
}

fn parse_duration(value: &str) -> f64 {
    value
        .strip_suffix('s')
        .and_then(|value| value.parse().ok())
        .unwrap_or_default()
}

fn convert_response(payload: GoogleCloudResponse) -> Response {
    let channel_count = payload
        .results
        .iter()
        .filter_map(|result| result.channel_tag)
        .max()
        .unwrap_or(1)
        .max(1);
    let channels = (1..=channel_count)
        .map(|channel_tag| {
            let matching_results = payload
                .results
                .iter()
                .filter(|result| result.channel_tag.unwrap_or(1) == channel_tag)
                .collect::<Vec<_>>();
            let transcript = matching_results
                .iter()
                .filter_map(|result| result.alternatives.first())
                .map(|alternative| alternative.transcript.trim())
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            let confidence = matching_results
                .first()
                .and_then(|result| result.alternatives.first())
                .map(|alternative| alternative.confidence)
                .unwrap_or(1.0);
            let words = matching_results
                .into_iter()
                .filter_map(|result| result.alternatives.first())
                .flat_map(|alternative| {
                    alternative.words.iter().map(|word| Word {
                        punctuated_word: Some(word.word.clone()),
                        word: word.word.clone(),
                        start: parse_duration(&word.start_time),
                        end: parse_duration(&word.end_time),
                        confidence: word.confidence,
                        channel: channel_tag - 1,
                        speaker: word.speaker_tag,
                    })
                })
                .collect();

            Channel {
                alternatives: vec![Alternatives {
                    transcript,
                    confidence,
                    words,
                }],
            }
        })
        .collect();

    Response {
        metadata: serde_json::json!({ "provider": "google_cloud" }),
        results: Results { channels },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_google_durations_and_channels() {
        let payload: GoogleCloudResponse = serde_json::from_value(serde_json::json!({
            "results": [{
                "channelTag": 1,
                "alternatives": [{
                    "transcript": "hello",
                    "confidence": 0.9,
                    "words": [{
                        "word": "hello",
                        "startTime": "0.100s",
                        "endTime": "0.500s",
                        "confidence": 0.8,
                        "speakerTag": 2
                    }]
                }]
            }]
        }))
        .unwrap();

        let response = convert_response(payload);
        let word = &response.results.channels[0].alternatives[0].words[0];

        assert_eq!(word.start, 0.1);
        assert_eq!(word.channel, 0);
        assert_eq!(word.speaker, Some(2));
    }
}
