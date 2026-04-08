use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use owhisper_interface::ListenParams;
use owhisper_interface::batch::{
    Alternatives as BatchAlternatives, Channel as BatchChannel, Response as BatchResponse,
    Results as BatchResults, Word as BatchWord,
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use super::model::PyannoteDiarizationModel;
use super::{PyannoteAdapter, PyannoteTranscriptionModel};
use crate::adapter::http::{ensure_success, mime_type_from_extension};
use crate::adapter::parsing::parse_speaker_id;
use crate::adapter::{
    BatchFuture, BatchSttAdapter, ClientWithMiddleware, MIXED_CAPTURE_CHANNEL,
    append_path_if_missing,
};
use crate::error::Error;
use crate::polling::{PollingConfig, PollingResult, poll_until};

impl BatchSttAdapter for PyannoteAdapter {
    fn provider_name(&self) -> &'static str {
        "pyannote"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        PyannoteAdapter::language_support_batch(languages, model).is_supported()
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
struct MediaUrlRequest {
    url: String,
}

#[derive(Debug, Deserialize)]
struct MediaResponse {
    url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiarizeRequest {
    url: String,
    model: PyannoteDiarizationModel,
    transcription: bool,
    transcription_config: TranscriptionConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_speakers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_speakers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_speakers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    turn_level_confidence: Option<bool>,
}

#[derive(Debug, Serialize)]
struct TranscriptionConfig {
    model: PyannoteTranscriptionModel,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobCreated {
    job_id: String,
    status: String,
    warning: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobResponse {
    job_id: String,
    status: String,
    output: Option<DiarizationJobOutput>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiarizationJobOutput {
    #[serde(default)]
    confidence: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    warning: Option<String>,
    #[serde(default)]
    diarization: Vec<DiarizationSegment>,
    #[serde(default)]
    exclusive_diarization: Vec<DiarizationSegment>,
    #[serde(default)]
    turn_level_transcription: Vec<TranscriptionSegment>,
    #[serde(default)]
    word_level_transcription: Vec<TranscriptionSegment>,
}

#[derive(Debug, Deserialize, Serialize)]
struct DiarizationSegment {
    speaker: String,
    start: f64,
    end: f64,
    #[serde(default)]
    confidence: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TranscriptionSegment {
    speaker: String,
    start: f64,
    end: f64,
    text: String,
}

impl PyannoteAdapter {
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

        let media_url = Self::create_media_url(&file_path);
        let upload_url = Self::create_upload_url(client, &base_url, api_key, &media_url).await?;
        Self::upload_audio(client, &upload_url, &file_path, file_bytes).await?;

        let job = Self::submit_job(client, &base_url, api_key, params, &media_url).await?;
        tracing::info!(
            hyprnote.stt.job.id = %job.job_id,
            status = %job.status,
            warning = ?job.warning,
            "pyannote_job_created"
        );

        Self::wait_for_job(client, &base_url, api_key, &job.job_id).await
    }

    fn create_media_url(file_path: &Path) -> String {
        let file_name = file_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("audio.wav");
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        format!("media://owhisper/{}-{}-{file_name}", std::process::id(), ts)
    }

    async fn create_upload_url(
        client: &ClientWithMiddleware,
        base_url: &url::Url,
        api_key: &str,
        media_url: &str,
    ) -> Result<String, Error> {
        let mut url = base_url.clone();
        append_path_if_missing(&mut url, "media/input");

        let response = client
            .post(url.to_string())
            .bearer_auth(api_key)
            .json(&MediaUrlRequest {
                url: media_url.to_string(),
            })
            .send()
            .await?;

        let response = ensure_success(response).await?;
        let body: MediaResponse = response.json().await?;
        Ok(body.url)
    }

    async fn upload_audio(
        client: &ClientWithMiddleware,
        upload_url: &str,
        file_path: &Path,
        file_bytes: Vec<u8>,
    ) -> Result<(), Error> {
        let response = client
            .put(upload_url)
            .header("Content-Type", mime_type_from_extension(file_path))
            .body(file_bytes)
            .send()
            .await?;

        ensure_success(response).await?;
        Ok(())
    }

    async fn submit_job(
        client: &ClientWithMiddleware,
        base_url: &url::Url,
        api_key: &str,
        params: &ListenParams,
        media_url: &str,
    ) -> Result<JobCreated, Error> {
        let mut url = base_url.clone();
        append_path_if_missing(&mut url, "diarize");

        let response = client
            .post(url.to_string())
            .bearer_auth(api_key)
            .json(&DiarizeRequest {
                url: media_url.to_string(),
                model: PyannoteDiarizationModel::Precision2,
                transcription: true,
                transcription_config: TranscriptionConfig {
                    model: Self::resolve_transcription_model(params.model.as_deref()),
                },
                max_speakers: Self::pyannote_speaker_range_option(
                    params,
                    params.max_speakers,
                    "pyannote_max_speakers",
                ),
                min_speakers: Self::pyannote_speaker_range_option(
                    params,
                    params.min_speakers,
                    "pyannote_min_speakers",
                ),
                num_speakers: params.num_speakers,
                turn_level_confidence: None,
            })
            .send()
            .await?;

        let response = ensure_success(response).await?;
        response.json().await.map_err(Error::from)
    }

    async fn wait_for_job(
        client: &ClientWithMiddleware,
        base_url: &url::Url,
        api_key: &str,
        job_id: &str,
    ) -> Result<BatchResponse, Error> {
        let mut url = base_url.clone();
        append_path_if_missing(&mut url, &format!("jobs/{job_id}"));

        let config = PollingConfig::default()
            .with_interval(Duration::from_secs(3))
            .with_timeout_error("pyannote job timed out".to_string());

        poll_until(
            || async {
                let response = client
                    .get(url.to_string())
                    .bearer_auth(api_key)
                    .send()
                    .await?;
                let status = response.status();
                let response = match ensure_success(response).await {
                    Ok(response) => response,
                    Err(Error::UnexpectedStatus { body, .. }) => {
                        return Ok(PollingResult::Failed {
                            message: format!("pyannote job polling failed: {body}"),
                            retryable: is_retryable_status(status),
                        });
                    }
                    Err(err) => return Err(err),
                };

                let job: JobResponse = response.json().await?;
                match job.status.as_str() {
                    "succeeded" => Ok(PollingResult::Complete(Self::convert_to_batch_response(
                        job,
                    ))),
                    "failed" | "canceled" => Ok(PollingResult::Failed {
                        message: Self::job_error_message(&job),
                        retryable: false,
                    }),
                    _ => Ok(PollingResult::Continue),
                }
            },
            config,
        )
        .await
    }

    fn convert_to_batch_response(job: JobResponse) -> BatchResponse {
        let output = job.output.unwrap_or(DiarizationJobOutput {
            confidence: None,
            error: None,
            warning: None,
            diarization: Vec::new(),
            exclusive_diarization: Vec::new(),
            turn_level_transcription: Vec::new(),
            word_level_transcription: Vec::new(),
        });

        let transcript = if !output.turn_level_transcription.is_empty() {
            output
                .turn_level_transcription
                .iter()
                .map(|segment| segment.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            output
                .word_level_transcription
                .iter()
                .map(|segment| segment.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        };

        let words = output
            .word_level_transcription
            .iter()
            .map(|segment| BatchWord {
                word: segment.text.clone(),
                start: segment.start,
                end: segment.end,
                confidence: 1.0,
                channel: MIXED_CAPTURE_CHANNEL,
                speaker: parse_speaker_id(&segment.speaker),
                punctuated_word: Some(segment.text.clone()),
            })
            .collect();

        BatchResponse {
            metadata: serde_json::json!({
                "job_id": job.job_id,
                "status": job.status,
                "warning": output.warning,
                "confidence": output.confidence,
                "diarization": output.diarization,
                "exclusive_diarization": output.exclusive_diarization,
            }),
            results: BatchResults {
                channels: vec![BatchChannel {
                    alternatives: vec![BatchAlternatives {
                        transcript,
                        confidence: 1.0,
                        words,
                    }],
                }],
            },
        }
    }

    fn job_error_message(job: &JobResponse) -> String {
        job.output
            .as_ref()
            .and_then(|output| output.error.clone())
            .unwrap_or_else(|| format!("pyannote job {}", job.status))
    }

    fn pyannote_speaker_range_option(
        params: &ListenParams,
        value: Option<u32>,
        legacy_key: &str,
    ) -> Option<u32> {
        value.or_else(|| {
            params
                .custom_query
                .as_ref()
                .and_then(|query| query.get(legacy_key))
                .and_then(|value| value.parse::<u32>().ok())
        })
    }
}

fn is_retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    ) || status.is_server_error()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_speaker_label_accepts_prefixed_ids() {
        assert_eq!(parse_speaker_id("SPEAKER_00"), Some(0));
        assert_eq!(parse_speaker_id("speaker_12"), Some(12));
        assert_eq!(parse_speaker_id("alice"), None);
    }

    #[test]
    fn pyannote_prefers_listen_params_speaker_range_fields() {
        let params = ListenParams {
            min_speakers: Some(2),
            max_speakers: Some(4),
            ..Default::default()
        };

        assert_eq!(
            PyannoteAdapter::pyannote_speaker_range_option(
                &params,
                params.min_speakers,
                "pyannote_min_speakers"
            ),
            Some(2)
        );
        assert_eq!(
            PyannoteAdapter::pyannote_speaker_range_option(
                &params,
                params.max_speakers,
                "pyannote_max_speakers"
            ),
            Some(4)
        );
    }

    #[test]
    fn pyannote_falls_back_to_legacy_custom_query_speaker_range_keys() {
        let params = ListenParams {
            custom_query: Some(std::collections::HashMap::from([
                ("pyannote_min_speakers".to_string(), "2".to_string()),
                ("pyannote_max_speakers".to_string(), "4".to_string()),
            ])),
            ..Default::default()
        };

        assert_eq!(
            PyannoteAdapter::pyannote_speaker_range_option(
                &params,
                params.min_speakers,
                "pyannote_min_speakers"
            ),
            Some(2)
        );
        assert_eq!(
            PyannoteAdapter::pyannote_speaker_range_option(
                &params,
                params.max_speakers,
                "pyannote_max_speakers"
            ),
            Some(4)
        );
    }

    #[test]
    fn convert_to_batch_response_uses_word_level_transcription() {
        let response = PyannoteAdapter::convert_to_batch_response(JobResponse {
            job_id: "job-123".to_string(),
            status: "succeeded".to_string(),
            output: Some(DiarizationJobOutput {
                confidence: Some(serde_json::json!({
                    "score": [95, 90],
                    "resolution": 0.02,
                })),
                error: None,
                warning: Some("warning".to_string()),
                diarization: vec![DiarizationSegment {
                    speaker: "SPEAKER_00".to_string(),
                    start: 0.0,
                    end: 1.0,
                    confidence: Some(serde_json::json!({"SPEAKER_00": 93})),
                }],
                exclusive_diarization: vec![DiarizationSegment {
                    speaker: "SPEAKER_00".to_string(),
                    start: 0.0,
                    end: 1.0,
                    confidence: None,
                }],
                turn_level_transcription: vec![TranscriptionSegment {
                    speaker: "SPEAKER_00".to_string(),
                    start: 0.0,
                    end: 1.0,
                    text: "Hello world".to_string(),
                }],
                word_level_transcription: vec![
                    TranscriptionSegment {
                        speaker: "SPEAKER_00".to_string(),
                        start: 0.0,
                        end: 0.4,
                        text: "Hello".to_string(),
                    },
                    TranscriptionSegment {
                        speaker: "SPEAKER_01".to_string(),
                        start: 0.5,
                        end: 1.0,
                        text: "world".to_string(),
                    },
                ],
            }),
        });

        let alternative = &response.results.channels[0].alternatives[0];
        assert_eq!(alternative.transcript, "Hello world");
        assert_eq!(alternative.words.len(), 2);
        assert_eq!(alternative.words[0].channel, MIXED_CAPTURE_CHANNEL);
        assert_eq!(alternative.words[0].speaker, Some(0));
        assert_eq!(alternative.words[1].speaker, Some(1));
        assert_eq!(response.metadata["job_id"], "job-123");
        assert_eq!(response.metadata["confidence"]["resolution"], 0.02);
        assert_eq!(
            response.metadata["exclusive_diarization"][0]["speaker"],
            "SPEAKER_00"
        );
    }

    #[test]
    fn diarize_request_serializes_speaker_range_options() {
        let value = serde_json::to_value(DiarizeRequest {
            url: "media://audio".to_string(),
            model: PyannoteDiarizationModel::Precision2,
            transcription: true,
            transcription_config: TranscriptionConfig {
                model: PyannoteTranscriptionModel::ParakeetTdt06bV3,
            },
            max_speakers: Some(4),
            min_speakers: Some(2),
            num_speakers: Some(2),
            turn_level_confidence: None,
        })
        .unwrap();

        assert_eq!(value["maxSpeakers"], 4);
        assert_eq!(value["minSpeakers"], 2);
        assert_eq!(value["numSpeakers"], 2);
    }

    #[test]
    fn diarize_request_omits_optional_speaker_controls_when_absent() {
        let value = serde_json::to_value(DiarizeRequest {
            url: "media://audio".to_string(),
            model: PyannoteDiarizationModel::Precision2,
            transcription: true,
            transcription_config: TranscriptionConfig {
                model: PyannoteTranscriptionModel::ParakeetTdt06bV3,
            },
            max_speakers: None,
            min_speakers: None,
            num_speakers: None,
            turn_level_confidence: None,
        })
        .unwrap();

        assert!(value.get("maxSpeakers").is_none());
        assert!(value.get("minSpeakers").is_none());
        assert!(value.get("numSpeakers").is_none());
        assert!(value.get("turnLevelConfidence").is_none());
    }
}
