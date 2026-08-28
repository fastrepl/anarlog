use anlg_ws_client::client::Message;
use base64::Engine;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};

use crate::adapter::{RealtimeSttAdapter, is_anarlog_proxy};
use crate::providers::Provider;

use super::GoogleGenerativeAiAdapter;

impl RealtimeSttAdapter for GoogleGenerativeAiAdapter {
    fn provider_name(&self) -> &'static str {
        "google_generative_ai"
    }

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
        _model: Option<&str>,
    ) -> bool {
        Self::language_support_live(languages).is_supported()
    }

    fn supports_native_multichannel(&self) -> bool {
        false
    }

    fn build_ws_url(&self, api_base: &str, _params: &ListenParams, _channels: u8) -> url::Url {
        let (mut url, existing_params) = Self::build_ws_url_from_base(api_base);
        if !existing_params.is_empty() {
            url.query_pairs_mut().extend_pairs(&existing_params);
        }
        url
    }

    fn build_ws_url_with_api_key(
        &self,
        api_base: &str,
        params: &ListenParams,
        channels: u8,
        api_key: Option<&str>,
    ) -> impl std::future::Future<Output = Option<url::Url>> + Send {
        let mut url = self.build_ws_url(api_base, params, channels);
        if !is_anarlog_proxy(api_base)
            && let Some(api_key) = api_key.filter(|key| !key.is_empty())
        {
            url.query_pairs_mut().append_pair("key", api_key);
        }
        async move { Some(url) }
    }

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)> {
        api_key.and_then(|key| Provider::GoogleGenerativeAi.build_auth_header(key))
    }

    fn keep_alive_message(&self) -> Option<Message> {
        None
    }

    fn finalize_message(&self) -> Message {
        Message::Text(r#"{"realtimeInput":{"audioStreamEnd":true}}"#.into())
    }

    fn audio_to_message(&self, audio: bytes::Bytes) -> Message {
        let payload = serde_json::json!({
            "realtimeInput": {
                "audio": {
                    "mimeType": "audio/pcm;rate=16000",
                    "data": base64::engine::general_purpose::STANDARD.encode(audio),
                }
            }
        });
        Message::Text(payload.to_string().into())
    }

    fn initial_message(
        &self,
        _api_key: Option<&str>,
        params: &ListenParams,
        _channels: u8,
    ) -> Option<Message> {
        let model = Self::model_resource(Self::resolve_live_model(params.model.as_deref()));
        let language_codes = Self::language_codes(&params.languages);
        let mut input_audio_transcription = serde_json::json!({ "mode": "smart" });
        if !language_codes.is_empty() {
            input_audio_transcription["languageCodes"] = serde_json::json!(language_codes);
        }
        if !params.keywords.is_empty() {
            input_audio_transcription["customVocabulary"] = serde_json::json!(params.keywords);
        }

        let payload = serde_json::json!({
            "setup": {
                "model": model,
                "generationConfig": {
                    "responseModalities": ["TEXT"],
                    "speechConfig": { "voiceConfig": {} }
                },
                "inputAudioTranscription": input_audio_transcription
            }
        });
        Some(Message::Text(payload.to_string().into()))
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let event: LiveEvent = match serde_json::from_str(raw) {
            Ok(event) => event,
            Err(error) => {
                tracing::warn!(
                    error = ?error,
                    anarlog.payload.size_bytes = raw.len() as u64,
                    "google_generative_ai_json_parse_failed"
                );
                return Vec::new();
            }
        };

        if let Some(error) = event.error {
            let message = error
                .message
                .filter(|message| !message.is_empty())
                .unwrap_or_else(|| "Gemini transcription error".to_string());
            return vec![StreamResponse::ErrorResponse {
                error_code: error.code,
                error_message: message,
                provider: "google_generative_ai".to_string(),
            }];
        }

        let Some(server_content) = event.server_content else {
            return Vec::new();
        };

        if let Some(transcription) = server_content.input_transcription {
            return transcript_response(transcription, true);
        }
        if let Some(transcription) = server_content.interim_input_transcription {
            return transcript_response(transcription, false);
        }

        Vec::new()
    }
}

fn transcript_response(transcription: LiveTranscription, is_final: bool) -> Vec<StreamResponse> {
    let transcript = transcription.text.trim().to_string();
    if transcript.is_empty() {
        return Vec::new();
    }

    let from_finalize = is_final && transcription.finished.unwrap_or(false);
    vec![StreamResponse::TranscriptResponse {
        start: 0.0,
        duration: 0.0,
        is_final,
        speech_final: is_final,
        from_finalize,
        channel: Channel {
            alternatives: vec![Alternatives {
                transcript,
                words: Vec::new(),
                confidence: 1.0,
                languages: Vec::new(),
            }],
        },
        metadata: Metadata::default(),
        channel_index: vec![0, 1],
    }]
}

#[derive(serde::Deserialize)]
struct LiveEvent {
    #[serde(default, rename = "serverContent")]
    server_content: Option<ServerContent>,
    #[serde(default)]
    error: Option<LiveError>,
}

#[derive(serde::Deserialize)]
struct ServerContent {
    #[serde(default, rename = "inputTranscription")]
    input_transcription: Option<LiveTranscription>,
    #[serde(default, rename = "interimInputTranscription")]
    interim_input_transcription: Option<LiveTranscription>,
}

#[derive(serde::Deserialize)]
struct LiveTranscription {
    #[serde(default)]
    text: String,
    #[serde(default)]
    finished: Option<bool>,
}

#[derive(serde::Deserialize)]
struct LiveError {
    #[serde(default)]
    code: Option<i32>,
    #[serde(default)]
    message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> ListenParams {
        ListenParams {
            sample_rate: 16_000,
            languages: vec!["en-US".parse().unwrap()],
            keywords: vec!["Anarlog".to_string()],
            model: Some("gemini-3.5-transcribe-live".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn builds_direct_and_proxy_urls() {
        let adapter = GoogleGenerativeAiAdapter;
        let direct = adapter.build_ws_url(
            "https://generativelanguage.googleapis.com/v1beta",
            &params(),
            1,
        );
        let proxy = adapter.build_ws_url(
            "https://api.anarlog.so/stt?provider=google_generative_ai",
            &params(),
            1,
        );

        assert_eq!(direct.scheme(), "wss");
        assert_eq!(direct.host_str(), Some("generativelanguage.googleapis.com"));
        assert_eq!(
            direct.path(),
            "/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent"
        );
        assert_eq!(
            proxy.as_str().split('?').next().unwrap(),
            "wss://api.anarlog.so/stt/listen"
        );
        assert!(
            proxy
                .query()
                .unwrap()
                .contains("provider=google_generative_ai")
        );
    }

    #[tokio::test]
    async fn appends_api_key_only_for_direct_urls() {
        let adapter = GoogleGenerativeAiAdapter;
        let direct = adapter
            .build_ws_url_with_api_key(
                "https://generativelanguage.googleapis.com/v1beta",
                &params(),
                1,
                Some("secret-key"),
            )
            .await
            .unwrap();
        let proxy = adapter
            .build_ws_url_with_api_key(
                "https://api.anarlog.so/stt?provider=google_generative_ai",
                &params(),
                1,
                Some("secret-key"),
            )
            .await
            .unwrap();

        assert!(direct.query().unwrap().contains("key=secret-key"));
        assert!(!proxy.query().unwrap().contains("key=secret-key"));
    }

    #[test]
    fn setup_message_includes_smart_mode_and_hints() {
        let message = GoogleGenerativeAiAdapter
            .initial_message(None, &params(), 1)
            .expect("setup message");
        let Message::Text(text) = message else {
            panic!("expected text setup");
        };
        let payload: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            payload["setup"]["model"],
            "models/gemini-3.5-transcribe-live"
        );
        assert_eq!(payload["setup"]["inputAudioTranscription"]["mode"], "smart");
        assert_eq!(
            payload["setup"]["inputAudioTranscription"]["languageCodes"][0],
            "en-US"
        );
        assert_eq!(
            payload["setup"]["inputAudioTranscription"]["customVocabulary"][0],
            "Anarlog"
        );
    }

    #[test]
    fn parses_interim_and_final_transcripts() {
        let adapter = GoogleGenerativeAiAdapter;
        let interim = adapter
            .parse_response(r#"{"serverContent":{"interimInputTranscription":{"text":"hel"}}}"#);
        let committed =
            adapter.parse_response(r#"{"serverContent":{"inputTranscription":{"text":"hello"}}}"#);
        let finalize_flush = adapter.parse_response(
            r#"{"serverContent":{"inputTranscription":{"text":"hello world","finished":true}}}"#,
        );

        let StreamResponse::TranscriptResponse {
            is_final,
            speech_final,
            from_finalize,
            channel,
            ..
        } = &interim[0]
        else {
            panic!("expected interim transcript");
        };
        assert!(!*is_final);
        assert!(!*speech_final);
        assert!(!*from_finalize);
        assert_eq!(channel.alternatives[0].transcript, "hel");

        let StreamResponse::TranscriptResponse {
            is_final,
            speech_final,
            from_finalize,
            channel,
            ..
        } = &committed[0]
        else {
            panic!("expected committed transcript");
        };
        assert!(*is_final);
        assert!(*speech_final);
        assert!(!*from_finalize);
        assert_eq!(channel.alternatives[0].transcript, "hello");

        let StreamResponse::TranscriptResponse {
            is_final,
            speech_final,
            from_finalize,
            channel,
            ..
        } = &finalize_flush[0]
        else {
            panic!("expected finalize flush");
        };
        assert!(*is_final);
        assert!(*speech_final);
        assert!(*from_finalize);
        assert_eq!(channel.alternatives[0].transcript, "hello world");
    }

    #[test]
    fn reports_upstream_errors() {
        let responses = GoogleGenerativeAiAdapter
            .parse_response(r#"{"error":{"code":400,"message":"invalid api key"}}"#);
        let StreamResponse::ErrorResponse {
            error_code,
            error_message,
            provider,
        } = &responses[0]
        else {
            panic!("expected error response");
        };
        assert_eq!(*error_code, Some(400));
        assert_eq!(error_message, "invalid api key");
        assert_eq!(provider, "google_generative_ai");
    }
}
