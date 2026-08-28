use std::path::{Path, PathBuf};

use base64::Engine;
use owhisper_interface::ListenParams;
use owhisper_interface::batch::{Alternatives, Channel, Response, Results, Word};

use crate::adapter::http::{ensure_success, mime_type_from_extension};
use crate::adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware};
use crate::error::Error;
use crate::providers::Provider;

use super::GoogleGenerativeAiAdapter;

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
            "Gemini 3.5 Transcribe accepts inline audio up to 20 MB".to_string(),
        ));
    }

    let audio = tokio::fs::read(&file_path)
        .await
        .map_err(|error| Error::AudioProcessing(error.to_string()))?;
    let model = GoogleGenerativeAiAdapter::resolve_batch_model(params.model.as_deref());
    let language_codes = GoogleGenerativeAiAdapter::language_codes(&params.languages);
    let mut audio_transcription_config = serde_json::json!({
        "wordTimestamp": true,
        "diarization": true,
    });
    if !language_codes.is_empty() {
        audio_transcription_config["languageCodes"] = serde_json::json!(language_codes);
    }
    if !params.keywords.is_empty() {
        audio_transcription_config["customVocabulary"] = serde_json::json!(params.keywords);
    }

    let request = serde_json::json!({
        "contents": [{
            "parts": [{
                "inlineData": {
                    "mimeType": mime_type_from_extension(&file_path),
                    "data": base64::engine::general_purpose::STANDARD.encode(audio),
                }
            }]
        }],
        "generationConfig": {
            "audioTranscriptionConfig": audio_transcription_config
        }
    });

    let url = batch_url(api_base, model)?;
    let response = client
        .post(url.to_string())
        .header("x-goog-api-key", api_key)
        .json(&request)
        .send()
        .await?;
    let payload: GenerateContentResponse = ensure_success(response).await?.json().await?;
    Ok(convert_response(payload))
}

fn batch_url(api_base: &str, model: &str) -> Result<url::Url, Error> {
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

    let path = url.path().trim_end_matches('/');
    if !path.contains(":generateContent") {
        url.set_path(&format!("{path}/models/{model}:generateContent"));
    }
    Ok(url)
}

#[derive(serde::Deserialize)]
struct GenerateContentResponse {
    #[serde(default)]
    candidates: Vec<Candidate>,
}

#[derive(serde::Deserialize)]
struct Candidate {
    #[serde(default)]
    content: Option<Content>,
}

#[derive(serde::Deserialize)]
struct Content {
    #[serde(default)]
    parts: Vec<Part>,
}

#[derive(serde::Deserialize)]
struct Part {
    #[serde(default)]
    text: Option<String>,
    #[serde(default, rename = "audioTranscription")]
    audio_transcription: Option<AudioTranscription>,
}

#[derive(serde::Deserialize)]
struct AudioTranscription {
    #[serde(default)]
    text: Option<String>,
    #[serde(default, rename = "speakerLabel")]
    speaker_label: Option<String>,
    #[serde(default)]
    words: Vec<AudioWord>,
}

#[derive(serde::Deserialize)]
struct AudioWord {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    word: Option<String>,
    #[serde(default, rename = "startOffset")]
    start_offset: Option<String>,
    #[serde(default, rename = "endOffset")]
    end_offset: Option<String>,
    #[serde(default, rename = "speakerLabel")]
    speaker_label: Option<String>,
}

fn convert_response(payload: GenerateContentResponse) -> Response {
    let parts = payload
        .candidates
        .into_iter()
        .next()
        .and_then(|candidate| candidate.content)
        .map(|content| content.parts)
        .unwrap_or_default();

    let mut transcripts = Vec::new();
    let mut words = Vec::new();

    for part in parts {
        let speaker = part.audio_transcription.as_ref().and_then(|transcription| {
            GoogleGenerativeAiAdapter::parse_speaker_label(transcription.speaker_label.as_deref())
        });
        let transcript = part
            .text
            .as_deref()
            .or(part
                .audio_transcription
                .as_ref()
                .and_then(|transcription| transcription.text.as_deref()))
            .unwrap_or("")
            .trim();
        if !transcript.is_empty() {
            transcripts.push(transcript.to_string());
        }

        if let Some(transcription) = part.audio_transcription {
            words.extend(transcription.words.into_iter().filter_map(|word| {
                let text = word
                    .word
                    .or(word.text)
                    .map(|text| text.trim().to_string())
                    .filter(|text| !text.is_empty())?;
                Some(Word {
                    punctuated_word: Some(text.clone()),
                    word: text,
                    start: word
                        .start_offset
                        .as_deref()
                        .map(GoogleGenerativeAiAdapter::parse_duration_secs)
                        .unwrap_or_default(),
                    end: word
                        .end_offset
                        .as_deref()
                        .map(GoogleGenerativeAiAdapter::parse_duration_secs)
                        .unwrap_or_default(),
                    confidence: 1.0,
                    channel: 0,
                    speaker: GoogleGenerativeAiAdapter::parse_speaker_label(
                        word.speaker_label.as_deref(),
                    )
                    .or(speaker),
                })
            }));
        }
    }

    Response {
        metadata: serde_json::json!({ "provider": "google_generative_ai" }),
        results: Results {
            channels: vec![Channel {
                alternatives: vec![Alternatives {
                    transcript: transcripts.join(" "),
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
    fn builds_generate_content_url() {
        let url = batch_url(
            "https://generativelanguage.googleapis.com/v1beta",
            "gemini-3.5-transcribe",
        )
        .unwrap();
        assert_eq!(
            url.as_str(),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3.5-transcribe:generateContent"
        );
    }

    #[test]
    fn converts_diarized_words_and_text() {
        let payload: GenerateContentResponse = serde_json::from_value(serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "text": "Hello there.",
                        "audioTranscription": {
                            "text": "Hello there.",
                            "speakerLabel": "Speaker 2",
                            "words": [
                                {
                                    "word": "Hello",
                                    "startOffset": "0.100s",
                                    "endOffset": "0.500s",
                                    "speakerLabel": "Speaker 2"
                                },
                                {
                                    "text": "there.",
                                    "startOffset": "0.500s",
                                    "endOffset": "1.000s"
                                }
                            ]
                        }
                    }]
                }
            }]
        }))
        .unwrap();

        let response = convert_response(payload);
        let alternative = &response.results.channels[0].alternatives[0];
        assert_eq!(alternative.transcript, "Hello there.");
        assert_eq!(alternative.words[0].start, 0.1);
        assert_eq!(alternative.words[0].speaker, Some(2));
        assert_eq!(alternative.words[1].speaker, Some(2));
        assert_eq!(response.metadata["provider"], "google_generative_ai");
    }
}
