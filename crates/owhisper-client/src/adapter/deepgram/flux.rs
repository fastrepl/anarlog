use anlg_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{
    Alternatives, Channel, Metadata, ModelInfo, StreamResponse, Word,
};
use serde::Deserialize;

use crate::adapter::{
    DeepgramModel, RealtimeSttAdapter, extract_query_params, set_scheme_from_host,
};

#[derive(Clone, Default)]
pub struct DeepgramFluxAdapter;

impl DeepgramFluxAdapter {
    pub fn is_model(model: &str) -> bool {
        model
            .parse::<DeepgramModel>()
            .is_ok_and(|model| model.is_flux())
    }

    fn endpoint_url(api_base: &str) -> (url::Url, Vec<(String, String)>) {
        let mut url: url::Url = api_base.parse().unwrap_or_else(|_error| {
            tracing::warn!(
                error.type = "invalid_api_base",
                "deepgram_flux_invalid_api_base_using_default"
            );
            "https://api.deepgram.com/v1"
                .parse()
                .expect("invalid_default_deepgram_api_base")
        });
        let existing_params = extract_query_params(&url);
        url.set_query(None);

        let path = url.path().trim_end_matches('/');
        if matches!(path, "/v1" | "/v1/listen" | "/v2" | "/v2/listen") {
            url.set_path("/v2/listen");
        } else {
            crate::adapter::append_path_if_missing(&mut url, "/v2/listen");
        }
        set_scheme_from_host(&mut url);

        (url, existing_params)
    }

    fn build_url(api_base: &str, params: &ListenParams) -> url::Url {
        let (mut url, existing_params) = Self::endpoint_url(api_base);
        let model = params
            .model
            .as_deref()
            .filter(|model| Self::is_model(model))
            .unwrap_or("flux-general-multi");

        {
            let mut query = url.query_pairs_mut();
            for (key, value) in existing_params {
                query.append_pair(&key, &value);
            }
            query
                .append_pair("model", model)
                .append_pair("encoding", "linear16")
                .append_pair("sample_rate", &params.sample_rate.to_string());

            if model == "flux-general-multi" {
                for language in &params.languages {
                    query.append_pair("language_hint", language.iso639().code());
                }
            }
            for keyword in &params.keywords {
                query.append_pair("keyterm", keyword);
            }
            if let Some(custom_query) = &params.custom_query {
                for (key, value) in custom_query {
                    query.append_pair(key, value);
                }
            }
        }

        url
    }

    fn parse_turn(turn: FluxTurnInfo) -> Option<StreamResponse> {
        if turn.transcript.is_empty() && turn.words.is_empty() {
            return None;
        }

        let is_final = turn.event == FluxTurnEvent::EndOfTurn;
        let confidence = turn.end_of_turn_confidence.unwrap_or_else(|| {
            if turn.words.is_empty() {
                1.0
            } else {
                turn.words.iter().map(|word| word.confidence).sum::<f64>() / turn.words.len() as f64
            }
        });
        let languages = turn.languages.unwrap_or_default();
        let word_language = (languages.len() == 1).then(|| languages[0].clone());
        let words = turn
            .words
            .into_iter()
            .map(|word| Word {
                punctuated_word: Some(word.word.clone()),
                language: word_language.clone(),
                word: word.word,
                start: word.start,
                end: word.end,
                confidence: word.confidence,
                speaker: None,
            })
            .collect();

        Some(StreamResponse::TranscriptResponse {
            start: turn.audio_window_start,
            duration: (turn.audio_window_end - turn.audio_window_start).max(0.0),
            is_final,
            speech_final: is_final,
            from_finalize: false,
            channel: Channel {
                alternatives: vec![Alternatives {
                    transcript: turn.transcript,
                    words,
                    confidence,
                    languages,
                }],
            },
            metadata: Metadata {
                request_id: turn.request_id,
                model_info: ModelInfo {
                    name: "flux".to_string(),
                    version: String::new(),
                    arch: String::new(),
                },
                model_uuid: String::new(),
                extra: None,
            },
            channel_index: vec![0, 1],
        })
    }
}

impl RealtimeSttAdapter for DeepgramFluxAdapter {
    fn provider_name(&self) -> &'static str {
        "deepgram"
    }

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
        model: Option<&str>,
    ) -> bool {
        if languages.is_empty() {
            return false;
        }

        let model = model
            .and_then(|model| model.parse::<DeepgramModel>().ok())
            .filter(DeepgramModel::is_flux);
        crate::adapter::DeepgramAdapter::language_support_live(languages, model).is_supported()
    }

    fn supports_native_multichannel(&self) -> bool {
        false
    }

    fn build_ws_url(&self, api_base: &str, params: &ListenParams, _channels: u8) -> url::Url {
        Self::build_url(api_base, params)
    }

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)> {
        api_key.and_then(|key| crate::providers::Provider::Deepgram.build_auth_header(key))
    }

    fn keep_alive_message(&self) -> Option<Message> {
        None
    }

    fn finalize_message(&self) -> Message {
        Message::Text(r#"{"type":"CloseStream"}"#.into())
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let message: FluxMessage = match serde_json::from_str(raw) {
            Ok(message) => message,
            Err(_error) => {
                tracing::warn!(
                    error.type = "invalid_provider_payload",
                    anarlog.payload.size_bytes = raw.len(),
                    "deepgram_flux_response_parse_failed"
                );
                return vec![];
            }
        };

        match message {
            FluxMessage::TurnInfo(turn) => Self::parse_turn(turn).into_iter().collect(),
            FluxMessage::Error(error) => vec![StreamResponse::ErrorResponse {
                error_code: None,
                error_message: error
                    .description
                    .or(error.message)
                    .unwrap_or_else(|| "Deepgram Flux returned an unknown error".to_string()),
                provider: "deepgram".to_string(),
            }],
            FluxMessage::Connected
            | FluxMessage::ConfigureSuccess
            | FluxMessage::ConfigureFailure
            | FluxMessage::Unknown => vec![],
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum FluxMessage {
    TurnInfo(FluxTurnInfo),
    #[serde(alias = "FatalError")]
    Error(FluxError),
    Connected,
    ConfigureSuccess,
    ConfigureFailure,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
struct FluxTurnInfo {
    #[serde(default)]
    request_id: String,
    event: FluxTurnEvent,
    #[serde(default)]
    audio_window_start: f64,
    #[serde(default)]
    audio_window_end: f64,
    #[serde(default)]
    transcript: String,
    #[serde(default)]
    words: Vec<FluxWord>,
    #[serde(default)]
    languages: Option<Vec<String>>,
    #[serde(default)]
    end_of_turn_confidence: Option<f64>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
enum FluxTurnEvent {
    Update,
    StartOfTurn,
    EagerEndOfTurn,
    TurnResumed,
    EndOfTurn,
}

#[derive(Debug, Deserialize)]
struct FluxWord {
    word: String,
    start: f64,
    end: f64,
    confidence: f64,
}

#[derive(Debug, Deserialize)]
struct FluxError {
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

#[cfg(test)]
mod tests {
    use anlg_language::ISO639;

    use super::*;

    #[test]
    fn builds_flux_v2_url_with_language_hints_and_keyterms() {
        let params = ListenParams {
            model: Some("flux-general-multi".to_string()),
            languages: vec![ISO639::En.into(), ISO639::Ja.into()],
            sample_rate: 16000,
            keywords: vec!["Anarlog".to_string()],
            ..Default::default()
        };

        let url = DeepgramFluxAdapter.build_ws_url("https://api.deepgram.com/v1", &params, 1);

        assert_eq!(url.path(), "/v2/listen");
        assert!(url.as_str().starts_with("wss://"));
        assert!(url.as_str().contains("model=flux-general-multi"));
        assert!(url.as_str().contains("language_hint=en"));
        assert!(url.as_str().contains("language_hint=ja"));
        assert!(url.as_str().contains("keyterm=Anarlog"));
        assert!(!url.as_str().contains("channels="));
    }

    #[test]
    fn parses_final_turn_info() {
        let raw = r#"{
            "type": "TurnInfo",
            "request_id": "request-1",
            "event": "EndOfTurn",
            "audio_window_start": 1.0,
            "audio_window_end": 2.5,
            "transcript": "hello world",
            "words": [
                {"word": "hello", "start": 1.0, "end": 1.5, "confidence": 0.9},
                {"word": "world", "start": 1.6, "end": 2.0, "confidence": 0.8}
            ],
            "languages": ["en"],
            "end_of_turn_confidence": 0.95
        }"#;

        let responses = DeepgramFluxAdapter.parse_response(raw);
        let StreamResponse::TranscriptResponse {
            duration,
            is_final,
            channel,
            ..
        } = &responses[0]
        else {
            panic!("expected transcript response");
        };

        assert_eq!(*duration, 1.5);
        assert!(*is_final);
        assert_eq!(channel.alternatives[0].transcript, "hello world");
        assert_eq!(
            channel.alternatives[0].words[0].language.as_deref(),
            Some("en")
        );
    }

    #[test]
    fn parses_flux_error_discriminators() {
        for message_type in ["Error", "FatalError"] {
            let raw = format!(
                r#"{{
                    "type": "{message_type}",
                    "code": "INTERNAL_SERVER_ERROR",
                    "description": "internal failure"
                }}"#
            );

            let responses = DeepgramFluxAdapter.parse_response(&raw);
            let StreamResponse::ErrorResponse {
                error_message,
                provider,
                ..
            } = &responses[0]
            else {
                panic!("expected error response");
            };

            assert_eq!(error_message, "internal failure");
            assert_eq!(provider, "deepgram");
        }
    }

    #[test]
    fn flux_models_are_live_only() {
        let languages = [anlg_language::Language::new(ISO639::En)];
        let model = Some(DeepgramModel::FluxGeneralEn);

        assert!(
            crate::adapter::DeepgramAdapter::language_support_live(&languages, model)
                .is_supported()
        );
        assert!(
            !crate::adapter::DeepgramAdapter::language_support_batch(&languages, model)
                .is_supported()
        );
    }
}
