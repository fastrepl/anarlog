use serde::{Deserialize, Serialize};

pub const API_HOST: &str = "https://api.soniox.com";

#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub is_retryable: bool,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for Error {}

fn is_retryable_status(status: u16) -> bool {
    matches!(status, 429 | 500..)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SpeakerId {
    Num(i32),
    Str(String),
}

impl SpeakerId {
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            SpeakerId::Num(n) => Some(*n),
            SpeakerId::Str(s) => s
                .trim_start_matches(|c: char| !c.is_ascii_digit())
                .parse()
                .ok(),
        }
    }

    pub fn as_u32(&self) -> Option<u32> {
        self.as_i32().and_then(|n| u32::try_from(n).ok())
    }

    pub fn as_usize(&self) -> Option<usize> {
        self.as_i32().and_then(|n| usize::try_from(n).ok())
    }
}

#[derive(Debug, Deserialize)]
pub struct Token {
    pub text: String,
    #[serde(default)]
    pub start_ms: Option<u64>,
    #[serde(default)]
    pub end_ms: Option<u64>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub is_final: Option<bool>,
    #[serde(default)]
    pub speaker: Option<SpeakerId>,
    #[serde(default)]
    pub language: Option<String>,
}

impl Token {
    pub fn is_fin(&self) -> bool {
        self.text == "<fin>" && self.is_final == Some(true)
    }

    pub fn is_end(&self) -> bool {
        self.text == "<end>"
    }

    pub fn is_control(&self) -> bool {
        self.is_fin() || self.is_end()
    }
}

#[derive(Debug, Deserialize)]
pub struct TranscriptResponse {
    pub text: String,
    #[serde(default)]
    pub tokens: Vec<Token>,
}

#[derive(Debug, Deserialize)]
pub struct StreamMessage {
    #[serde(default)]
    pub tokens: Vec<Token>,
    #[serde(default)]
    pub finished: Option<bool>,
    #[serde(default)]
    pub error_code: Option<i32>,
    #[serde(default)]
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CallbackPayload {
    pub id: String,
    pub status: String,
}

#[derive(Deserialize)]
struct CreateTranscriptionResponse {
    id: String,
}

pub async fn create_transcription(
    client: &reqwest::Client,
    body: &impl Serialize,
    api_key: &str,
) -> Result<String, Error> {
    let response = hypr_observability::with_current_trace_context(
        client
            .post(format!("{API_HOST}/v1/transcriptions"))
            .header("Authorization", format!("Bearer {api_key}"))
            .json(body),
    )
    .send()
    .await
    .map_err(|e| Error {
        message: format!("request failed: {e}"),
        is_retryable: true,
    })?;

    let status = response.status().as_u16();
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(Error {
            message: format!("{status} - {error_text}"),
            is_retryable: is_retryable_status(status),
        });
    }

    let result: CreateTranscriptionResponse = response.json().await.map_err(|e| Error {
        message: format!("failed to parse response: {e}"),
        is_retryable: false,
    })?;

    if result.id.is_empty() {
        return Err(Error {
            message: "missing transcription id".to_string(),
            is_retryable: false,
        });
    }

    Ok(result.id)
}

pub async fn fetch_transcript(
    client: &reqwest::Client,
    transcription_id: &str,
    api_key: &str,
) -> Result<TranscriptResponse, Error> {
    let response = hypr_observability::with_current_trace_context(
        client
            .get(format!(
                "{API_HOST}/v1/transcriptions/{transcription_id}/transcript"
            ))
            .header("Authorization", format!("Bearer {api_key}")),
    )
    .send()
    .await
    .map_err(|e| Error {
        message: format!("fetch transcript failed: {e}"),
        is_retryable: true,
    })?;

    let status = response.status().as_u16();
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(Error {
            message: format!("fetch transcript: {status} - {error_text}"),
            is_retryable: is_retryable_status(status),
        });
    }

    response.json().await.map_err(|e| Error {
        message: format!("failed to parse transcript response: {e}"),
        is_retryable: false,
    })
}

pub async fn fetch_transcript_raw(
    client: &reqwest::Client,
    transcription_id: &str,
    api_key: &str,
) -> Result<serde_json::Value, Error> {
    let response = hypr_observability::with_current_trace_context(
        client
            .get(format!(
                "{API_HOST}/v1/transcriptions/{transcription_id}/transcript"
            ))
            .header("Authorization", format!("Bearer {api_key}")),
    )
    .send()
    .await
    .map_err(|e| Error {
        message: format!("fetch transcript failed: {e}"),
        is_retryable: true,
    })?;

    let status = response.status().as_u16();
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(Error {
            message: format!("fetch transcript: {status} - {error_text}"),
            is_retryable: is_retryable_status(status),
        });
    }

    response.json().await.map_err(|e| Error {
        message: format!("failed to parse transcript response: {e}"),
        is_retryable: false,
    })
}

pub async fn upload_file(
    client: &reqwest::Client,
    file_name: &str,
    file_bytes: Vec<u8>,
    api_key: &str,
) -> Result<String, Error> {
    let part = reqwest::multipart::Part::bytes(file_bytes).file_name(file_name.to_string());
    let form = reqwest::multipart::Form::new().part("file", part);

    let response = client
        .post(format!("{API_HOST}/v1/files"))
        .header("Authorization", format!("Bearer {api_key}"))
        .multipart(form)
        .send()
        .await
        .map_err(|e| Error {
            message: format!("upload file failed: {e}"),
            is_retryable: true,
        })?;

    let status = response.status().as_u16();
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(Error {
            message: format!("upload file: {status} - {error_text}"),
            is_retryable: is_retryable_status(status),
        });
    }

    #[derive(Deserialize)]
    struct FileUploadResponse {
        id: String,
    }

    let result: FileUploadResponse = response.json().await.map_err(|e| Error {
        message: format!("failed to parse upload response: {e}"),
        is_retryable: false,
    })?;

    Ok(result.id)
}

pub async fn delete_transcription(
    client: &reqwest::Client,
    transcription_id: &str,
    api_key: &str,
) -> Result<(), Error> {
    let response = client
        .delete(format!("{API_HOST}/v1/transcriptions/{transcription_id}"))
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .await
        .map_err(|e| Error {
            message: format!("delete transcription failed: {e}"),
            is_retryable: true,
        })?;

    let status = response.status().as_u16();
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(Error {
            message: format!("delete transcription: {status} - {error_text}"),
            is_retryable: is_retryable_status(status),
        });
    }

    Ok(())
}

pub async fn delete_file(
    client: &reqwest::Client,
    file_id: &str,
    api_key: &str,
) -> Result<(), Error> {
    let response = client
        .delete(format!("{API_HOST}/v1/files/{file_id}"))
        .header("Authorization", format!("Bearer {api_key}"))
        .send()
        .await
        .map_err(|e| Error {
            message: format!("delete file failed: {e}"),
            is_retryable: true,
        })?;

    let status = response.status().as_u16();
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(Error {
            message: format!("delete file: {status} - {error_text}"),
            is_retryable: is_retryable_status(status),
        });
    }

    Ok(())
}

pub async fn wait_for_completion(
    client: &reqwest::Client,
    transcription_id: &str,
    api_key: &str,
) -> Result<(), Error> {
    use std::time::Duration;

    #[derive(Deserialize)]
    struct StatusResponse {
        status: String,
        #[serde(default)]
        error_message: Option<String>,
    }

    let url = format!("{API_HOST}/v1/transcriptions/{transcription_id}");

    for _ in 0..300 {
        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .send()
            .await
            .map_err(|e| Error {
                message: format!("poll transcription failed: {e}"),
                is_retryable: true,
            })?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error {
                message: format!("poll transcription: {status} - {error_text}"),
                is_retryable: is_retryable_status(status),
            });
        }

        let result: StatusResponse = response.json().await.map_err(|e| Error {
            message: format!("failed to parse status response: {e}"),
            is_retryable: false,
        })?;

        match result.status.as_str() {
            "completed" => return Ok(()),
            "error" => {
                return Err(Error {
                    message: format!(
                        "transcription failed: {}",
                        result
                            .error_message
                            .unwrap_or_else(|| "unknown error".to_string())
                    ),
                    is_retryable: false,
                });
            }
            "queued" | "processing" => {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            unknown => {
                return Err(Error {
                    message: format!("unexpected transcription status: {unknown}"),
                    is_retryable: false,
                });
            }
        }
    }

    Err(Error {
        message: "transcription timed out".to_string(),
        is_retryable: false,
    })
}
