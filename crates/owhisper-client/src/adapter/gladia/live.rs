// https://docs.gladia.io/api-reference/v2/live/init

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use hypr_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};
use serde::{Deserialize, Serialize};

use super::GladiaAdapter;
use crate::adapter::RealtimeSttAdapter;
use crate::adapter::parsing::WordBuilder;

struct SessionChannels;

impl SessionChannels {
    fn store() -> &'static Mutex<HashMap<String, u8>> {
        static SESSION_CHANNELS: OnceLock<Mutex<HashMap<String, u8>>> = OnceLock::new();
        SESSION_CHANNELS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn insert(session_id: String, channels: u8) {
        if let Ok(mut map) = Self::store().lock() {
            map.insert(session_id, channels);
        }
    }

    fn get(session_id: &str) -> Option<u8> {
        Self::store()
            .lock()
            .ok()
            .and_then(|map| map.get(session_id).copied())
    }

    fn remove(session_id: &str) -> Option<u8> {
        Self::store()
            .lock()
            .ok()
            .and_then(|mut map| map.remove(session_id))
    }

    fn get_or_infer(session_id: &str, channel_idx: i32) -> u8 {
        Self::get(session_id).unwrap_or_else(|| (channel_idx + 1).max(1) as u8)
    }
}

impl RealtimeSttAdapter for GladiaAdapter {
    fn provider_name(&self) -> &'static str {
        "gladia"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        GladiaAdapter::is_supported_languages_live(languages)
    }

    fn supports_native_multichannel(&self) -> bool {
        true
    }

    fn build_ws_url(&self, api_base: &str, _params: &ListenParams, _channels: u8) -> url::Url {
        let (mut url, existing_params) = Self::build_ws_url_from_base(api_base);

        if !existing_params.is_empty() {
            let mut query_pairs = url.query_pairs_mut();
            for (key, value) in &existing_params {
                query_pairs.append_pair(key, value);
            }
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
        let api_base = api_base.to_string();
        let params = params.clone();
        let api_key = api_key.map(ToString::to_string);

        async move {
            if let Some(proxy_result) = crate::adapter::build_proxy_ws_url(&api_base) {
                let (mut url, existing_params) = proxy_result;
                if !existing_params.is_empty() {
                    let mut query_pairs = url.query_pairs_mut();
                    for (key, value) in &existing_params {
                        query_pairs.append_pair(key, value);
                    }
                }
                return Some(url);
            }

            let key = api_key.as_deref()?;
            let post_url = Self::build_http_url(&api_base);

            let languages: Vec<String> = params
                .languages
                .iter()
                .map(|l| l.iso639().code().to_string())
                .collect();

            let language_config = if languages.is_empty() {
                None
            } else {
                Some(LanguageConfig {
                    code_switching: languages.len() > 1,
                    languages,
                })
            };

            let default = crate::providers::Provider::Gladia.default_live_model();
            let model = match params.model.as_deref() {
                Some(m) if crate::providers::is_meta_model(m) => Some(default),
                Some(m) => Some(m),
                None => None,
            };

            let has_keywords = !params.keywords.is_empty();
            let custom_vocabulary_config = has_keywords.then(|| CustomVocabularyConfig {
                vocabulary: params
                    .keywords
                    .iter()
                    .map(|k| CustomVocabularyEntry::Simple(k.clone()))
                    .collect(),
                default_intensity: None,
            });

            let body = GladiaConfig {
                model,
                encoding: "wav/pcm",
                sample_rate: params.sample_rate,
                bit_depth: 16,
                channels,
                language_config,
                custom_metadata: None,
                messages_config: Some(MessagesConfig {
                    receive_partial_transcripts: true,
                    receive_final_transcripts: true,
                }),
                pre_processing: Some(PreProcessing {
                    audio_enhancer: true,
                }),
                realtime_processing: Some(RealtimeProcessing {
                    words_accurate_timestamps: true,
                    custom_vocabulary: has_keywords,
                    custom_vocabulary_config,
                }),
            };

            let client = reqwest::Client::new();
            let resp = client
                .post(post_url.as_str())
                .header("x-gladia-key", key)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| {
                    tracing::error!(error.message = ?e, "gladia_init_request_failed");
                })
                .ok()?;

            let init: InitResponse = resp
                .json()
                .await
                .map_err(|e| {
                    tracing::error!(error.message = ?e, "gladia_init_parse_failed");
                })
                .ok()?;

            let (id, url) = match init {
                InitResponse::Success { id, url } => (id, url),
                InitResponse::Error {
                    message,
                    validation_errors,
                } => {
                    tracing::error!(
                        error.message = %message,
                        hyprnote.validation.errors = ?validation_errors,
                        "gladia_init_failed"
                    );
                    return None;
                }
            };

            SessionChannels::insert(id, channels);

            url::Url::parse(&url).ok()
        }
    }

    fn build_auth_header(&self, _api_key: Option<&str>) -> Option<(&'static str, String)> {
        None
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

    fn finalize_message(&self) -> Message {
        Message::Text(r#"{"type":"stop_recording"}"#.into())
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let msg: GladiaMessage = match serde_json::from_str(raw) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    error.message = ?e,
                    hyprnote.payload.raw = raw,
                    "gladia_json_parse_failed"
                );
                return vec![];
            }
        };

        match msg {
            GladiaMessage::Transcript(transcript) => Self::parse_transcript(transcript),
            GladiaMessage::StartSession { id } => {
                tracing::debug!(hyprnote.stt.provider_session.id = %id, "gladia_session_started");
                vec![]
            }
            GladiaMessage::EndSession { id } => {
                let channels = SessionChannels::remove(&id).unwrap_or_else(|| {
                    tracing::warn!(
                        hyprnote.stt.provider_session.id = %id,
                        "gladia_session_channels_not_found"
                    );
                    1
                });
                tracing::debug!(
                    hyprnote.stt.provider_session.id = %id,
                    hyprnote.audio.channel_count = channels,
                    "gladia_session_ended"
                );
                vec![StreamResponse::TerminalResponse {
                    request_id: id,
                    created: String::new(),
                    duration: 0.0,
                    channels: channels.into(),
                }]
            }
            GladiaMessage::SpeechStart { .. } => vec![],
            GladiaMessage::SpeechEnd { .. } => vec![],
            GladiaMessage::StartRecording { .. } => vec![],
            GladiaMessage::EndRecording { .. } => vec![],
            GladiaMessage::Error { message, code } => {
                tracing::error!(error.message = %message, error.code = ?code, "gladia_error");
                vec![StreamResponse::ErrorResponse {
                    error_code: code,
                    error_message: message,
                    provider: "gladia".to_string(),
                }]
            }
            GladiaMessage::Unknown => {
                tracing::debug!(hyprnote.payload.raw = raw, "gladia_unknown_message");
                vec![]
            }
        }
    }
}

#[derive(Serialize)]
struct GladiaConfig<'a> {
    encoding: &'a str,
    sample_rate: u32,
    bit_depth: u8,
    channels: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language_config: Option<LanguageConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    custom_metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    messages_config: Option<MessagesConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pre_processing: Option<PreProcessing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    realtime_processing: Option<RealtimeProcessing>,
}

#[derive(Serialize, Debug, PartialEq)]
struct LanguageConfig {
    languages: Vec<String>,
    code_switching: bool,
}

impl GladiaAdapter {
    #[cfg(test)]
    fn build_language_config(params: &ListenParams) -> Option<LanguageConfig> {
        let languages: Vec<String> = params
            .languages
            .iter()
            .map(|l| l.iso639().code().to_string())
            .collect();

        if languages.is_empty() {
            None
        } else {
            Some(LanguageConfig {
                code_switching: languages.len() > 1,
                languages,
            })
        }
    }
}

#[derive(Serialize)]
struct MessagesConfig {
    receive_partial_transcripts: bool,
    receive_final_transcripts: bool,
}

#[derive(Serialize)]
struct PreProcessing {
    audio_enhancer: bool,
}

#[derive(Serialize)]
struct RealtimeProcessing {
    words_accurate_timestamps: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    custom_vocabulary: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    custom_vocabulary_config: Option<CustomVocabularyConfig>,
}

#[derive(Serialize)]
struct CustomVocabularyConfig {
    vocabulary: Vec<CustomVocabularyEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_intensity: Option<f64>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum CustomVocabularyEntry {
    Simple(String),
    #[allow(dead_code)]
    Detailed {
        value: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pronunciations: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        intensity: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        language: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum InitResponse {
    Success {
        id: String,
        url: String,
    },
    Error {
        message: String,
        #[serde(default)]
        validation_errors: Vec<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum GladiaMessage {
    #[serde(rename = "transcript")]
    Transcript(TranscriptMessage),
    #[serde(rename = "start_session")]
    StartSession { id: String },
    #[serde(rename = "end_session")]
    EndSession { id: String },
    #[serde(rename = "speech_start")]
    SpeechStart {
        #[serde(default)]
        session_id: Option<String>,
    },
    #[serde(rename = "speech_end")]
    SpeechEnd {
        #[serde(default)]
        session_id: Option<String>,
    },
    #[serde(rename = "start_recording")]
    StartRecording {
        #[serde(default)]
        session_id: Option<String>,
    },
    #[serde(rename = "end_recording")]
    EndRecording {
        #[serde(default)]
        session_id: Option<String>,
    },
    #[serde(rename = "error")]
    Error {
        message: String,
        #[serde(default)]
        code: Option<i32>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct TranscriptMessage {
    #[serde(default)]
    session_id: String,
    data: TranscriptData,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TranscriptData {
    #[serde(default)]
    id: String,
    #[serde(default)]
    is_final: bool,
    utterance: Utterance,
}

#[derive(Debug, Deserialize)]
struct Utterance {
    #[serde(default)]
    text: String,
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    channel: Option<i32>,
    #[serde(default)]
    words: Vec<GladiaWord>,
}

#[derive(Debug, Deserialize)]
struct GladiaWord {
    #[serde(default)]
    word: String,
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
    #[serde(default)]
    confidence: f64,
}

impl GladiaAdapter {
    fn parse_transcript(msg: TranscriptMessage) -> Vec<StreamResponse> {
        let session_id = msg.session_id;
        let data = msg.data;
        let utterance = data.utterance;

        if utterance.text.is_empty() && utterance.words.is_empty() {
            return vec![];
        }

        let is_final = data.is_final;
        let speech_final = data.is_final;
        let from_finalize = false;

        let words: Vec<_> = utterance
            .words
            .iter()
            .map(|w| {
                WordBuilder::new(&w.word)
                    .start(w.start)
                    .end(w.end)
                    .confidence(w.confidence)
                    .language(utterance.language.clone())
                    .build()
            })
            .collect();

        let start = utterance.start;
        let duration = utterance.end - utterance.start;

        let channel = Channel {
            alternatives: vec![Alternatives {
                transcript: utterance.text,
                words,
                confidence: 1.0,
                languages: utterance.language.map(|l| vec![l]).unwrap_or_default(),
            }],
        };

        let channel_idx = utterance.channel.unwrap_or(0);
        let total_channels = SessionChannels::get_or_infer(&session_id, channel_idx);

        vec![StreamResponse::TranscriptResponse {
            is_final,
            speech_final,
            from_finalize,
            start,
            duration,
            channel,
            metadata: Metadata::default(),
            channel_index: vec![channel_idx, total_channels as i32],
        }]
    }
}

#[cfg(test)]
mod tests {
    use hypr_language::ISO639;

    use super::{GladiaAdapter, LanguageConfig};
    use crate::ListenClient;
    use crate::test_utils::{UrlTestCase, run_dual_test, run_single_test, run_url_test_cases};

    const API_BASE: &str = "https://api.gladia.io";

    #[test]
    fn test_base_url() {
        run_url_test_cases(
            &GladiaAdapter::default(),
            API_BASE,
            &[UrlTestCase {
                name: "base_url_structure",
                model: None,
                languages: &[ISO639::En],
                contains: &["api.gladia.io"],
                not_contains: &[],
            }],
        );
    }

    #[test]
    fn test_build_language_config_single_language() {
        let params = owhisper_interface::ListenParams {
            languages: vec![hypr_language::ISO639::En.into()],
            ..Default::default()
        };

        let config = GladiaAdapter::build_language_config(&params).unwrap();

        assert_eq!(config.languages, vec!["en"]);
        assert!(
            !config.code_switching,
            "Single language should have code_switching=false"
        );
    }

    #[test]
    fn test_build_language_config_multi_language() {
        let params = owhisper_interface::ListenParams {
            languages: vec![
                hypr_language::ISO639::En.into(),
                hypr_language::ISO639::Es.into(),
            ],
            ..Default::default()
        };

        let config = GladiaAdapter::build_language_config(&params).unwrap();

        assert_eq!(config.languages, vec!["en", "es"]);
        assert!(
            config.code_switching,
            "Multi language should have code_switching=true"
        );
    }

    #[test]
    fn test_build_language_config_three_languages() {
        let params = owhisper_interface::ListenParams {
            languages: vec![
                hypr_language::ISO639::En.into(),
                hypr_language::ISO639::Ko.into(),
                hypr_language::ISO639::Ja.into(),
            ],
            ..Default::default()
        };

        let config = GladiaAdapter::build_language_config(&params).unwrap();

        assert_eq!(config.languages, vec!["en", "ko", "ja"]);
        assert!(
            config.code_switching,
            "Three languages should have code_switching=true"
        );
    }

    #[test]
    fn test_build_language_config_empty_languages() {
        let params = owhisper_interface::ListenParams {
            languages: vec![],
            ..Default::default()
        };

        let config = GladiaAdapter::build_language_config(&params);

        assert!(config.is_none(), "Empty languages should return None");
    }

    #[test]
    fn test_build_language_config_serialization() {
        let config = LanguageConfig {
            languages: vec!["en".to_string(), "fr".to_string()],
            code_switching: true,
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"code_switching\":true"));
        assert!(json.contains("\"languages\":[\"en\",\"fr\"]"));
    }

    macro_rules! single_test {
        ($name:ident, $params:expr) => {
            #[tokio::test]
            #[ignore]
            async fn $name() {
                let client = ListenClient::builder()
                    .adapter::<GladiaAdapter>()
                    .api_base("https://api.gladia.io")
                    .api_key(std::env::var("GLADIA_API_KEY").expect("GLADIA_API_KEY not set"))
                    .params($params)
                    .build_single()
                    .await;
                run_single_test(client, "gladia").await;
            }
        };
    }

    single_test!(
        test_build_single,
        owhisper_interface::ListenParams {
            languages: vec![hypr_language::ISO639::En.into()],
            ..Default::default()
        }
    );

    single_test!(
        test_single_with_keywords,
        owhisper_interface::ListenParams {
            languages: vec![hypr_language::ISO639::En.into()],
            keywords: vec!["Hyprnote".to_string(), "transcription".to_string()],
            ..Default::default()
        }
    );

    single_test!(
        test_single_multi_lang_1,
        owhisper_interface::ListenParams {
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
            .adapter::<GladiaAdapter>()
            .api_base("https://api.gladia.io")
            .api_key(std::env::var("GLADIA_API_KEY").expect("GLADIA_API_KEY not set"))
            .params(owhisper_interface::ListenParams {
                languages: vec![hypr_language::ISO639::En.into()],
                ..Default::default()
            })
            .build_dual()
            .await;

        run_dual_test(client, "gladia").await;
    }
}
