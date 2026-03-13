use hypr_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};
use serde::{Deserialize, Serialize};

use super::{ElevenLabsAdapter, ElevenLabsWord};
use crate::adapter::RealtimeSttAdapter;
use crate::adapter::parsing::{WordBuilder, calculate_time_span};

impl RealtimeSttAdapter for ElevenLabsAdapter {
    fn provider_name(&self) -> &'static str {
        "elevenlabs"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        ElevenLabsAdapter::is_supported_languages_live(languages)
    }

    fn supports_native_multichannel(&self) -> bool {
        false
    }

    fn build_ws_url(&self, api_base: &str, params: &ListenParams, _channels: u8) -> url::Url {
        let (mut url, existing_params) = Self::build_ws_url_from_base(api_base);

        {
            let mut query_pairs = url.query_pairs_mut();

            for (key, value) in &existing_params {
                query_pairs.append_pair(key, value);
            }

            let default = crate::providers::Provider::ElevenLabs.default_live_model();
            let model = match params.model.as_deref() {
                Some(m) if crate::providers::is_meta_model(m) => default,
                Some("scribe_v2") => default,
                Some(m) => m,
                None => default,
            };
            query_pairs.append_pair("model_id", model);

            let audio_format = format!("pcm_{}", params.sample_rate);
            query_pairs.append_pair("audio_format", &audio_format);

            query_pairs.append_pair("include_timestamps", "true");

            query_pairs.append_pair("commit_strategy", "vad");

            if let Some(lang) = params.languages.first() {
                query_pairs.append_pair("language_code", lang.iso639().code());
            }
        }

        url
    }

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)> {
        api_key.map(|key| ("xi-api-key", key.to_string()))
    }

    fn keep_alive_message(&self) -> Option<Message> {
        None
    }

    fn initial_message(
        &self,
        _api_key: Option<&str>,
        _params: &ListenParams,
        _channels: u8,
    ) -> Option<Message> {
        None
    }

    fn audio_to_message(&self, audio: bytes::Bytes) -> Message {
        let chunk = AudioChunk {
            message_type: "input_audio_chunk",
            audio_base_64: base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &audio,
            ),
        };
        let json = serde_json::to_string(&chunk).unwrap();
        Message::Text(json.into())
    }

    fn finalize_message(&self) -> Message {
        Message::Text(r#"{"message_type":"commit"}"#.into())
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let msg: ElevenLabsMessage = match serde_json::from_str(raw) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    error = ?e,
                    hyprnote.payload.size_bytes = raw.len() as u64,
                    "elevenlabs_json_parse_failed"
                );
                return vec![];
            }
        };

        match msg {
            ElevenLabsMessage::SessionStarted { session_id, .. } => {
                tracing::debug!(
                    hyprnote.stt.provider_session.id = %session_id,
                    "elevenlabs_session_started"
                );
                vec![]
            }
            ElevenLabsMessage::PartialTranscript { text } => {
                if text.is_empty() {
                    return vec![];
                }
                vec![Self::build_response(&text, vec![], false, false, false)]
            }
            ElevenLabsMessage::CommittedTranscript { text } => {
                if text.is_empty() {
                    return vec![];
                }
                vec![Self::build_response(&text, vec![], true, true, false)]
            }
            ElevenLabsMessage::CommittedTranscriptWithTimestamps { text, words } => {
                if text.is_empty() && words.is_empty() {
                    return vec![];
                }
                vec![Self::build_response(&text, words, true, true, false)]
            }
            ElevenLabsMessage::Error {
                error_type,
                message,
            } => {
                tracing::error!(
                    error.type = %error_type,
                    error = %message,
                    "elevenlabs_error"
                );
                vec![StreamResponse::ErrorResponse {
                    error_code: None,
                    error_message: format!("{}: {}", error_type, message),
                    provider: "elevenlabs".to_string(),
                }]
            }
            ElevenLabsMessage::Unknown => {
                tracing::debug!(
                    hyprnote.payload.size_bytes = raw.len() as u64,
                    "elevenlabs_unknown_message"
                );
                vec![]
            }
        }
    }
}

#[derive(Serialize)]
struct AudioChunk<'a> {
    message_type: &'a str,
    audio_base_64: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "message_type")]
enum ElevenLabsMessage {
    #[serde(rename = "session_started")]
    SessionStarted {
        session_id: String,
        #[serde(default)]
        #[allow(dead_code)]
        config: Option<serde_json::Value>,
    },
    #[serde(rename = "partial_transcript")]
    PartialTranscript {
        #[serde(default)]
        text: String,
    },
    #[serde(rename = "committed_transcript")]
    CommittedTranscript {
        #[serde(default)]
        text: String,
    },
    #[serde(rename = "committed_transcript_with_timestamps")]
    CommittedTranscriptWithTimestamps {
        #[serde(default)]
        text: String,
        #[serde(default)]
        words: Vec<ElevenLabsWord>,
    },
    #[serde(rename = "error")]
    Error {
        #[serde(default, rename = "type")]
        error_type: String,
        #[serde(default)]
        message: String,
    },
    #[serde(other)]
    Unknown,
}

impl ElevenLabsAdapter {
    fn build_response(
        text: &str,
        words: Vec<ElevenLabsWord>,
        is_final: bool,
        speech_final: bool,
        from_finalize: bool,
    ) -> StreamResponse {
        let parsed_words: Vec<_> = words
            .iter()
            .filter(|w| w.word_type.as_deref() == Some("word"))
            .map(|w| {
                WordBuilder::new(&w.text)
                    .start(w.start)
                    .end(w.end)
                    .confidence(1.0)
                    .build()
            })
            .collect();

        let (start, duration) = calculate_time_span(&parsed_words);

        let channel = Channel {
            alternatives: vec![Alternatives {
                transcript: text.to_string(),
                words: parsed_words,
                confidence: 1.0,
                languages: vec![],
            }],
        };

        StreamResponse::TranscriptResponse {
            is_final,
            speech_final,
            from_finalize,
            start,
            duration,
            channel,
            metadata: Metadata::default(),
            channel_index: vec![0, 1],
        }
    }
}

#[cfg(test)]
mod tests {
    use hypr_language::ISO639;

    use super::ElevenLabsAdapter;
    use crate::ListenClient;
    use crate::test_utils::{UrlTestCase, run_dual_test, run_single_test, run_url_test_cases};

    const API_BASE: &str = "https://api.elevenlabs.io";

    #[test]
    fn test_default_params() {
        run_url_test_cases(
            &ElevenLabsAdapter::default(),
            API_BASE,
            &[UrlTestCase {
                name: "default_params",
                model: Some("scribe_v2"),
                languages: &[ISO639::En],
                contains: &[
                    "model_id=",
                    "audio_format=pcm_16000",
                    "include_timestamps=true",
                    "commit_strategy=vad",
                    "language_code=en",
                ],
                not_contains: &[],
            }],
        );
    }

    #[test]
    fn test_language_urls() {
        run_url_test_cases(
            &ElevenLabsAdapter::default(),
            API_BASE,
            &[
                UrlTestCase {
                    name: "with_language",
                    model: None,
                    languages: &[ISO639::Es],
                    contains: &["language_code=es"],
                    not_contains: &[],
                },
                UrlTestCase {
                    name: "empty_languages",
                    model: None,
                    languages: &[],
                    contains: &["model_id=", "include_timestamps=true"],
                    not_contains: &["language_code="],
                },
                UrlTestCase {
                    name: "multi_lang_uses_first",
                    model: None,
                    languages: &[ISO639::Fr, ISO639::De],
                    contains: &["language_code=fr"],
                    not_contains: &["language_code=de"],
                },
            ],
        );
    }

    macro_rules! single_test {
        ($name:ident, $params:expr) => {
            #[tokio::test]
            #[ignore]
            async fn $name() {
                let client = ListenClient::builder()
                    .adapter::<ElevenLabsAdapter>()
                    .api_base("https://api.elevenlabs.io")
                    .api_key(
                        std::env::var("ELEVENLABS_API_KEY").expect("ELEVENLABS_API_KEY not set"),
                    )
                    .params($params)
                    .build_single()
                    .await;
                run_single_test(client, "elevenlabs").await;
            }
        };
    }

    single_test!(
        test_build_single,
        owhisper_interface::ListenParams {
            model: Some("scribe_v2".to_string()),
            languages: vec![hypr_language::ISO639::En.into()],
            ..Default::default()
        }
    );

    single_test!(
        test_single_multi_lang_1,
        owhisper_interface::ListenParams {
            model: Some("scribe_v2".to_string()),
            languages: vec![
                hypr_language::ISO639::En.into(),
                hypr_language::ISO639::Es.into(),
            ],
            ..Default::default()
        }
    );

    single_test!(
        test_single_multi_lang_2,
        owhisper_interface::ListenParams {
            model: Some("scribe_v2".to_string()),
            languages: vec![
                hypr_language::ISO639::En.into(),
                hypr_language::ISO639::Ko.into(),
            ],
            ..Default::default()
        }
    );

    #[tokio::test]
    #[ignore]
    async fn test_build_dual() {
        let client = ListenClient::builder()
            .adapter::<ElevenLabsAdapter>()
            .api_base("https://api.elevenlabs.io")
            .api_key(std::env::var("ELEVENLABS_API_KEY").expect("ELEVENLABS_API_KEY not set"))
            .params(owhisper_interface::ListenParams {
                model: Some("scribe_v2".to_string()),
                languages: vec![hypr_language::ISO639::En.into()],
                ..Default::default()
            })
            .build_dual()
            .await;

        run_dual_test(client, "elevenlabs").await;
    }
}
