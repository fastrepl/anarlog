use std::path::Path;

use base64::Engine;
use owhisper_interface::ListenParams;
use owhisper_interface::batch::Response;

use crate::adapter::openai_compatible_batch::{self, OpenAICompatibleBatchConfig, transcribe};
use crate::adapter::{
    BatchFuture, BatchSttAdapter, ClientWithMiddleware, LanguageQuality, LanguageSupport,
    append_path_if_missing,
};
use crate::error::Error;

const DEFAULT_API_BASE: &str = "https://openrouter.ai/api/v1";
const DEFAULT_MODEL: &str = "openai/gpt-transcribe";
const TRANSCRIPTION_PATH: &str = "audio/transcriptions";

// App attribution per https://openrouter.ai/docs/app-attribution
const APP_REFERER: &str = "https://anarlog.so";
const APP_TITLE: &str = "Anarlog";
const APP_CATEGORIES: &str = "writing-assistant,personal-agent";

// Shared with `openai_compatible_batch::transcribe`, whose multipart path
// also serves non-OpenRouter providers and so must gate this explicitly.
pub(crate) fn with_attribution_headers(
    builder: reqwest_middleware::RequestBuilder,
) -> reqwest_middleware::RequestBuilder {
    builder
        .header("HTTP-Referer", APP_REFERER)
        .header("X-Title", APP_TITLE)
        .header("X-OpenRouter-Categories", APP_CATEGORIES)
}

#[derive(Clone, Default)]
pub struct OpenRouterAdapter;

impl OpenRouterAdapter {
    pub fn language_support_batch(_languages: &[anlg_language::Language]) -> LanguageSupport {
        LanguageSupport::Supported {
            quality: LanguageQuality::NoData,
        }
    }
}

impl BatchSttAdapter for OpenRouterAdapter {
    fn provider_name(&self) -> &'static str {
        "openrouter"
    }

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
        _model: Option<&str>,
    ) -> bool {
        Self::language_support_batch(languages).is_supported()
    }

    fn transcribe_file<'a, P: AsRef<Path> + Send + 'a>(
        &'a self,
        client: &'a ClientWithMiddleware,
        api_base: &'a str,
        api_key: &'a str,
        params: &'a ListenParams,
        file_path: P,
    ) -> BatchFuture<'a> {
        let path = file_path.as_ref().to_path_buf();
        Box::pin(async move {
            let model = openai_compatible_batch::resolve_model(params, DEFAULT_MODEL);

            match provider_options_for_model(model, params) {
                Some(request) => {
                    transcribe_with_provider_options(
                        client, api_base, api_key, params, &path, model, request,
                    )
                    .await
                }
                None => {
                    transcribe(
                        client,
                        api_base,
                        api_key,
                        params,
                        &path,
                        OpenAICompatibleBatchConfig {
                            provider: "openrouter",
                            default_api_base: DEFAULT_API_BASE,
                            default_model: DEFAULT_MODEL,
                            transcription_path: TRANSCRIPTION_PATH,
                            response_format: None,
                            timestamp_field: None,
                            include_language: true,
                        },
                    )
                    .await
                }
            }
        })
    }
}

/// A `provider.options.<slug>` passthrough request, per
/// https://openrouter.ai/docs/api/api-reference/stt/create-transcription and
/// https://openrouter.ai/docs/guides/overview/multimodal/stt. Only reachable
/// through OpenRouter's JSON `input_audio` body — the multipart/form-data
/// variant has no `provider` field at all.
struct ProviderOptionsRequest {
    options: serde_json::Map<String, serde_json::Value>,
    /// Speaker labels only appear on segments/words when the response asks
    /// for them, so diarization needs `verbose_json` + word timestamps.
    needs_verbose_json: bool,
}

/// Maps an OpenRouter model id to the provider-specific request needed to get
/// a feature that has no OpenRouter-normalized equivalent. Each entry mirrors
/// a concrete example from OpenRouter's docs rather than guessing at a
/// provider's field names, since provider integrations vary in which fields
/// they forward silently versus reject outright.
fn provider_options_for_model(
    model: &str,
    params: &ListenParams,
) -> Option<ProviderOptionsRequest> {
    match model {
        "openai/gpt-transcribe" if !params.keywords.is_empty() => {
            let mut options = serde_json::Map::new();
            options.insert(
                "openai".to_string(),
                serde_json::json!({ "keywords": params.keywords }),
            );
            Some(ProviderOptionsRequest {
                options,
                needs_verbose_json: false,
            })
        }
        "microsoft/mai-transcribe-2" => {
            let mut options = serde_json::Map::new();
            options.insert(
                "azure".to_string(),
                serde_json::json!({ "diarization": { "enabled": true } }),
            );
            Some(ProviderOptionsRequest {
                options,
                needs_verbose_json: true,
            })
        }
        "x-ai/grok-stt-1.0"
            if params.num_speakers.is_some()
                || params.min_speakers.is_some()
                || params.max_speakers.is_some() =>
        {
            // Mirrors the direct xAI adapter's own gating and field name
            // (crates/owhisper-client/src/adapter/xai/batch.rs), since xAI's
            // API has no public docs to independently confirm the field
            // against; only enable it once we know how many speakers to
            // expect.
            let mut options = serde_json::Map::new();
            options.insert("xai".to_string(), serde_json::json!({ "diarize": true }));
            Some(ProviderOptionsRequest {
                options,
                needs_verbose_json: true,
            })
        }
        // Deepgram always turns diarization on for this codebase's direct
        // integration too (crates/owhisper-client/src/adapter/deepgram/callback.rs),
        // and the docs list `diarize` as one of Deepgram's allowlisted
        // provider.options fields. `deepgram/nova-3` has a single serving
        // endpoint, so there's no ambiguity about which provider gets it.
        "deepgram/nova-3" => {
            let mut options = serde_json::Map::new();
            options.insert(
                "deepgram".to_string(),
                serde_json::json!({ "diarize": true }),
            );
            Some(ProviderOptionsRequest {
                options,
                needs_verbose_json: true,
            })
        }
        // NOT `mistralai/voxtral-mini-transcribe`: unlike Mistral's own direct
        // API, this model as served through OpenRouter rejects
        // `response_format: "verbose_json"` outright (400: "The selected
        // model does not support response_format \"verbose_json\""), and
        // verbose_json is the only way this response schema can carry
        // segment/word speaker data back — so there is no way to retrieve
        // diarization output for it through this endpoint at all.
        _ => None,
    }
}

async fn transcribe_with_provider_options(
    client: &ClientWithMiddleware,
    api_base: &str,
    api_key: &str,
    params: &ListenParams,
    file_path: &Path,
    model: &str,
    request: ProviderOptionsRequest,
) -> Result<Response, Error> {
    let audio = tokio::fs::read(file_path)
        .await
        .map_err(|error| Error::AudioProcessing(error.to_string()))?;
    let format = file_path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("wav");

    let mut body = serde_json::json!({
        "model": model,
        "input_audio": {
            "data": base64::engine::general_purpose::STANDARD.encode(audio),
            "format": format,
        },
        "provider": { "options": request.options },
    });

    if request.needs_verbose_json {
        body["response_format"] = serde_json::Value::String("verbose_json".to_string());
        body["timestamp_granularities"] = serde_json::json!(["segment", "word"]);
    }
    if let Some(language) = params.languages.first() {
        body["language"] = serde_json::Value::String(language.iso639().code().to_string());
    }

    let mut url: url::Url = if api_base.is_empty() {
        DEFAULT_API_BASE
            .parse()
            .expect("invalid_default_openrouter_api_base")
    } else {
        api_base.parse().map_err(|error: url::ParseError| {
            Error::AudioProcessing(format!("invalid api_base: {error}"))
        })?
    };
    append_path_if_missing(&mut url, TRANSCRIPTION_PATH);

    let response = with_attribution_headers(client.post(url.to_string()).bearer_auth(api_key))
        .json(&body)
        .send()
        .await?;

    openai_compatible_batch::parse_response("openrouter", response).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn transcribe_with(
        params: &ListenParams,
        response_json: serde_json::Value,
    ) -> (wiremock::Request, Response) {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/audio/transcriptions"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_json))
            .mount(&server)
            .await;

        let response = OpenRouterAdapter
            .transcribe_file(
                &crate::http_client::create_client(),
                &server.uri(),
                "test-key",
                params,
                anlg_data::english_1::AUDIO_PATH,
            )
            .await
            .unwrap();

        let requests = server.received_requests().await.unwrap();
        (requests.into_iter().next().unwrap(), response)
    }

    fn content_type(request: &wiremock::Request) -> String {
        request
            .headers
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string()
    }

    #[tokio::test]
    async fn multipart_upload_is_used_when_no_json_path_option_is_needed() {
        for model in [
            "openai/gpt-4o-mini-transcribe",
            "openai/gpt-transcribe",
            // Keywords do not reroute models outside the known-capable list.
            "google/chirp-3",
            // xAI only switches to the JSON path on an explicit speaker hint.
            "x-ai/grok-stt-1.0",
            // Regression guard: this model rejects
            // response_format=verbose_json outright (confirmed via a live
            // 400 from OpenRouter), so it must never be routed through the
            // JSON provider.options path, which would need verbose_json to
            // carry any diarization data back anyway.
            "mistralai/voxtral-mini-transcribe",
        ] {
            let params = ListenParams {
                model: Some(model.to_string()),
                languages: vec![anlg_language::ISO639::En.into()],
                keywords: if model == "google/chirp-3" {
                    vec!["Hyprnote".to_string()]
                } else {
                    vec![]
                },
                ..Default::default()
            };

            let (request, response) = transcribe_with(
                &params,
                serde_json::json!({"text": "Hello from OpenRouter."}),
            )
            .await;

            assert!(
                content_type(&request).starts_with("multipart/form-data"),
                "model: {model}"
            );
            let body = String::from_utf8_lossy(&request.body);
            assert!(body.contains(model), "model: {model}");
            assert!(body.contains("name=\"language\""), "model: {model}");
            assert!(!body.contains("response_format"), "model: {model}");
            assert_eq!(
                response.results.channels[0].alternatives[0].transcript,
                "Hello from OpenRouter."
            );
        }
    }

    #[tokio::test]
    async fn forwards_dictionary_keywords_as_openai_provider_options_for_gpt_transcribe() {
        let params = ListenParams {
            model: Some("openai/gpt-transcribe".to_string()),
            languages: vec![anlg_language::ISO639::En.into()],
            keywords: vec!["Hyprnote".to_string(), "Anarlog".to_string()],
            ..Default::default()
        };

        let (request, _) = transcribe_with(
            &params,
            serde_json::json!({"text": "Hello from OpenRouter."}),
        )
        .await;

        assert!(content_type(&request).starts_with("application/json"));

        let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
        assert_eq!(body["model"], "openai/gpt-transcribe");
        assert_eq!(body["language"], "en");
        assert!(!body["input_audio"]["data"].as_str().unwrap().is_empty());
        assert_eq!(body["input_audio"]["format"], "wav");
        assert!(body.get("response_format").is_none());
        assert_eq!(
            body["provider"]["options"]["openai"]["keywords"],
            serde_json::json!(["Hyprnote", "Anarlog"])
        );
    }

    #[tokio::test]
    async fn diarization_routes_through_json_provider_options() {
        for (model, params_extra, option_path) in [
            // Azure's real diarization is phrase/segment-scoped, not
            // per-word (see adapter::azure_speech), so the flat top-level
            // `words` array carries no speaker of its own here — this is
            // the shape that actually comes back in production, as
            // opposed to OpenRouter's docs' own admittedly "(abridged)"
            // example, which duplicates speaker onto both.
            (
                "microsoft/mai-transcribe-2",
                ListenParams::default(),
                &["azure", "diarization", "enabled"][..],
            ),
            (
                "x-ai/grok-stt-1.0",
                ListenParams {
                    num_speakers: Some(2),
                    ..Default::default()
                },
                &["xai", "diarize"][..],
            ),
            (
                "deepgram/nova-3",
                ListenParams::default(),
                &["deepgram", "diarize"][..],
            ),
        ] {
            let params = ListenParams {
                model: Some(model.to_string()),
                languages: vec![anlg_language::ISO639::En.into()],
                ..params_extra
            };

            let response_json = if model == "microsoft/mai-transcribe-2" {
                serde_json::json!({
                    "text": "Hello there. Hi, how are you?",
                    "segments": [
                        { "id": 0, "start": 0.0, "end": 1.2, "text": "Hello there.", "speaker": 0 },
                        { "id": 1, "start": 1.5, "end": 3.1, "text": "Hi, how are you?", "speaker": 1 }
                    ],
                    "words": [
                        { "word": "Hello", "start": 0.0, "end": 0.4 },
                        { "word": "there.", "start": 0.4, "end": 1.2 }
                    ]
                })
            } else {
                serde_json::json!({
                    "text": "Hello there. Hi, how are you?",
                    "words": [
                        { "word": "Hello", "start": 0.0, "end": 0.4, "speaker": 0 },
                        { "word": "there.", "start": 0.4, "end": 1.2, "speaker": 1 }
                    ]
                })
            };

            let (request, response) = transcribe_with(&params, response_json).await;

            assert!(
                content_type(&request).starts_with("application/json"),
                "model: {model}"
            );

            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
            assert_eq!(body["model"], model);
            assert_eq!(body["response_format"], "verbose_json");
            if model != "deepgram/nova-3" {
                assert_eq!(
                    body["timestamp_granularities"],
                    serde_json::json!(["segment", "word"]),
                    "model: {model}"
                );
            }
            let mut option = &body["provider"]["options"];
            for key in option_path {
                option = &option[key];
            }
            assert_eq!(*option, true, "model: {model}");

            let words = &response.results.channels[0].alternatives[0].words;
            assert_eq!(words[0].speaker, Some(0));
            // Must land on the mixed-capture channel, not channel 0
            // (DirectMic) — the render pipeline treats DirectMic as always
            // exactly one speaker and collapses every diarized label back
            // into one there.
            assert_eq!(words[0].channel, crate::adapter::MIXED_CAPTURE_CHANNEL);
            if model != "microsoft/mai-transcribe-2" {
                assert_eq!(words[1].speaker, Some(1));
            } else {
                assert_eq!(words[1].speaker, Some(0));
            }
        }
    }
}
