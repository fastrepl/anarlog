use std::{io::Write, path::Path, time::Duration};

use owhisper_client::{
    AdapterKind, AquaVoiceAdapter, AssemblyAIAdapter, AwsTranscribeAdapter, AzureSpeechAdapter,
    BatchSttAdapter, CartesiaAdapter, CohereAdapter, DeepgramAdapter, ElevenLabsAdapter,
    GladiaAdapter, GoogleCloudAdapter, GoogleGenerativeAiAdapter, GroqAdapter, MistralAdapter,
    OpenAIAdapter, OpenRouterAdapter, PyannoteAdapter, RevAiAdapter, SiliconFlowAdapter,
    SmallestAIAdapter, SonioxAdapter, SpeechmaticsAdapter, TogetherAdapter, XaiAdapter, ZaiAdapter,
};
use owhisper_interface::{ListenParams, batch::Response};
use reqwest_middleware::ClientWithMiddleware;
use serde::{Deserialize, Serialize};

const MAX_AUDIO_BYTES: u64 = 512 * 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ProviderTranscriptionError {
    #[error("Check the transcription provider settings.")]
    InvalidSettings,
    #[error("This recording could not be read.")]
    AudioMissing,
    #[error("This provider accepts recordings up to {max_megabytes} MB.")]
    AudioTooLarge { max_megabytes: u64 },
    #[error("The transcription provider took too long. Please try again.")]
    TimedOut,
    #[error("The transcription response is too large.")]
    ResponseTooLarge,
    #[error("The transcription request could not be completed.")]
    RequestFailed,
}

#[derive(Deserialize)]
struct Request {
    provider: String,
    base_url: String,
    api_key: String,
    file_uri: String,
    params: ListenParams,
}

#[derive(Serialize)]
struct Reply {
    status: u16,
    body: String,
}

// UniFFI owns this future, so an AbortSignal from JS drops uploads and polling
// immediately. It does not use the database runtime or outlive a DB close.
#[uniffi::export(async_runtime = "tokio")]
pub async fn transcribe_provider_audio(
    request_json: String,
) -> Result<String, ProviderTranscriptionError> {
    let request: Request = serde_json::from_str(&request_json)
        .map_err(|_| ProviderTranscriptionError::InvalidSettings)?;
    let adapter = validate_request(&request)?;
    let path = url::Url::parse(&request.file_uri)
        .ok()
        .and_then(|url| url.to_file_path().ok())
        .ok_or(ProviderTranscriptionError::AudioMissing)?;
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|_| ProviderTranscriptionError::AudioMissing)?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(ProviderTranscriptionError::AudioMissing);
    }
    let max_bytes = adapter
        .batch_upload_limit(request.params.model.as_deref())
        .map_or(MAX_AUDIO_BYTES, |limit| {
            limit.max_bytes.min(MAX_AUDIO_BYTES)
        });
    if metadata.len() > max_bytes {
        return Err(ProviderTranscriptionError::AudioTooLarge {
            max_megabytes: max_bytes / (1024 * 1024),
        });
    }
    let client = reqwest_middleware::ClientBuilder::new(
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| ProviderTranscriptionError::RequestFailed)?,
    )
    .build();
    let result = tokio::time::timeout(
        Duration::from_secs(15 * 60),
        dispatch(adapter, &client, &request, &path),
    )
    .await
    .map_err(|_| ProviderTranscriptionError::TimedOut)?;
    let reply = match result {
        Ok(response) => Reply {
            status: 200,
            body: serialize_response(&response)?,
        },
        Err(error) => Reply {
            status: error_status(&error),
            // Provider errors can contain credentials, filenames, or audio
            // content. Only the status crosses into JS and error reporting.
            body: String::new(),
        },
    };
    serde_json::to_string(&reply).map_err(|_| ProviderTranscriptionError::ResponseTooLarge)
}

fn validate_request(request: &Request) -> Result<AdapterKind, ProviderTranscriptionError> {
    let adapter = validate_provider_settings(
        &request.provider,
        &request.base_url,
        &request.api_key,
        &request.params,
    )?;
    if matches!(
        adapter,
        AdapterKind::Anarlog
            | AdapterKind::Argmax
            | AdapterKind::DashScope
            | AdapterKind::Fireworks
    ) {
        return Err(ProviderTranscriptionError::InvalidSettings);
    }
    Ok(adapter)
}

pub(crate) fn validate_provider_settings(
    provider: &str,
    base_url: &str,
    api_key: &str,
    params: &ListenParams,
) -> Result<AdapterKind, ProviderTranscriptionError> {
    let invalid = || ProviderTranscriptionError::InvalidSettings;
    let url = url::Url::parse(base_url).map_err(|_| invalid())?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || api_key.trim().is_empty()
        || api_key.len() > 8192
        || api_key.contains(['\r', '\n'])
        || params.model.as_deref().is_none_or(|model| {
            model.trim().is_empty() || model.len() > 200 || model.contains(['\r', '\n'])
        })
    {
        return Err(invalid());
    }
    let adapter = match provider {
        "cloudflare_workers_ai" => AdapterKind::Deepgram,
        provider => provider.parse().map_err(|_| invalid())?,
    };
    Ok(adapter)
}

async fn dispatch(
    adapter: AdapterKind,
    client: &ClientWithMiddleware,
    request: &Request,
    path: &Path,
) -> Result<Response, owhisper_client::Error> {
    macro_rules! adapters {
        ($($variant:ident => $adapter:ty),+ $(,)?) => {
            match adapter {
                $(AdapterKind::$variant => <$adapter>::default().transcribe_file(
                    client, &request.base_url, &request.api_key, &request.params, path,
                ).await,)+
                _ => Err(owhisper_client::Error::provider_configuration(
                    adapter.to_string(), "Batch transcription is unavailable.",
                )),
            }
        };
    }
    adapters! {
        AquaVoice => AquaVoiceAdapter,
        AssemblyAI => AssemblyAIAdapter,
        AwsTranscribe => AwsTranscribeAdapter,
        AzureSpeech => AzureSpeechAdapter,
        Cartesia => CartesiaAdapter,
        Cohere => CohereAdapter,
        Deepgram => DeepgramAdapter,
        ElevenLabs => ElevenLabsAdapter,
        Gladia => GladiaAdapter,
        GoogleCloud => GoogleCloudAdapter,
        GoogleGenerativeAi => GoogleGenerativeAiAdapter,
        Groq => GroqAdapter,
        Mistral => MistralAdapter,
        OpenAI => OpenAIAdapter,
        OpenRouter => OpenRouterAdapter,
        Pyannote => PyannoteAdapter,
        RevAi => RevAiAdapter,
        SiliconFlow => SiliconFlowAdapter,
        SmallestAI => SmallestAIAdapter,
        Soniox => SonioxAdapter,
        Speechmatics => SpeechmaticsAdapter,
        Together => TogetherAdapter,
        Xai => XaiAdapter,
        Zai => ZaiAdapter,
    }
}

fn error_status(error: &owhisper_client::Error) -> u16 {
    use owhisper_client::Error;
    match error {
        Error::UnexpectedStatus { status, .. }
        | Error::ProviderFailure {
            status: Some(status),
            ..
        } => status.as_u16(),
        Error::ProviderFailure { retryable, .. } => {
            if *retryable {
                503
            } else {
                422
            }
        }
        Error::Http(error) => error.status().map_or(503, |status| status.as_u16()),
        Error::ProviderConfiguration { .. } | Error::AudioProcessing(_) => 422,
        _ => 503,
    }
}

fn serialize_response(response: &Response) -> Result<String, ProviderTranscriptionError> {
    struct BoundedWriter(Vec<u8>);
    impl Write for BoundedWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.0.len().saturating_add(bytes.len()) > MAX_RESPONSE_BYTES {
                return Err(std::io::Error::other("response too large"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = BoundedWriter(Vec::new());
    serde_json::to_writer(&mut writer, response)
        .map_err(|_| ProviderTranscriptionError::ResponseTooLarge)?;
    String::from_utf8(writer.0).map_err(|_| ProviderTranscriptionError::ResponseTooLarge)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(provider: &str) -> Request {
        serde_json::from_value(json!({
            "provider": provider,
            "base_url": "https://provider.example/v1",
            "api_key": "synthetic-key",
            "file_uri": "file:///audio.wav",
            "params": {"model": "test-model", "languages": ["ko", "en"], "keywords": ["Anarlog"]},
        }))
        .unwrap()
    }

    #[test]
    fn validates_desktop_adapter_ids_and_rejects_non_batch_providers() {
        for provider in [
            "aquavoice",
            "assemblyai",
            "aws_transcribe",
            "azure_speech",
            "cartesia",
            "cohere",
            "deepgram",
            "elevenlabs",
            "gladia",
            "google_cloud",
            "google_generative_ai",
            "groq",
            "mistral",
            "openai",
            "openrouter",
            "pyannote",
            "revai",
            "siliconflow",
            "smallestai",
            "soniox",
            "speechmatics",
            "together",
            "xai",
            "zai",
            "cloudflare_workers_ai",
        ] {
            assert!(validate_request(&request(provider)).is_ok(), "{provider}");
        }
        for provider in [
            "anarlog",
            "argmax",
            "dashscope",
            "fireworks",
            "local_file",
            "unknown",
        ] {
            assert!(validate_request(&request(provider)).is_err(), "{provider}");
        }
        assert_eq!(
            validate_request(&request("cloudflare_workers_ai")).unwrap(),
            AdapterKind::Deepgram
        );
    }

    #[test]
    fn rejects_unsafe_urls_and_header_injection_without_echoing_credentials() {
        for url in [
            "http://provider.example",
            "https://secret@provider.example",
            "https://provider.example?key=secret",
            "https://provider.example#fragment",
        ] {
            let mut request = request("openai");
            request.base_url = url.into();
            assert!(validate_request(&request).is_err());
        }
        let mut request = request("openai");
        request.api_key = "secret\r\nx-injected: true".into();
        assert_eq!(
            validate_request(&request).unwrap_err().to_string(),
            "Check the transcription provider settings."
        );
    }

    #[tokio::test]
    async fn rejects_oversized_files_before_upload() {
        let file = tempfile::NamedTempFile::new().unwrap();
        file.as_file().set_len(26 * 1024 * 1024).unwrap();
        let request = json!({
            "provider": "openai", "base_url": "https://provider.example/v1", "api_key": "synthetic-key",
            "file_uri": url::Url::from_file_path(file.path()).unwrap().as_str(), "params": {"model": "whisper-1"},
        });
        assert!(matches!(
            transcribe_provider_audio(request.to_string()).await,
            Err(ProviderTranscriptionError::AudioTooLarge { max_megabytes: 25 })
        ));
    }

    #[test]
    fn preserves_retryable_and_auth_status_without_provider_error_text() {
        use owhisper_client::Error;
        assert_eq!(
            error_status(&Error::UnexpectedStatus {
                status: reqwest::StatusCode::UNAUTHORIZED,
                body: "secret".into()
            }),
            401
        );
        assert_eq!(
            error_status(&Error::provider_failure_with_status(
                reqwest::StatusCode::TOO_MANY_REQUESTS,
                "secret",
                true
            )),
            429
        );
        assert_eq!(error_status(&Error::provider_failure("secret", false)), 422);
        assert_eq!(error_status(&Error::provider_failure("secret", true)), 503);
    }

    #[tokio::test]
    async fn native_dispatch_uploads_to_the_desktop_adapter_and_returns_canonical_words() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut received = Vec::new();
            loop {
                let mut buffer = [0; 1024];
                let count = socket.read(&mut buffer).await.unwrap();
                assert!(count > 0);
                received.extend_from_slice(&buffer[..count]);
                if received.ends_with(b"audio") {
                    break;
                }
            }
            let payload = json!({"results": {"channels": [{"alternatives": [{"transcript": "Hello", "confidence": 1.0, "words": [{"word": "Hello", "punctuated_word": "Hello", "start": 0.0, "end": 1.0, "confidence": 1.0, "speaker": 0}]}]}]}, "metadata": {}}).to_string();
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", payload.len(), payload).as_bytes()).await.unwrap();
            String::from_utf8(received).unwrap()
        });
        let mut file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
        file.write_all(b"audio").unwrap();
        let mut request = request("deepgram");
        request.base_url = format!("http://{address}/v1");
        request.params.model = Some("nova-3".into());
        let client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build();
        let response = dispatch(AdapterKind::Deepgram, &client, &request, file.path())
            .await
            .unwrap();
        let received = server.await.unwrap().to_lowercase();
        assert!(received.starts_with("post /v1/listen?"));
        assert!(received.contains("authorization: token synthetic-key"));
        assert!(received.contains("keyterm=anarlog"));
        assert_eq!(
            response.results.channels[0].alternatives[0].words[0].word,
            "Hello"
        );
        assert!(serialize_response(&response).unwrap().contains("Hello"));
    }
}
