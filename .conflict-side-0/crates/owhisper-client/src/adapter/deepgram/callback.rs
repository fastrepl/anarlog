use super::DeepgramAdapter;
use crate::Error;
use crate::adapter::{CallbackProcessFuture, CallbackResult, CallbackSubmitFuture};

const API_HOST: &str = "https://api.deepgram.com";

async fn submit(
    client: &reqwest::Client,
    api_key: &str,
    audio_url: &str,
    callback_url: &str,
) -> Result<String, Error> {
    let mut url = url::Url::parse(&format!("{API_HOST}/v1/listen")).expect("valid base URL");

    url.query_pairs_mut()
        .append_pair("callback", callback_url)
        .append_pair("model", super::DeepgramModel::default().as_ref())
        .append_pair("diarize", "true")
        .append_pair("detect_language", "true")
        .append_pair("punctuate", "true")
        .append_pair("smart_format", "true")
        .append_pair("utterances", "true");

    let response = hypr_observability::with_current_trace_context(
        client
            .post(url)
            .header("Authorization", format!("Token {api_key}"))
            .json(&serde_json::json!({ "url": audio_url })),
    )
    .send()
    .await?;

    if !response.status().is_success() {
        return Err(Error::UnexpectedStatus {
            status: response.status(),
            body: response.text().await.unwrap_or_default(),
        });
    }

    let body: serde_json::Value = response.json().await?;

    body.get("request_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| Error::AudioProcessing("missing request_id in response".into()))
}

impl crate::adapter::CallbackSttAdapter for DeepgramAdapter {
    fn submit_callback<'a>(
        &'a self,
        client: &'a reqwest::Client,
        api_key: &'a str,
        audio_url: &'a str,
        callback_url: &'a str,
    ) -> CallbackSubmitFuture<'a> {
        Box::pin(submit(client, api_key, audio_url, callback_url))
    }

    fn process_callback<'a>(
        &'a self,
        _client: &'a reqwest::Client,
        _api_key: &'a str,
        payload: serde_json::Value,
    ) -> CallbackProcessFuture<'a> {
        Box::pin(async move { Ok(CallbackResult::Done(payload)) })
    }
}
