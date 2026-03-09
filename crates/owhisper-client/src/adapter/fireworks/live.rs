use hypr_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse};
use serde::Deserialize;

use super::FireworksAdapter;
use crate::adapter::RealtimeSttAdapter;
use crate::adapter::parsing::WordBuilder;

// https://docs.fireworks.ai/guides/querying-asr-models#streaming-transcription
// https://docs.fireworks.ai/api-reference/audio-streaming-transcriptions
impl RealtimeSttAdapter for FireworksAdapter {
    fn provider_name(&self) -> &'static str {
        "fireworks"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        FireworksAdapter::is_supported_languages_live(languages)
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

            query_pairs.append_pair("response_format", "verbose_json");
            query_pairs.append_pair("timestamp_granularities", "word,segment");

            if let Some(lang) = params.languages.first() {
                query_pairs.append_pair("language", lang.iso639().code());
            }
        }

        url
    }

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)> {
        api_key.and_then(|k| crate::providers::Provider::Fireworks.build_auth_header(k))
    }

    fn keep_alive_message(&self) -> Option<Message> {
        None
    }

    fn finalize_message(&self) -> Message {
        Message::Text(r#"{"checkpoint_id":"final"}"#.into())
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let msg: FireworksMessage = match serde_json::from_str(raw) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(
                    error.message = ?e,
                    hyprnote.payload.raw = raw,
                    "fireworks_json_parse_failed"
                );
                return vec![];
            }
        };

        if let Some(error) = msg.error {
            tracing::error!(
                error.message = %error.message,
                error.code = ?error.code,
                "fireworks_error"
            );
            return vec![StreamResponse::ErrorResponse {
                error_code: None,
                error_message: error.message,
                provider: "fireworks".to_string(),
            }];
        }

        if msg.checkpoint_id.is_some() {
            return vec![];
        }

        let mut responses = Vec::new();

        if !msg.segments.is_empty() {
            for segment in msg.segments {
                let words_to_use = if segment.words.is_empty() {
                    &msg.words
                } else {
                    &segment.words
                };

                let is_final = words_to_use.iter().all(|w| w.is_final);

                let words: Vec<_> = words_to_use
                    .iter()
                    .map(|w| {
                        WordBuilder::new(&w.word)
                            .start(w.start.unwrap_or(0.0))
                            .end(w.end.unwrap_or(0.0))
                            .confidence(w.probability.unwrap_or(1.0))
                            .language(w.language.clone())
                            .build()
                    })
                    .collect();

                let (start, duration) = if let (Some(first), Some(last)) =
                    (words_to_use.first(), words_to_use.last())
                {
                    let start_secs = first.start.unwrap_or(0.0);
                    let end_secs = last.end.unwrap_or(0.0);
                    (start_secs, end_secs - start_secs)
                } else {
                    (segment.start.unwrap_or(0.0), segment.end.unwrap_or(0.0))
                };

                let channel = Channel {
                    alternatives: vec![Alternatives {
                        transcript: segment.text,
                        words,
                        confidence: 1.0,
                        languages: vec![],
                    }],
                };

                responses.push(StreamResponse::TranscriptResponse {
                    is_final,
                    speech_final: is_final,
                    from_finalize: false,
                    start,
                    duration,
                    channel,
                    metadata: Metadata::default(),
                    channel_index: vec![0, 1],
                });
            }
        } else if !msg.text.is_empty() {
            let is_final = msg.words.iter().all(|w| w.is_final);

            let words: Vec<_> = msg
                .words
                .iter()
                .map(|w| {
                    WordBuilder::new(&w.word)
                        .start(w.start.unwrap_or(0.0))
                        .end(w.end.unwrap_or(0.0))
                        .confidence(w.probability.unwrap_or(1.0))
                        .language(w.language.clone())
                        .build()
                })
                .collect();

            let (start, duration) =
                if let (Some(first), Some(last)) = (msg.words.first(), msg.words.last()) {
                    let start_secs = first.start.unwrap_or(0.0);
                    let end_secs = last.end.unwrap_or(0.0);
                    (start_secs, end_secs - start_secs)
                } else {
                    (0.0, 0.0)
                };

            let channel = Channel {
                alternatives: vec![Alternatives {
                    transcript: msg.text,
                    words,
                    confidence: 1.0,
                    languages: vec![],
                }],
            };

            responses.push(StreamResponse::TranscriptResponse {
                is_final,
                speech_final: is_final,
                from_finalize: false,
                start,
                duration,
                channel,
                metadata: Metadata::default(),
                channel_index: vec![0, 1],
            });
        }

        responses
    }
}

#[derive(Debug, Deserialize)]
struct FireworksMessage {
    #[serde(default)]
    text: String,
    #[serde(default)]
    words: Vec<FireworksWord>,
    #[serde(default)]
    segments: Vec<Segment>,
    #[serde(default)]
    checkpoint_id: Option<String>,
    #[serde(default)]
    error: Option<FireworksError>,
}

#[derive(Debug, Deserialize)]
struct FireworksError {
    #[serde(default)]
    message: String,
    #[serde(default)]
    code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Segment {
    #[allow(dead_code)]
    id: String,
    text: String,
    #[serde(default)]
    start: Option<f64>,
    #[serde(default)]
    end: Option<f64>,
    #[serde(default)]
    words: Vec<FireworksWord>,
}

#[derive(Debug, Deserialize)]
struct FireworksWord {
    word: String,
    #[serde(default)]
    start: Option<f64>,
    #[serde(default)]
    end: Option<f64>,
    #[serde(default)]
    probability: Option<f64>,
    #[serde(default)]
    is_final: bool,
    #[serde(default)]
    language: Option<String>,
}

#[cfg(test)]
mod tests {
    use hypr_language::ISO639;

    use super::FireworksAdapter;
    use crate::ListenClient;
    use crate::test_utils::{UrlTestCase, run_dual_test, run_single_test, run_url_test_cases};

    const API_BASE: &str = "https://api.fireworks.ai";

    #[test]
    fn test_default_params() {
        run_url_test_cases(
            &FireworksAdapter::default(),
            API_BASE,
            &[UrlTestCase {
                name: "default_params",
                model: None,
                languages: &[ISO639::En],
                contains: &[
                    "response_format=verbose_json",
                    "timestamp_granularities=word",
                    "language=en",
                ],
                not_contains: &[],
            }],
        );
    }

    #[test]
    fn test_language_urls() {
        run_url_test_cases(
            &FireworksAdapter::default(),
            API_BASE,
            &[
                UrlTestCase {
                    name: "with_language",
                    model: None,
                    languages: &[ISO639::Es],
                    contains: &["language=es"],
                    not_contains: &[],
                },
                UrlTestCase {
                    name: "empty_languages",
                    model: None,
                    languages: &[],
                    contains: &["response_format=verbose_json"],
                    not_contains: &["language="],
                },
                UrlTestCase {
                    name: "multi_lang_uses_first",
                    model: None,
                    languages: &[ISO639::Fr, ISO639::De],
                    contains: &["language=fr"],
                    not_contains: &["language=de"],
                },
            ],
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_build_single() {
        let client = ListenClient::builder()
            .adapter::<FireworksAdapter>()
            .api_base("https://api.fireworks.ai")
            .api_key(std::env::var("FIREWORKS_API_KEY").expect("FIREWORKS_API_KEY not set"))
            .params(owhisper_interface::ListenParams {
                languages: vec![hypr_language::ISO639::En.into()],
                ..Default::default()
            })
            .build_single()
            .await;

        run_single_test(client, "fireworks").await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_build_dual() {
        let client = ListenClient::builder()
            .adapter::<FireworksAdapter>()
            .api_base("https://api.fireworks.ai")
            .api_key(std::env::var("FIREWORKS_API_KEY").expect("FIREWORKS_API_KEY not set"))
            .params(owhisper_interface::ListenParams {
                languages: vec![hypr_language::ISO639::En.into()],
                ..Default::default()
            })
            .build_dual()
            .await;

        run_dual_test(client, "fireworks").await;
    }
}
