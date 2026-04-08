use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{collections::HashMap, mem};

use owhisper_interface::ListenParams;
use owhisper_interface::batch::{
    Alternatives as BatchAlternatives, Channel as BatchChannel, Response as BatchResponse,
    Results as BatchResults, Word as BatchWord,
};
use serde::{Deserialize, Serialize};

use super::AssemblyAIAdapter;
use super::language::BATCH_LANGUAGES;
use crate::adapter::http::ensure_success;
use crate::adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware, append_path_if_missing};
use crate::error::Error;
use crate::polling::{PollingConfig, PollingResult, poll_until};

// API
// https://www.assemblyai.com/docs/api-reference/transcripts/submit.md
// https://www.assemblyai.com/docs/api-reference/transcripts/get.md
// Model & Language
// https://www.assemblyai.com/docs/pre-recorded-audio/select-the-speech-model.md
impl BatchSttAdapter for AssemblyAIAdapter {
    fn provider_name(&self) -> &'static str {
        "assemblyai"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        let primary_lang = languages.first().map(|l| l.iso639().code()).unwrap_or("en");
        BATCH_LANGUAGES.contains(&primary_lang)
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
        Box::pin(Self::do_transcribe_file(
            client, api_base, api_key, params, path,
        ))
    }
}

#[derive(Debug, Serialize)]
struct TranscriptRequest {
    audio_url: String,
    speech_models: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language_detection: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    speaker_labels: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    speakers_expected: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    speaker_options: Option<SpeakerOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    multichannel: Option<bool>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    keyterms_prompt: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SpeakerOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    min_speakers_expected: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_speakers_expected: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct UploadResponse {
    upload_url: String,
}

#[derive(Debug, Deserialize)]
struct TranscriptResponse {
    id: String,
    status: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    words: Option<Vec<AssemblyAIBatchWord>>,
    #[serde(default)]
    #[allow(dead_code)]
    utterances: Option<Vec<Utterance>>,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    audio_duration: Option<u64>,
    #[serde(default)]
    audio_channels: Option<u32>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AssemblyAIBatchWord {
    text: String,
    start: u64,
    end: u64,
    confidence: f64,
    #[serde(default)]
    speaker: Option<String>,
    #[serde(default)]
    channel: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Utterance {
    #[serde(default)]
    text: String,
    #[serde(default)]
    start: u64,
    #[serde(default)]
    end: u64,
    #[serde(default)]
    confidence: f64,
    #[serde(default)]
    speaker: Option<String>,
    #[serde(default)]
    words: Vec<AssemblyAIBatchWord>,
}

impl AssemblyAIAdapter {
    fn resolve_batch_speech_models(params: &ListenParams) -> Vec<String> {
        match params.model.as_deref() {
            Some("u3-rt-pro" | "universal-3-pro") => {
                vec!["universal-3-pro".to_string(), "universal-2".to_string()]
            }
            Some(m) if !m.is_empty() && !crate::providers::is_meta_model(m) => {
                vec![m.to_string()]
            }
            _ => {
                vec!["universal-3-pro".to_string(), "universal-2".to_string()]
            }
        }
    }

    fn assemblyai_speaker_range_option(
        params: &ListenParams,
        value: Option<u32>,
        legacy_key: &str,
    ) -> Option<u32> {
        value.or_else(|| {
            params
                .custom_query
                .as_ref()
                .and_then(|query| query.get(legacy_key))
                .and_then(|value| value.parse().ok())
        })
    }

    async fn do_transcribe_file(
        client: &ClientWithMiddleware,
        api_base: &str,
        api_key: &str,
        params: &ListenParams,
        file_path: PathBuf,
    ) -> Result<BatchResponse, Error> {
        let base_url = Self::batch_api_url(api_base);

        let audio_data = tokio::fs::read(&file_path)
            .await
            .map_err(|e| Error::AudioProcessing(format!("failed to read file: {}", e)))?;

        let content_type = match file_path.extension().and_then(|e| e.to_str()) {
            Some("wav") => "audio/wav",
            Some("mp3") => "audio/mpeg",
            Some("ogg") => "audio/ogg",
            Some("flac") => "audio/flac",
            Some("m4a") => "audio/mp4",
            Some("webm") => "audio/webm",
            _ => "application/octet-stream",
        };

        let mut upload_url = base_url.clone();
        append_path_if_missing(&mut upload_url, "upload");
        let upload_response = client
            .post(upload_url.to_string())
            .header("Authorization", api_key)
            .header("Content-Type", content_type)
            .body(audio_data)
            .send()
            .await?;

        let upload_response = ensure_success(upload_response).await?;
        let upload_result: UploadResponse = upload_response.json().await?;

        let use_language_detection = params.languages.len() != 1;
        let language_code = if use_language_detection {
            None
        } else {
            params
                .languages
                .first()
                .map(|l| l.iso639().code().to_string())
        };
        let language_detection = if use_language_detection {
            Some(true)
        } else {
            None
        };

        let speech_models = Self::resolve_batch_speech_models(params);
        let speaker_options = match (
            Self::assemblyai_speaker_range_option(
                params,
                params.min_speakers,
                "pyannote_min_speakers",
            ),
            Self::assemblyai_speaker_range_option(
                params,
                params.max_speakers,
                "pyannote_max_speakers",
            ),
        ) {
            (None, None) => None,
            (min_speakers_expected, max_speakers_expected) => Some(SpeakerOptions {
                min_speakers_expected,
                max_speakers_expected,
            }),
        };

        let transcript_request = TranscriptRequest {
            audio_url: upload_result.upload_url,
            speech_models,
            language_code,
            language_detection,
            speaker_labels: Some(true),
            speakers_expected: params.num_speakers,
            speaker_options,
            multichannel: Some(params.channels > 1),
            keyterms_prompt: params.keywords.clone(),
        };

        let mut transcript_url = base_url.clone();
        append_path_if_missing(&mut transcript_url, "transcript");
        let create_response = client
            .post(transcript_url.to_string())
            .header("Authorization", api_key)
            .header("Content-Type", "application/json")
            .json(&transcript_request)
            .send()
            .await?;

        let create_response = ensure_success(create_response).await?;
        let create_result: TranscriptResponse = create_response.json().await?;
        let transcript_id = create_result.id;

        let mut poll_url = base_url.clone();
        append_path_if_missing(&mut poll_url, &format!("transcript/{transcript_id}"));

        let config = PollingConfig::default()
            .with_interval(Duration::from_secs(3))
            .with_timeout_error("transcription timed out".to_string());

        poll_until(
            || async {
                let poll_response = client
                    .get(poll_url.to_string())
                    .header("Authorization", api_key)
                    .send()
                    .await?;

                let poll_response = ensure_success(poll_response).await?;
                let result: TranscriptResponse = poll_response.json().await?;

                match result.status.as_str() {
                    "completed" => Ok(PollingResult::Complete(Self::convert_to_batch_response(
                        result,
                    ))),
                    "error" => {
                        let error_msg = result.error.unwrap_or_else(|| "unknown error".to_string());
                        Ok(PollingResult::Failed {
                            message: format!("transcription failed: {}", error_msg),
                            retryable: false,
                        })
                    }
                    _ => Ok(PollingResult::Continue),
                }
            },
            config,
        )
        .await
    }

    fn convert_word(
        w: AssemblyAIBatchWord,
        speaker_ids: &mut HashMap<String, usize>,
        next_speaker_id: &mut usize,
    ) -> BatchWord {
        let speaker = w.speaker.as_deref().map(|label| {
            *speaker_ids.entry(label.to_string()).or_insert_with(|| {
                let current = *next_speaker_id;
                *next_speaker_id += 1;
                current
            })
        });
        let channel = w
            .channel
            .as_deref()
            .and_then(|s| s.parse::<i32>().ok())
            .map(|channel| channel.saturating_sub(1))
            .unwrap_or(0)
            .max(0);

        BatchWord {
            word: w.text.clone(),
            start: w.start as f64 / 1000.0,
            end: w.end as f64 / 1000.0,
            confidence: w.confidence,
            channel,
            speaker,
            punctuated_word: Some(w.text),
        }
    }

    fn convert_to_batch_response(response: TranscriptResponse) -> BatchResponse {
        let mut all_words = response.words.unwrap_or_default();
        let num_channels = response.audio_channels.unwrap_or(1).max(1) as usize;
        let confidence = response.confidence.unwrap_or(1.0);
        let mut speaker_ids = HashMap::new();
        let mut next_speaker_id = 0;

        let channels = if num_channels <= 1 {
            let words: Vec<BatchWord> = all_words
                .into_iter()
                .map(|word| Self::convert_word(word, &mut speaker_ids, &mut next_speaker_id))
                .collect();
            let transcript = response.text.unwrap_or_default();
            vec![BatchChannel {
                alternatives: vec![BatchAlternatives {
                    transcript,
                    confidence,
                    words,
                }],
            }]
        } else {
            let mut channel_words: Vec<Vec<BatchWord>> = vec![Vec::new(); num_channels];
            for w in mem::take(&mut all_words) {
                let ch = w
                    .channel
                    .as_deref()
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(1)
                    .saturating_sub(1)
                    .min(num_channels - 1);
                channel_words[ch].push(Self::convert_word(
                    w,
                    &mut speaker_ids,
                    &mut next_speaker_id,
                ));
            }

            channel_words
                .into_iter()
                .map(|words| {
                    let transcript = words
                        .iter()
                        .map(|w| w.punctuated_word.as_deref().unwrap_or(&w.word))
                        .collect::<Vec<_>>()
                        .join(" ");
                    BatchChannel {
                        alternatives: vec![BatchAlternatives {
                            transcript,
                            confidence,
                            words,
                        }],
                    }
                })
                .collect()
        };

        BatchResponse {
            metadata: serde_json::json!({
                "audio_duration": response.audio_duration,
            }),
            results: BatchResults { channels },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_client::create_client;

    #[test]
    fn batch_defaults_expand_to_current_model_stack() {
        assert_eq!(
            AssemblyAIAdapter::resolve_batch_speech_models(&ListenParams::default()),
            vec!["universal-3-pro".to_string(), "universal-2".to_string()]
        );
    }

    #[test]
    fn batch_explicit_u3_models_expand_to_current_model_stack() {
        for model in ["u3-rt-pro", "universal-3-pro"] {
            let params = ListenParams {
                model: Some(model.to_string()),
                ..Default::default()
            };

            assert_eq!(
                AssemblyAIAdapter::resolve_batch_speech_models(&params),
                vec!["universal-3-pro".to_string(), "universal-2".to_string()]
            );
        }
    }

    #[test]
    fn batch_explicit_universal_2_is_preserved() {
        let params = ListenParams {
            model: Some("universal-2".to_string()),
            ..Default::default()
        };

        assert_eq!(
            AssemblyAIAdapter::resolve_batch_speech_models(&params),
            vec!["universal-2".to_string()]
        );
    }

    #[test]
    fn assemblyai_prefers_listen_params_speaker_range_fields() {
        let params = ListenParams {
            min_speakers: Some(2),
            max_speakers: Some(4),
            ..Default::default()
        };

        assert_eq!(
            AssemblyAIAdapter::assemblyai_speaker_range_option(
                &params,
                params.min_speakers,
                "pyannote_min_speakers",
            ),
            Some(2)
        );
        assert_eq!(
            AssemblyAIAdapter::assemblyai_speaker_range_option(
                &params,
                params.max_speakers,
                "pyannote_max_speakers",
            ),
            Some(4)
        );
    }

    #[test]
    fn assemblyai_falls_back_to_legacy_custom_query_speaker_range_keys() {
        let params = ListenParams {
            custom_query: Some(std::collections::HashMap::from([
                ("pyannote_min_speakers".to_string(), "2".to_string()),
                ("pyannote_max_speakers".to_string(), "4".to_string()),
            ])),
            ..Default::default()
        };

        assert_eq!(
            AssemblyAIAdapter::assemblyai_speaker_range_option(
                &params,
                params.min_speakers,
                "pyannote_min_speakers",
            ),
            Some(2)
        );
        assert_eq!(
            AssemblyAIAdapter::assemblyai_speaker_range_option(
                &params,
                params.max_speakers,
                "pyannote_max_speakers",
            ),
            Some(4)
        );
    }

    #[test]
    fn multichannel_words_are_normalized_to_zero_based_channels() {
        let response = TranscriptResponse {
            id: "id".to_string(),
            status: "completed".to_string(),
            text: None,
            words: Some(vec![
                AssemblyAIBatchWord {
                    text: "left-one".to_string(),
                    start: 0,
                    end: 500,
                    confidence: 0.9,
                    speaker: Some("1A".to_string()),
                    channel: Some("1".to_string()),
                },
                AssemblyAIBatchWord {
                    text: "left-two".to_string(),
                    start: 500,
                    end: 1000,
                    confidence: 0.85,
                    speaker: Some("1B".to_string()),
                    channel: Some("1".to_string()),
                },
                AssemblyAIBatchWord {
                    text: "right".to_string(),
                    start: 1000,
                    end: 1500,
                    confidence: 0.8,
                    speaker: Some("2A".to_string()),
                    channel: Some("2".to_string()),
                },
            ]),
            utterances: None,
            confidence: Some(0.85),
            audio_duration: Some(1),
            audio_channels: Some(2),
            error: None,
        };

        let result = AssemblyAIAdapter::convert_to_batch_response(response);

        assert_eq!(result.results.channels.len(), 2);
        assert_eq!(result.results.channels[0].alternatives[0].words.len(), 2);
        assert_eq!(
            result.results.channels[0].alternatives[0].words[0].channel,
            0
        );
        assert_eq!(
            result.results.channels[0].alternatives[0].words[0].speaker,
            Some(0)
        );
        assert_eq!(
            result.results.channels[0].alternatives[0].words[1].speaker,
            Some(1)
        );
        assert_eq!(
            result.results.channels[1].alternatives[0].words[0].channel,
            1
        );
        assert_eq!(
            result.results.channels[1].alternatives[0].words[0].speaker,
            Some(2)
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_assemblyai_batch_transcription() {
        let api_key = std::env::var("ASSEMBLYAI_API_KEY").expect("ASSEMBLYAI_API_KEY not set");
        let client = create_client();
        let adapter = AssemblyAIAdapter::default();
        let params = ListenParams::default();

        let audio_path = std::path::PathBuf::from(hypr_data::english_1::AUDIO_PATH);

        let result = adapter
            .transcribe_file(&client, "", &api_key, &params, &audio_path)
            .await
            .expect("transcription failed");

        assert!(!result.results.channels.is_empty());
        assert!(!result.results.channels[0].alternatives.is_empty());
        assert!(
            !result.results.channels[0].alternatives[0]
                .transcript
                .is_empty()
        );
        assert!(!result.results.channels[0].alternatives[0].words.is_empty());
    }
}
