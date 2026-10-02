// https://docs.gladia.io/api-reference/v2/live/init

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anlg_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};
use serde::{Deserialize, Serialize};

use super::GladiaAdapter;
use crate::adapter::RealtimeSttAdapter;
use crate::adapter::parsing::WordBuilder;

struct SessionChannels;

struct SessionChannelEntry {
    channels: u8,
    inserted_at: Instant,
}

const SESSION_CHANNEL_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_SESSION_CHANNELS: usize = 64;

impl SessionChannels {
    fn store() -> &'static Mutex<HashMap<String, SessionChannelEntry>> {
        static SESSION_CHANNELS: OnceLock<Mutex<HashMap<String, SessionChannelEntry>>> =
            OnceLock::new();
        SESSION_CHANNELS.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn insert(session_id: String, channels: u8) {
        if let Ok(mut map) = Self::store().lock() {
            Self::prune(&mut map);
            if !map.contains_key(&session_id)
                && map.len() >= MAX_SESSION_CHANNELS
                && let Some(oldest) = map
                    .iter()
                    .min_by_key(|(_, entry)| entry.inserted_at)
                    .map(|(id, _)| id.clone())
            {
                map.remove(&oldest);
            }
            map.insert(
                session_id,
                SessionChannelEntry {
                    channels,
                    inserted_at: Instant::now(),
                },
            );
        }
    }

    fn get(session_id: &str) -> Option<u8> {
        let mut map = Self::store().lock().ok()?;
        Self::prune(&mut map);
        map.get(session_id).map(|entry| entry.channels)
    }

    fn remove(session_id: &str) -> Option<u8> {
        Self::store()
            .lock()
            .ok()
            .and_then(|mut map| map.remove(session_id).map(|entry| entry.channels))
    }

    #[cfg(test)]
    fn clear() {
        if let Ok(mut map) = Self::store().lock() {
            map.clear();
        }
    }

    fn get_or_infer(session_id: &str, channel_idx: i32) -> u8 {
        Self::get(session_id).unwrap_or_else(|| (channel_idx + 1).max(1) as u8)
    }

    fn prune(map: &mut HashMap<String, SessionChannelEntry>) {
        map.retain(|_, entry| entry.inserted_at.elapsed() < SESSION_CHANNEL_TTL);
    }

    #[cfg(test)]
    fn len() -> usize {
        Self::store().lock().map(|map| map.len()).unwrap_or(0)
    }
}

impl RealtimeSttAdapter for GladiaAdapter {
    fn provider_name(&self) -> &'static str {
        "gladia"
    }

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> bool {
        GladiaAdapter::is_supported_languages_live(languages, model)
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
                .map(|language| super::language::provider_code(language).to_string())
                .collect();

            let language_config = if languages.is_empty() {
                None
            } else {
                Some(LanguageConfig {
                    code_switching: false,
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
                .map_err(|_e| {
                    tracing::error!(error.type = "provider_request_failed", "gladia_init_request_failed");
                })
                .ok()?;

            let init: InitResponse = resp
                .json()
                .await
                .map_err(|_e| {
                    tracing::error!(error.type = "invalid_provider_payload", "gladia_init_parse_failed");
                })
                .ok()?;

            let (id, url) = match init {
                InitResponse::Success { id, url } => (id, url),
                InitResponse::Error {
                    message,
                    validation_errors: _,
                } => {
                    crate::log_provider_failure("gladia", "session_init_failed", None, &message);
                    return None;
                }
            };

            let url = url::Url::parse(&url).ok()?;
            SessionChannels::insert(id, channels);
            Some(url)
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
            Err(_e) => {
                tracing::warn!(
                    error.type = "invalid_provider_payload",
                    anarlog.payload.size_bytes = raw.len() as u64,
                    "gladia_json_parse_failed"
                );
                return vec![];
            }
        };

        match msg {
            GladiaMessage::Transcript(transcript) => Self::parse_transcript(transcript),
            GladiaMessage::StartSession { id } => {
                tracing::debug!("gladia_session_started");
                let _ = id;
                vec![]
            }
            GladiaMessage::EndSession { id } => {
                let channels = SessionChannels::remove(&id).unwrap_or_else(|| {
                    tracing::warn!("gladia_session_channels_not_found");
                    1
                });
                tracing::debug!(
                    anarlog.audio.channel_count = channels,
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
            GladiaMessage::Error {
                message,
                code,
                session_id,
            } => {
                if let Some(session_id) = session_id {
                    SessionChannels::remove(&session_id);
                }
                crate::log_provider_failure("gladia", "provider_error", None, &message);
                vec![StreamResponse::ErrorResponse {
                    error_code: code,
                    error_message: message,
                    provider: "gladia".to_string(),
                }]
            }
            GladiaMessage::Unknown => {
                tracing::debug!(
                    anarlog.payload.size_bytes = raw.len() as u64,
                    "gladia_unknown_message"
                );
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

// `languages` is a candidate set Gladia detects within; `code_switching` additionally lets it
// switch language mid-audio. Configured languages describe the user, not one meeting, so leave
// switching off — otherwise a monolingual German meeting is decoded by the code-switching path
// and loses accuracy against detecting German once and locking to it.
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
            .map(|language| super::language::provider_code(language).to_string())
            .collect();

        if languages.is_empty() {
            None
        } else {
            Some(LanguageConfig {
                code_switching: false,
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
        #[serde(default, alias = "id")]
        session_id: Option<String>,
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
    use anlg_language::ISO639;

    use super::{GladiaAdapter, LanguageConfig, MAX_SESSION_CHANNELS, SessionChannels};
    use crate::RealtimeSttAdapter;
    use crate::test_utils::{UrlTestCase, run_url_test_cases};

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
    fn test_build_language_config() {
        for (languages, expected) in [
            (vec![anlg_language::ISO639::En], vec!["en"]),
            (
                vec![anlg_language::ISO639::En, anlg_language::ISO639::Es],
                vec!["en", "es"],
            ),
            (
                vec![
                    anlg_language::ISO639::En,
                    anlg_language::ISO639::Ko,
                    anlg_language::ISO639::Ja,
                ],
                vec!["en", "ko", "ja"],
            ),
        ] {
            let params = owhisper_interface::ListenParams {
                languages: languages.into_iter().map(Into::into).collect(),
                ..Default::default()
            };

            let config = GladiaAdapter::build_language_config(&params).unwrap();

            assert_eq!(config.languages, expected);
            assert!(
                !config.code_switching,
                "language detection should stay within the candidates, not code-switch"
            );
        }

        let params = owhisper_interface::ListenParams {
            languages: vec![],
            ..Default::default()
        };
        assert!(
            GladiaAdapter::build_language_config(&params).is_none(),
            "Empty languages should return None"
        );
    }

    #[test]
    fn test_build_language_config_serialization() {
        let config = LanguageConfig {
            languages: vec!["en".to_string(), "fr".to_string()],
            code_switching: false,
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"code_switching\":false"));
        assert!(json.contains("\"languages\":[\"en\",\"fr\"]"));
    }

    #[test]
    fn session_channels_clean_up_terminal_messages_and_stay_bounded() {
        SessionChannels::clear();
        SessionChannels::insert("session-1".to_string(), 2);
        SessionChannels::insert("session-2".to_string(), 1);

        GladiaAdapter
            .parse_response(r#"{"type":"error","message":"closed","session_id":"session-1"}"#);
        assert_eq!(SessionChannels::get("session-1"), None);
        assert_eq!(SessionChannels::get("session-2"), Some(1));

        GladiaAdapter.parse_response(r#"{"type":"end_session","id":"session-2"}"#);
        assert_eq!(SessionChannels::len(), 0);

        for index in 0..=MAX_SESSION_CHANNELS {
            SessionChannels::insert(format!("session-{index}"), 2);
        }
        assert_eq!(SessionChannels::len(), MAX_SESSION_CHANNELS);

        GladiaAdapter.parse_response(r#"{"type":"error","message":"closed"}"#);
        assert_eq!(SessionChannels::len(), MAX_SESSION_CHANNELS);
        SessionChannels::clear();
    }
}
