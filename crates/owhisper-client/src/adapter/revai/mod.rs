use std::path::{Path, PathBuf};
use std::time::Duration;

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
pub struct RevAiAdapter;

impl RevAiAdapter {
    pub fn language_support_batch(_languages: &[anlg_language::Language]) -> LanguageSupport {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    }
}

impl BatchSttAdapter for RevAiAdapter {
    fn provider_name(&self) -> &'static str {
        "revai"
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
    let mut form = Form::new().part("media", streaming_file_part(&file_path).await?);
    let mut options = serde_json::Map::new();
    if let Some(language) = params.languages.first() {
        options.insert(
            "language".to_string(),
            serde_json::Value::String(revai_language_code(language)),
        );
    }
    if !params.keywords.is_empty() {
        options.insert(
            "custom_vocabularies".to_string(),
            serde_json::json!([{
                "phrases": params.keywords,
            }]),
        );
    }
    if !options.is_empty() {
        form = form.text("options", serde_json::Value::Object(options).to_string());
    }

    let jobs_url = jobs_url(api_base)?;
    let response = client
        .post(jobs_url.to_string())
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await?;
    let job: RevJob = ensure_success(response).await?.json().await?;
    wait_for_job(client, api_key, &jobs_url, &job.id).await?;

    let mut transcript_url = jobs_url;
    append_path_if_missing(&mut transcript_url, &format!("{}/transcript", job.id));
    let response = client
        .get(transcript_url.to_string())
        .bearer_auth(api_key)
        .header("Accept", "application/vnd.rev.transcript.v1.0+json")
        .send()
        .await?;
    let transcript: RevTranscript = ensure_success(response).await?.json().await?;

    Ok(convert_response(transcript))
}

fn revai_language_code(language: &anlg_language::Language) -> String {
    language.bcp47_code().to_ascii_lowercase()
}

fn jobs_url(api_base: &str) -> Result<url::Url, Error> {
    let mut url: url::Url = if api_base.is_empty() {
        "https://api.rev.ai/speechtotext/v1"
            .parse()
            .expect("invalid_default_revai_api_base")
    } else {
        api_base.parse().map_err(|error: url::ParseError| {
            Error::AudioProcessing(format!("invalid api_base: {error}"))
        })?
    };
    append_path_if_missing(&mut url, "jobs");
    Ok(url)
}

async fn wait_for_job(
    client: &ClientWithMiddleware,
    api_key: &str,
    jobs_url: &url::Url,
    job_id: &str,
) -> Result<(), Error> {
    let mut status_url = jobs_url.clone();
    append_path_if_missing(&mut status_url, job_id);

    for _ in 0..450 {
        let response = client
            .get(status_url.to_string())
            .bearer_auth(api_key)
            .send()
            .await?;
        let job: RevJob = ensure_success(response).await?.json().await?;
        match job.status.as_str() {
            "transcribed" => return Ok(()),
            "failed" => {
                return Err(Error::AudioProcessing(
                    job.failure
                        .unwrap_or_else(|| "Rev AI transcription failed".to_string()),
                ));
            }
            _ => tokio::time::sleep(Duration::from_secs(2)).await,
        }
    }

    Err(Error::AudioProcessing(
        "Rev AI transcription did not finish before the polling limit".to_string(),
    ))
}

#[derive(serde::Deserialize)]
struct RevJob {
    id: String,
    #[serde(default)]
    status: String,
    #[serde(default, alias = "failure_detail")]
    failure: Option<String>,
}

#[derive(serde::Deserialize)]
struct RevTranscript {
    #[serde(default)]
    monologues: Vec<RevMonologue>,
}

#[derive(serde::Deserialize)]
struct RevMonologue {
    #[serde(default)]
    speaker: Option<usize>,
    #[serde(default)]
    elements: Vec<RevElement>,
}

#[derive(serde::Deserialize)]
struct RevElement {
    #[serde(rename = "type")]
    element_type: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    ts: Option<f64>,
    #[serde(default)]
    end_ts: Option<f64>,
    #[serde(default)]
    confidence: Option<f64>,
}

fn convert_response(transcript: RevTranscript) -> Response {
    let transcript_text = transcript
        .monologues
        .iter()
        .flat_map(|monologue| monologue.elements.iter())
        .map(|element| element.value.as_str())
        .collect::<String>()
        .trim()
        .to_string();
    let words = transcript
        .monologues
        .into_iter()
        .flat_map(|monologue| {
            monologue.elements.into_iter().filter_map(move |element| {
                if element.element_type != "text" {
                    return None;
                }
                Some(Word {
                    punctuated_word: Some(element.value.clone()),
                    word: element.value,
                    start: element.ts.unwrap_or_default(),
                    end: element.end_ts.unwrap_or_default(),
                    confidence: element.confidence.unwrap_or(1.0),
                    channel: 0,
                    speaker: monologue.speaker,
                })
            })
        })
        .collect();

    Response {
        metadata: serde_json::json!({ "provider": "revai" }),
        results: Results {
            channels: vec![Channel {
                alternatives: vec![Alternatives {
                    transcript: transcript_text,
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
    fn lowercases_regional_language_codes() {
        let language: anlg_language::Language = "en-US".parse().unwrap();

        assert_eq!(revai_language_code(&language), "en-us");
    }

    #[test]
    fn converts_rev_monologues() {
        let transcript: RevTranscript = serde_json::from_value(serde_json::json!({
            "monologues": [{
                "speaker": 1,
                "elements": [
                    { "type": "text", "value": "Hello", "ts": 0.1, "end_ts": 0.5, "confidence": 0.9 },
                    { "type": "punct", "value": " " },
                    { "type": "text", "value": "world.", "ts": 0.5, "end_ts": 1.0, "confidence": 0.8 }
                ]
            }]
        }))
        .unwrap();

        let response = convert_response(transcript);
        let alternative = &response.results.channels[0].alternatives[0];

        assert_eq!(alternative.transcript, "Hello world.");
        assert_eq!(alternative.words.len(), 2);
        assert_eq!(alternative.words[0].speaker, Some(1));
    }
}
