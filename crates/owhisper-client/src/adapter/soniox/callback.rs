use super::{SonioxAdapter, SonioxModel};
use crate::Error;
use crate::adapter::{CallbackProcessFuture, CallbackResult, CallbackSubmitFuture};

async fn submit(
    client: &reqwest::Client,
    api_key: &str,
    audio_url: &str,
    callback_url: &str,
) -> Result<String, Error> {
    let body = serde_json::json!({
        "model": SonioxModel::default().batch_model(),
        "audio_url": audio_url,
        "webhook_url": callback_url,
        "enable_speaker_diarization": true,
        "enable_language_identification": true,
    });

    soniox::create_transcription(client, &body, api_key)
        .await
        .map_err(|e| Error::provider_failure(e.message, e.is_retryable))
}

async fn process(
    client: &reqwest::Client,
    api_key: &str,
    payload: serde_json::Value,
) -> Result<CallbackResult, Error> {
    let callback: soniox::CallbackPayload =
        serde_json::from_value(payload).map_err(|e| Error::AudioProcessing(e.to_string()))?;

    if callback.status == "error" {
        return Ok(CallbackResult::ProviderError(
            "provider reported transcription error".into(),
        ));
    }

    let transcript = soniox::fetch_transcript_raw(client, &callback.id, api_key)
        .await
        .map_err(|e| Error::provider_failure(e.message, e.is_retryable))?;

    if let Err(e) = soniox::delete_transcription(client, &callback.id, api_key).await {
        tracing::warn!(
            hyprnote.stt.job.id = %callback.id,
            error.message = %e,
            "failed_to_delete_soniox_transcription"
        );
    }

    Ok(CallbackResult::Done(transcript))
}

impl crate::adapter::CallbackSttAdapter for SonioxAdapter {
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
        client: &'a reqwest::Client,
        api_key: &'a str,
        payload: serde_json::Value,
    ) -> CallbackProcessFuture<'a> {
        Box::pin(process(client, api_key, payload))
    }
}
