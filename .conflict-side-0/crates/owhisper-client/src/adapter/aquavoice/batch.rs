use std::path::{Path, PathBuf};

use owhisper_interface::ListenParams;
use owhisper_interface::batch::{Alternatives, Channel, Response as BatchResponse, Results};
use reqwest::multipart::{Form, Part};

use crate::adapter::http::mime_type_from_extension;
use crate::adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware, append_path_if_missing};
use crate::error::Error;

use super::AquaVoiceAdapter;

const DEFAULT_API_BASE: &str = "https://api.aquavoice.com/api/v1";
const DEFAULT_MODEL: &str = "avalon-v1-en";

impl BatchSttAdapter for AquaVoiceAdapter {
    fn provider_name(&self) -> &'static str {
        "aquavoice"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        AquaVoiceAdapter::is_supported_languages_batch(languages)
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
) -> Result<BatchResponse, Error> {
    let file_part = build_file_part(&file_path).await?;
    let model = params.model.as_deref().unwrap_or(DEFAULT_MODEL);
    let form = Form::new()
        .part("file", file_part)
        .text("model", model.to_string());

    let url = transcription_url(api_base)?;

    let response = client
        .post(url.to_string())
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await?;

    let status = response.status();
    if status.is_success() {
        let body: AquaVoiceResponse = response.json().await?;
        Ok(build_batch_response(body.text))
    } else {
        Err(Error::UnexpectedStatus {
            status,
            body: response.text().await.unwrap_or_default(),
        })
    }
}

#[derive(serde::Deserialize)]
struct AquaVoiceResponse {
    text: String,
}

async fn build_file_part(file_path: &Path) -> Result<Part, Error> {
    let fallback_name = match file_path.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("audio.{}", ext),
        None => "audio".to_string(),
    };

    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or(fallback_name);

    let file_bytes = tokio::fs::read(file_path)
        .await
        .map_err(|e| Error::AudioProcessing(e.to_string()))?;

    let mime_type = mime_type_from_extension(file_path);

    Part::bytes(file_bytes)
        .file_name(file_name)
        .mime_str(mime_type)
        .map_err(|e| Error::AudioProcessing(e.to_string()))
}

fn transcription_url(api_base: &str) -> Result<url::Url, Error> {
    let mut url: url::Url = if api_base.is_empty() {
        DEFAULT_API_BASE
            .parse()
            .expect("invalid_default_aquavoice_api_base")
    } else {
        api_base.parse().map_err(|e: url::ParseError| {
            Error::AudioProcessing(format!("invalid api_base: {e}"))
        })?
    };
    append_path_if_missing(&mut url, "audio/transcriptions");
    Ok(url)
}

fn build_batch_response(transcript: String) -> BatchResponse {
    let alternatives = Alternatives {
        transcript,
        confidence: 1.0,
        words: Vec::new(),
    };

    BatchResponse {
        metadata: serde_json::json!({}),
        results: Results {
            channels: vec![Channel {
                alternatives: vec![alternatives],
            }],
        },
    }
}
