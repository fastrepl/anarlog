// https://docs.gladia.io/api-reference/v2/pre-recorded/init

use std::path::{Path, PathBuf};
use std::time::Duration;

use owhisper_interface::ListenParams;
use owhisper_interface::batch::{
    Alternatives as BatchAlternatives, Channel as BatchChannel, Response as BatchResponse,
    Results as BatchResults, Word as BatchWord,
};
use serde::{Deserialize, Serialize};

use super::GladiaAdapter;
use crate::adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware, append_path_if_missing};
use crate::error::Error;
use crate::polling::{PollingConfig, PollingResult, poll_until};

impl BatchSttAdapter for GladiaAdapter {
    fn provider_name(&self) -> &'static str {
        "gladia"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        GladiaAdapter::is_supported_languages_batch(languages)
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
struct TranscriptRequest<'a> {
    audio_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language_config: Option<LanguageConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diarization: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    custom_vocabulary: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name_consistency: Option<bool>,
}

#[derive(Debug, Serialize)]
struct LanguageConfig {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    languages: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code_switching: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct UploadResponse {
    audio_url: String,
}

#[derive(Debug, Deserialize)]
struct InitResponse {
    id: String,
}

#[derive(Debug, Deserialize)]
struct TranscriptResponse {
    status: String,
    #[serde(default)]
    error_code: Option<String>,
    #[serde(default)]
    file: Option<FileInfo>,
    #[serde(default)]
    result: Option<TranscriptResult>,
}

#[derive(Debug, Deserialize)]
struct FileInfo {
    #[serde(default)]
    audio_duration: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
struct TranscriptResult {
    #[serde(default)]
    metadata: Option<ResultMetadata>,
    #[serde(default)]
    transcription: Option<Transcription>,
}

#[derive(Debug, Deserialize)]
struct ResultMetadata {
    #[serde(default)]
    audio_duration: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
struct Transcription {
    #[serde(default)]
    full_transcript: Option<String>,
    #[serde(default)]
    utterances: Vec<Utterance>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Utterance {
    text: String,
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
    #[serde(default)]
    confidence: f64,
    #[serde(default)]
    channel: usize,
    #[serde(default)]
    speaker: Option<usize>,
    #[serde(default)]
    words: Vec<GladiaWord>,
}

#[derive(Debug, Deserialize)]
struct GladiaWord {
    word: String,
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
    #[serde(default)]
    confidence: f64,
}

impl GladiaAdapter {
    async fn do_transcribe_file(
        client: &ClientWithMiddleware,
        api_base: &str,
        api_key: &str,
        params: &ListenParams,
        file_path: PathBuf,
    ) -> Result<BatchResponse, Error> {
        let base_url = Self::batch_api_url(api_base);

        let file_bytes = tokio::fs::read(&file_path)
            .await
            .map_err(|e| Error::AudioProcessing(format!("failed to read file: {}", e)))?;

        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.wav")
            .to_string();

        let mime_type = match file_path.extension().and_then(|e| e.to_str()) {
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
        let form = reqwest::multipart::Form::new().part(
            "audio",
            reqwest::multipart::Part::bytes(file_bytes)
                .file_name(file_name)
                .mime_str(mime_type)
                .map_err(|e| Error::AudioProcessing(e.to_string()))?,
        );

        let upload_response = client
            .post(upload_url.to_string())
            .header("x-gladia-key", api_key)
            .multipart(form)
            .send()
            .await?;

        let upload_status = upload_response.status();
        if !upload_status.is_success() {
            return Err(Error::UnexpectedStatus {
                status: upload_status,
                body: upload_response.text().await.unwrap_or_default(),
            });
        }

        let upload_result: UploadResponse = upload_response.json().await?;

        let languages: Vec<String> = params
            .languages
            .iter()
            .map(|l| l.iso639().code().to_string())
            .collect();

        let language_config = (!languages.is_empty()).then(|| LanguageConfig {
            languages,
            code_switching: (params.languages.len() > 1).then_some(true),
        });

        let custom_vocabulary = (!params.keywords.is_empty()).then(|| params.keywords.clone());

        let default = crate::providers::Provider::Gladia.default_batch_model();
        let model = match params.model.as_deref() {
            Some(m) if crate::providers::is_meta_model(m) => Some(default),
            Some(m) => Some(m),
            None => None,
        };

        let transcript_request = TranscriptRequest {
            audio_url: upload_result.audio_url,
            model,
            language_config,
            diarization: Some(true),
            custom_vocabulary,
            name_consistency: Some(true),
        };

        let mut transcript_url = base_url.clone();
        append_path_if_missing(&mut transcript_url, "pre-recorded");
        let create_response = client
            .post(transcript_url.to_string())
            .header("x-gladia-key", api_key)
            .header("Content-Type", "application/json")
            .json(&transcript_request)
            .send()
            .await?;

        let create_status = create_response.status();
        if !create_status.is_success() {
            return Err(Error::UnexpectedStatus {
                status: create_status,
                body: create_response.text().await.unwrap_or_default(),
            });
        }

        let create_result: InitResponse = create_response.json().await?;
        let transcript_id = create_result.id;

        let mut poll_url = base_url.clone();
        append_path_if_missing(&mut poll_url, &format!("pre-recorded/{transcript_id}"));

        let config = PollingConfig::default()
            .with_interval(Duration::from_secs(3))
            .with_timeout_error("transcription timed out".to_string());

        poll_until(
            || async {
                let poll_response = client
                    .get(poll_url.to_string())
                    .header("x-gladia-key", api_key)
                    .send()
                    .await?;

                let poll_status = poll_response.status();
                if !poll_status.is_success() {
                    return Err(Error::UnexpectedStatus {
                        status: poll_status,
                        body: poll_response.text().await.unwrap_or_default(),
                    });
                }

                let result: TranscriptResponse = poll_response.json().await?;

                match result.status.as_str() {
                    "done" => Ok(PollingResult::Complete(Self::convert_to_batch_response(
                        result,
                    ))),
                    "error" => {
                        let error_msg = result
                            .error_code
                            .unwrap_or_else(|| "unknown error".to_string());
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

    fn convert_to_batch_response(response: TranscriptResponse) -> BatchResponse {
        let result = response.result.unwrap_or_default();
        let transcription = result.transcription.unwrap_or_default();

        let words: Vec<BatchWord> = transcription
            .utterances
            .iter()
            .flat_map(|u| {
                u.words.iter().map(|w| {
                    let trimmed = w.word.trim().to_string();
                    BatchWord {
                        word: trimmed.clone(),
                        start: w.start,
                        end: w.end,
                        confidence: w.confidence,
                        speaker: u.speaker,
                        punctuated_word: Some(trimmed),
                    }
                })
            })
            .collect();

        let transcript = transcription.full_transcript.unwrap_or_default();

        let avg_confidence = if words.is_empty() {
            1.0
        } else {
            words.iter().map(|w| w.confidence).sum::<f64>() / words.len() as f64
        };

        let channel = BatchChannel {
            alternatives: vec![BatchAlternatives {
                transcript,
                confidence: avg_confidence,
                words,
            }],
        };

        let audio_duration = result
            .metadata
            .and_then(|m| m.audio_duration)
            .or_else(|| response.file.and_then(|f| f.audio_duration));

        BatchResponse {
            metadata: serde_json::json!({
                "audio_duration": audio_duration,
            }),
            results: BatchResults {
                channels: vec![channel],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_client::create_client;

    #[tokio::test]
    #[ignore]
    async fn test_gladia_batch_transcription() {
        let api_key = std::env::var("GLADIA_API_KEY").expect("GLADIA_API_KEY not set");
        let client = create_client();
        let adapter = GladiaAdapter::default();
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
