use std::path::{Path, PathBuf};

use owhisper_interface::ListenParams;
use owhisper_interface::batch::{Alternatives, Channel, Response, Results, Word};
use reqwest::StatusCode;
use reqwest::multipart::Form;

use crate::adapter::http::{ensure_success, streaming_file_part};
use crate::adapter::{
    BatchFuture, BatchSttAdapter, ClientWithMiddleware, LanguageQuality, LanguageSupport,
    append_path_if_missing,
};
use crate::error::Error;

#[derive(Clone, Default)]
pub struct SpeechmaticsAdapter;

impl SpeechmaticsAdapter {
    pub fn language_support_batch(_languages: &[anlg_language::Language]) -> LanguageSupport {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    }
}

impl BatchSttAdapter for SpeechmaticsAdapter {
    fn provider_name(&self) -> &'static str {
        "speechmatics"
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
    let language = params
        .languages
        .first()
        .map(anlg_language::Language::iso639_code)
        .unwrap_or("en");
    let model = match params.model.as_deref() {
        Some(model) if !crate::providers::is_meta_model(model) => model,
        _ => "enhanced",
    };
    let mut transcription_config = serde_json::json!({
        "language": language,
        "model": model,
    });
    if params.num_speakers.is_some()
        || params.min_speakers.is_some()
        || params.max_speakers.is_some()
    {
        transcription_config["diarization"] = serde_json::json!("speaker");
    }
    if !params.keywords.is_empty() {
        transcription_config["additional_vocab"] = serde_json::json!(
            params
                .keywords
                .iter()
                .map(|content| serde_json::json!({ "content": content }))
                .collect::<Vec<_>>()
        );
    }
    let config = serde_json::json!({
        "type": "transcription",
        "transcription_config": transcription_config,
    });
    let form = Form::new()
        .text("config", config.to_string())
        .part("data_file", streaming_file_part(&file_path).await?);

    let mut url = jobs_url(api_base)?;
    url.query_pairs_mut()
        .append_pair("wait", "120")
        .append_pair("format", "json-v2");
    let response = client
        .post(url.to_string())
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await?;
    let job: SpeechmaticsJob = ensure_success(response).await?.json().await?;
    let terminal_error = terminal_job_error(&job);

    let transcript = match job.transcript {
        Some(transcript) => transcript,
        None if job.status == "done" => {
            fetch_transcript(client, api_base, api_key, &job.id).await?
        }
        None => match terminal_error {
            Some(error) => return Err(error),
            None => wait_for_transcript(client, api_base, api_key, &job.id).await?,
        },
    };

    Ok(convert_response(transcript))
}

fn jobs_url(api_base: &str) -> Result<url::Url, Error> {
    let mut url: url::Url = if api_base.is_empty() {
        "https://eu1.asr.api.speechmatics.com/v2"
            .parse()
            .expect("invalid_default_speechmatics_api_base")
    } else {
        api_base.parse().map_err(|error: url::ParseError| {
            Error::AudioProcessing(format!("invalid api_base: {error}"))
        })?
    };
    append_path_if_missing(&mut url, "jobs");
    Ok(url)
}

async fn wait_for_transcript(
    client: &ClientWithMiddleware,
    api_base: &str,
    api_key: &str,
    job_id: &str,
) -> Result<SpeechmaticsTranscript, Error> {
    for _ in 0..30 {
        match fetch_transcript(client, api_base, api_key, job_id).await {
            Ok(transcript) => return Ok(transcript),
            Err(Error::UnexpectedStatus { status, .. }) if status == StatusCode::NOT_FOUND => {
                let job = fetch_job(client, api_base, api_key, job_id).await?;
                if let Some(error) = terminal_job_error(&job) {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        }
    }

    Err(Error::AudioProcessing(
        "Speechmatics transcription did not finish before the polling limit".to_string(),
    ))
}

async fn fetch_job(
    client: &ClientWithMiddleware,
    api_base: &str,
    api_key: &str,
    job_id: &str,
) -> Result<SpeechmaticsJob, Error> {
    let mut url = jobs_url(api_base)?;
    append_path_if_missing(&mut url, job_id);

    let response = client
        .get(url.to_string())
        .bearer_auth(api_key)
        .send()
        .await?;
    let payload: SpeechmaticsJobResponse = ensure_success(response).await?.json().await?;
    Ok(payload.job)
}

async fn fetch_transcript(
    client: &ClientWithMiddleware,
    api_base: &str,
    api_key: &str,
    job_id: &str,
) -> Result<SpeechmaticsTranscript, Error> {
    let mut url = jobs_url(api_base)?;
    append_path_if_missing(&mut url, &format!("{job_id}/transcript"));
    url.query_pairs_mut()
        .append_pair("wait", "60")
        .append_pair("format", "json-v2");

    let response = client
        .get(url.to_string())
        .bearer_auth(api_key)
        .send()
        .await?;
    Ok(ensure_success(response).await?.json().await?)
}

#[derive(serde::Deserialize)]
struct SpeechmaticsJob {
    id: String,
    status: String,
    #[serde(default)]
    errors: Vec<SpeechmaticsJobError>,
    #[serde(default, rename = "json-v2")]
    transcript: Option<SpeechmaticsTranscript>,
}

#[derive(serde::Deserialize)]
struct SpeechmaticsJobResponse {
    job: SpeechmaticsJob,
}

#[derive(serde::Deserialize)]
struct SpeechmaticsJobError {
    message: String,
}

fn terminal_job_error(job: &SpeechmaticsJob) -> Option<Error> {
    if !matches!(job.status.as_str(), "rejected" | "deleted" | "expired") {
        return None;
    }

    let detail = job
        .errors
        .last()
        .map(|error| error.message.trim())
        .filter(|message| !message.is_empty());
    let message = match detail {
        Some(detail) => format!("Speechmatics job {}: {detail}", job.status),
        None => format!("Speechmatics job {}", job.status),
    };
    Some(Error::AudioProcessing(message))
}

#[derive(serde::Deserialize)]
struct SpeechmaticsTranscript {
    #[serde(default)]
    results: Vec<SpeechmaticsResult>,
}

#[derive(serde::Deserialize)]
struct SpeechmaticsResult {
    #[serde(default)]
    alternatives: Vec<SpeechmaticsAlternative>,
    #[serde(default)]
    attaches_to: Option<String>,
    #[serde(default)]
    start_time: f64,
    #[serde(default)]
    end_time: f64,
    #[serde(default, rename = "type")]
    result_type: String,
}

#[derive(serde::Deserialize)]
struct SpeechmaticsAlternative {
    #[serde(default)]
    content: String,
    #[serde(default = "default_confidence")]
    confidence: f64,
    #[serde(default)]
    speaker: Option<String>,
}

fn default_confidence() -> f64 {
    1.0
}

fn convert_response(transcript: SpeechmaticsTranscript) -> Response {
    let mut full_text = String::new();
    let mut attach_next = false;
    let mut next_word_prefix = String::new();
    let mut words = Vec::<Word>::new();

    for result in transcript.results {
        let Some(alternative) = result.alternatives.into_iter().next() else {
            continue;
        };
        let attach_previous = matches!(
            result.attaches_to.as_deref(),
            Some("previous") | Some("both")
        ) || (result.result_type == "punctuation"
            && result.attaches_to.is_none());

        if !full_text.is_empty() && !attach_previous && !attach_next {
            full_text.push(' ');
        }
        full_text.push_str(&alternative.content);
        attach_next = matches!(result.attaches_to.as_deref(), Some("next") | Some("both"));

        if result.result_type == "punctuation" {
            if attach_previous {
                if let Some(word) = words.last_mut() {
                    word.punctuated_word
                        .get_or_insert_with(|| word.word.clone())
                        .push_str(&alternative.content);
                }
            } else if attach_next {
                next_word_prefix.push_str(&alternative.content);
            }
        } else if result.result_type == "word" {
            let punctuated_word = format!("{next_word_prefix}{}", alternative.content);
            next_word_prefix.clear();
            words.push(Word {
                punctuated_word: Some(punctuated_word),
                word: alternative.content,
                start: result.start_time,
                end: result.end_time,
                confidence: alternative.confidence,
                channel: 0,
                speaker: alternative.speaker.as_deref().and_then(parse_speaker_label),
            });
        }
    }

    Response {
        metadata: serde_json::json!({ "provider": "speechmatics" }),
        results: Results {
            channels: vec![Channel {
                alternatives: vec![Alternatives {
                    transcript: full_text,
                    confidence: 1.0,
                    words,
                }],
            }],
        },
    }
}

fn parse_speaker_label(label: &str) -> Option<usize> {
    if label == "UU" {
        return None;
    }
    label
        .trim_start_matches(|character: char| !character.is_ascii_digit())
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn converts_json_v2_words_and_speakers() {
        let transcript: SpeechmaticsTranscript = serde_json::from_value(serde_json::json!({
            "results": [
                {
                    "type": "word",
                    "start_time": 0.1,
                    "end_time": 0.4,
                    "alternatives": [{
                        "content": "Hello",
                        "confidence": 0.9,
                        "speaker": "S2"
                    }]
                },
                {
                    "type": "punctuation",
                    "attaches_to": "previous",
                    "alternatives": [{ "content": "," }]
                },
                {
                    "type": "word",
                    "start_time": 0.5,
                    "end_time": 0.8,
                    "alternatives": [{
                        "content": "world",
                        "confidence": 0.8,
                        "speaker": "S2"
                    }]
                },
                {
                    "type": "punctuation",
                    "attaches_to": "previous",
                    "alternatives": [{ "content": "." }]
                }
            ]
        }))
        .unwrap();

        let response = convert_response(transcript);
        let alternative = &response.results.channels[0].alternatives[0];
        let word = &alternative.words[0];

        assert_eq!(alternative.transcript, "Hello, world.");
        assert_eq!(alternative.words.len(), 2);
        assert_eq!(word.word, "Hello");
        assert_eq!(word.punctuated_word.as_deref(), Some("Hello,"));
        assert_eq!(
            alternative.words[1].punctuated_word.as_deref(),
            Some("world.")
        );
        assert_eq!(word.speaker, Some(2));
    }

    #[tokio::test]
    async fn returns_rejected_job_details_while_polling() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/jobs/job-1/transcript"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/jobs/job-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "job": {
                    "id": "job-1",
                    "status": "rejected",
                    "errors": [{ "message": "unsupported audio" }]
                }
            })))
            .mount(&server)
            .await;

        let client = crate::http_client::create_client();
        let Err(error) = wait_for_transcript(&client, &server.uri(), "test-key", "job-1").await
        else {
            panic!("expected rejected job error");
        };

        assert!(matches!(
            error,
            Error::AudioProcessing(message)
                if message == "Speechmatics job rejected: unsupported audio"
        ));
    }
}
