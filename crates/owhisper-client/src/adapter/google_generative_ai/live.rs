use anlg_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};

use super::{GoogleGenerativeAiAdapter, WS_PATH, parse_duration_secs};
use crate::adapter::parsing::{WordBuilder, parse_speaker_id};
use crate::adapter::{
    RealtimeSttAdapter, build_proxy_ws_url, build_url_with_scheme, is_anarlog_proxy,
};
use crate::providers::Provider;

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

    fn build_ws_url(&self, api_base: &str, params: &ListenParams, _channels: u8) -> url::Url {
        self.build_ws_url_inner(api_base, params, None)
    }

    fn build_ws_url_with_api_key(
        &self,
        api_base: &str,
        params: &ListenParams,
        _channels: u8,
        api_key: Option<&str>,
    ) -> impl std::future::Future<Output = Option<url::Url>> + Send {
        let url = self.build_ws_url_inner(api_base, params, api_key);
        async move { Some(url) }
    }

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)> {
        api_key.and_then(|key| Provider::GoogleGenerativeAi.build_auth_header(key))
    }

    fn keep_alive_message(&self) -> Option<Message> {
        None
    }

    fn audio_to_message(&self, audio: bytes::Bytes) -> Message {
        use base64::Engine;
        let event = serde_json::json!({
            "realtimeInput": {
                "audio": {
                    "mimeType": "audio/pcm;rate=16000",
                    "data": base64::engine::general_purpose::STANDARD.encode(audio),
                }
            }
        });
        Message::Text(event.to_string().into())
    }

    fn initial_message(
        &self,
        _api_key: Option<&str>,
        params: &ListenParams,
        _channels: u8,
    ) -> Option<Message> {
        let model = Self::setup_model_name(&Self::resolve_live_model(params.model.as_deref()));
        let language_codes = Self::language_codes(params);
        let mut input_audio_transcription = serde_json::Map::new();
        if !language_codes.is_empty() {
            input_audio_transcription.insert(
                "languageCodes".to_string(),
                serde_json::Value::Array(
                    language_codes
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }
        if !params.keywords.is_empty() {
            input_audio_transcription.insert(
                "customVocabulary".to_string(),
                serde_json::Value::Array(
                    params
                        .keywords
                        .iter()
                        .cloned()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }

        // Omitting generationConfig.responseModalities: on the current Live
        // endpoint that field suppresses final inputTranscription segments.
        let setup = serde_json::json!({
            "setup": {
                "model": model,
                "inputAudioTranscription": input_audio_transcription,
            }
        });
        Some(Message::Text(setup.to_string().into()))
    }

    fn finalize_message(&self) -> Message {
        Message::Text(r#"{"realtimeInput":{"audioStreamEnd":true}}"#.into())
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let event: serde_json::Value = match serde_json::from_str(raw) {
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

        if let Some(message) = event
            .pointer("/error/message")
            .and_then(serde_json::Value::as_str)
        {
            return vec![StreamResponse::ErrorResponse {
                error_code: event
                    .pointer("/error/code")
                    .and_then(|value| value.as_i64().and_then(|code| i32::try_from(code).ok())),
                error_message: message.to_string(),
                provider: "google_generative_ai".to_string(),
            }];
        }

        let server_content = event
            .get("serverContent")
            .or_else(|| event.get("server_content"));
        let Some(server_content) = server_content else {
            return Vec::new();
        };

        if let Some(response) = transcription_response(
            server_content
                .get("inputTranscription")
                .or_else(|| server_content.get("input_transcription")),
            true,
        ) {
            return vec![response];
        }

        if let Some(response) = transcription_response(
            server_content
                .get("interimInputTranscription")
                .or_else(|| server_content.get("interim_input_transcription")),
            false,
        ) {
            return vec![response];
        }

        Vec::new()
    }
}

impl GoogleGenerativeAiAdapter {
    fn build_ws_url_inner(
        &self,
        api_base: &str,
        _params: &ListenParams,
        api_key: Option<&str>,
    ) -> url::Url {
        let (mut url, existing_params) = if let Some(result) = build_proxy_ws_url(api_base) {
            result
        } else {
            let parsed: url::Url = if api_base.is_empty() {
                Provider::GoogleGenerativeAi
                    .default_api_base()
                    .parse()
                    .expect("invalid_default_google_generative_ai_api_base")
            } else {
                api_base.parse().unwrap_or_else(|_| {
                    Provider::GoogleGenerativeAi
                        .default_api_base()
                        .parse()
                        .expect("invalid_default_google_generative_ai_api_base")
                })
            };
            (
                build_url_with_scheme(
                    &parsed,
                    Provider::GoogleGenerativeAi.default_ws_host(),
                    WS_PATH,
                    true,
                ),
                Vec::new(),
            )
        };

        {
            let mut query = url.query_pairs_mut();
            if is_anarlog_proxy(api_base) {
                query.extend_pairs(&existing_params);
            } else if let Some(api_key) = api_key.filter(|key| !key.is_empty()) {
                query.append_pair("key", api_key);
            }
        }

        url
    }
}

fn transcription_response(
    value: Option<&serde_json::Value>,
    is_final: bool,
) -> Option<StreamResponse> {
    let value = value?;
    let text = value
        .get("text")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .trim();
    if text.is_empty() {
        return None;
    }

    let speaker = value
        .get("speakerLabel")
        .or_else(|| value.get("speaker_label"))
        .and_then(serde_json::Value::as_str)
        .and_then(parse_speaker_id)
        .and_then(|speaker| i32::try_from(speaker).ok());
    let start = value
        .get("startOffset")
        .or_else(|| value.get("start_offset"))
        .map(parse_duration_secs)
        .unwrap_or_default();
    let end = value
        .get("endOffset")
        .or_else(|| value.get("end_offset"))
        .map(parse_duration_secs)
        .unwrap_or(start);
    let words = text
        .split_whitespace()
        .map(|word| {
            WordBuilder::new(word)
                .start(start)
                .end(end)
                .speaker(speaker)
                .build()
        })
        .collect();

    let from_finalize = is_final
        && value
            .get("finished")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

    Some(StreamResponse::TranscriptResponse {
        start,
        duration: (end - start).max(0.0),
        is_final,
        speech_final: is_final,
        from_finalize,
        channel: Channel {
            alternatives: vec![Alternatives {
                transcript: text.to_string(),
                words,
                confidence: 1.0,
                languages: Vec::new(),
            }],
        },
        metadata: Metadata::default(),
        channel_index: vec![0, 1],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_direct_and_proxy_urls() {
        let params = ListenParams {
            sample_rate: 16_000,
            languages: vec![anlg_language::ISO639::En.into()],
            ..Default::default()
        };
        let adapter = GoogleGenerativeAiAdapter;

        let direct = adapter.build_ws_url(
            "https://generativelanguage.googleapis.com/v1beta",
            &params,
            1,
        );
        let keyed = adapter.build_ws_url_inner(
            "https://generativelanguage.googleapis.com/v1beta",
            &params,
            Some("test-key"),
        );
        let proxy = adapter.build_ws_url(
            "https://api.anarlog.so/stt?provider=google_generative_ai",
            &params,
            1,
        );

        assert_eq!(direct.scheme(), "wss");
        assert_eq!(direct.host_str(), Some("generativelanguage.googleapis.com"));
        assert_eq!(direct.path(), WS_PATH);
        assert!(keyed.query().unwrap().contains("key=test-key"));
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

    #[test]
    fn initial_message_includes_languages_and_keywords() {
        let params = ListenParams {
            languages: vec!["en-US".parse().unwrap()],
            keywords: vec!["Anarlog".to_string()],
            model: Some("gemini-3.5-transcribe-live".to_string()),
            ..Default::default()
        };
        let Message::Text(payload) = GoogleGenerativeAiAdapter
            .initial_message(None, &params, 1)
            .expect("setup message")
        else {
            panic!("expected text setup message");
        };
        let json: serde_json::Value = serde_json::from_str(&payload).unwrap();

        assert_eq!(json["setup"]["model"], "models/gemini-3.5-transcribe-live");
        assert!(json["setup"].get("generationConfig").is_none());
        assert_eq!(
            json["setup"]["inputAudioTranscription"]["languageCodes"][0],
            "en-US"
        );
        assert_eq!(
            json["setup"]["inputAudioTranscription"]["customVocabulary"][0],
            "Anarlog"
        );
    }

    #[test]
    fn initial_message_omits_languages_when_unset() {
        let Message::Text(payload) = GoogleGenerativeAiAdapter
            .initial_message(None, &ListenParams::default(), 1)
            .expect("setup message")
        else {
            panic!("expected text setup message");
        };
        let json: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert!(
            json["setup"]["inputAudioTranscription"]
                .get("languageCodes")
                .is_none()
        );
    }

    #[test]
    fn parses_interim_and_final_transcriptions() {
        let adapter = GoogleGenerativeAiAdapter;
        let interim = adapter
            .parse_response(r#"{"serverContent":{"interimInputTranscription":{"text":"hel"}}}"#);
        let final_event = adapter.parse_response(
            r#"{"serverContent":{"inputTranscription":{"text":"hello there","speakerLabel":"Speaker 2","startOffset":"0.10s","endOffset":"0.80s"}}}"#,
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
            start,
            duration,
            ..
        } = &final_event[0]
        else {
            panic!("expected final transcript");
        };
        assert!(*is_final);
        assert!(*speech_final);
        assert!(!*from_finalize);
        assert_eq!(channel.alternatives[0].transcript, "hello there");
        assert_eq!(channel.alternatives[0].words[0].speaker, Some(2));
        assert_eq!(*start, 0.1);
        assert!((*duration - 0.7).abs() < f64::EPSILON);

        let finalize_flush = adapter.parse_response(
            r#"{"serverContent":{"inputTranscription":{"text":"hello there","finished":true}}}"#,
        );
        let StreamResponse::TranscriptResponse { from_finalize, .. } = &finalize_flush[0] else {
            panic!("expected finalize flush");
        };
        assert!(*from_finalize);
    }

    #[test]
    fn reports_provider_errors() {
        let responses = GoogleGenerativeAiAdapter
            .parse_response(r#"{"error":{"code":13,"message":"quota exceeded"}}"#);
        let StreamResponse::ErrorResponse {
            error_message,
            provider,
            ..
        } = &responses[0]
        else {
            panic!("expected error response");
        };
        assert_eq!(error_message, "quota exceeded");
        assert_eq!(provider, "google_generative_ai");
    }
}
