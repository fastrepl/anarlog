use std::sync::atomic::Ordering;

use anlg_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse, Word};

use crate::adapter::{RealtimeSttAdapter, build_proxy_ws_url, build_url_with_scheme};
use crate::providers::Provider;

use super::XaiAdapter;

impl RealtimeSttAdapter for XaiAdapter {
    fn provider_name(&self) -> &'static str {
        "xai"
    }

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
        _model: Option<&str>,
    ) -> bool {
        Self::language_support_live(languages).is_supported()
    }

    fn supports_native_multichannel(&self) -> bool {
        true
    }

    fn build_ws_url(&self, api_base: &str, params: &ListenParams, channels: u8) -> url::Url {
        self.live_channels.store(channels.max(1), Ordering::Relaxed);

        let (mut url, existing_params) = if let Some(result) = build_proxy_ws_url(api_base) {
            result
        } else {
            let parsed: url::Url = if api_base.is_empty() {
                Provider::Xai
                    .default_api_base()
                    .parse()
                    .expect("invalid_default_xai_api_base")
            } else {
                api_base.parse().expect("invalid_xai_api_base")
            };
            (
                build_url_with_scheme(
                    &parsed,
                    Provider::Xai.default_ws_host(),
                    Provider::Xai.ws_path(),
                    true,
                ),
                Vec::new(),
            )
        };

        {
            let mut query = url.query_pairs_mut();
            query.extend_pairs(&existing_params);
            query.append_pair("sample_rate", &params.sample_rate.to_string());
            query.append_pair("encoding", "pcm");
            query.append_pair("interim_results", "true");

            if let Some(language) = params.languages.first() {
                query.append_pair("language", language.iso639().code());
            }
            for keyword in &params.keywords {
                query.append_pair("keyterm", keyword);
            }
            if channels > 1 {
                query.append_pair("multichannel", "true");
                query.append_pair("channels", &channels.to_string());
            }
            if params.num_speakers.is_some()
                || params.min_speakers.is_some()
                || params.max_speakers.is_some()
            {
                query.append_pair("diarize", "true");
            }
        }

        url
    }

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)> {
        api_key.and_then(|key| Provider::Xai.build_auth_header(key))
    }

    fn keep_alive_message(&self) -> Option<Message> {
        None
    }

    fn finalize_message(&self) -> Message {
        Message::Text(r#"{"type":"audio.done"}"#.into())
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let event: XaiEvent = match serde_json::from_str(raw) {
            Ok(event) => event,
            Err(_error) => {
                tracing::warn!(
                    error.type = "invalid_provider_payload",
                    anarlog.payload.size_bytes = raw.len() as u64,
                    "xai_json_parse_failed"
                );
                return Vec::new();
            }
        };

        match event.event_type.as_str() {
            "transcript.created" => Vec::new(),
            "error" => vec![StreamResponse::ErrorResponse {
                error_code: None,
                error_message: event
                    .message
                    .unwrap_or_else(|| "xAI transcription error".into()),
                provider: "xai".to_string(),
            }],
            "transcript.done" if event.text.trim().is_empty() => {
                vec![StreamResponse::TerminalResponse {
                    request_id: String::new(),
                    created: String::new(),
                    duration: event.duration.unwrap_or_default(),
                    channels: 1,
                }]
            }
            "transcript.partial" | "transcript.done" => {
                let is_done = event.event_type == "transcript.done";
                let channel_index = event.channel_index.unwrap_or(0);
                let total_channels = self.live_channels.load(Ordering::Relaxed).max(1);
                vec![StreamResponse::TranscriptResponse {
                    start: event.start.unwrap_or_default(),
                    duration: event.duration.unwrap_or_default(),
                    is_final: is_done || event.is_final.unwrap_or(false),
                    speech_final: is_done || event.speech_final.unwrap_or(false),
                    from_finalize: is_done,
                    channel: Channel {
                        alternatives: vec![Alternatives {
                            transcript: event.text,
                            words: event
                                .words
                                .into_iter()
                                .map(|word| Word {
                                    punctuated_word: Some(word.text.clone()),
                                    word: word.text,
                                    start: word.start,
                                    end: word.end,
                                    confidence: 1.0,
                                    speaker: word.speaker,
                                    language: None,
                                })
                                .collect(),
                            confidence: 1.0,
                            languages: Vec::new(),
                        }],
                    },
                    metadata: Metadata::default(),
                    channel_index: vec![channel_index, i32::from(total_channels)],
                }]
            }
            _ => Vec::new(),
        }
    }
}

#[derive(serde::Deserialize)]
struct XaiEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    words: Vec<XaiWord>,
    #[serde(default)]
    is_final: Option<bool>,
    #[serde(default)]
    speech_final: Option<bool>,
    #[serde(default)]
    start: Option<f64>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    channel_index: Option<i32>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(serde::Deserialize)]
struct XaiWord {
    text: String,
    start: f64,
    end: f64,
    #[serde(default)]
    speaker: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_direct_and_proxy_urls() {
        let params = ListenParams {
            sample_rate: 16_000,
            languages: vec![anlg_language::ISO639::En.into()],
            keywords: vec!["Anarlog".to_string()],
            ..Default::default()
        };

        let adapter = XaiAdapter::default();
        let direct = adapter.build_ws_url("https://api.x.ai/v1", &params, 2);
        let proxy = adapter.build_ws_url("https://api.anarlog.so/stt?provider=xai", &params, 1);

        assert_eq!(direct.scheme(), "wss");
        assert_eq!(direct.path(), "/v1/stt");
        assert!(direct.query().unwrap().contains("multichannel=true"));
        assert_eq!(
            proxy.as_str().split('?').next().unwrap(),
            "wss://api.anarlog.so/stt/listen"
        );
        assert!(proxy.query().unwrap().contains("provider=xai"));
    }

    #[test]
    fn parses_partial_transcript() {
        let responses = XaiAdapter::default().parse_response(
            r#"{"type":"transcript.partial","text":"hello","is_final":true,"speech_final":true,"start":0.1,"duration":0.5,"words":[{"text":"hello","start":0.1,"end":0.6,"speaker":1}]}"#,
        );

        let StreamResponse::TranscriptResponse {
            is_final,
            speech_final,
            channel,
            ..
        } = &responses[0]
        else {
            panic!("expected transcript response");
        };
        assert!(*is_final);
        assert!(*speech_final);
        assert_eq!(channel.alternatives[0].words[0].speaker, Some(1));
    }

    #[test]
    fn reports_the_configured_multichannel_total() {
        let adapter = XaiAdapter::default();
        adapter.build_ws_url("https://api.x.ai/v1", &ListenParams::default(), 2);

        let responses = adapter
            .parse_response(r#"{"type":"transcript.partial","text":"hello","channel_index":1}"#);

        let StreamResponse::TranscriptResponse { channel_index, .. } = &responses[0] else {
            panic!("expected transcript response");
        };
        assert_eq!(channel_index, &[1, 2]);
    }
}
