use std::{collections::HashSet, sync::OnceLock, time::Duration};

use futures::stream::{self, StreamExt};
use serde_json::{Value, json};

use crate::meeting::config::ProviderKind;

use super::provider_catalog;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

pub const GOOGLE_VERTEX_AI_MODELS: &[&str] = &[
    "google/gemini-3.8-flash",
    "google/gemini-3.7-flash",
    "google/gemini-3.6-flash",
    "google/gemini-3.5-flash-lite",
    "google/gemini-3.1-pro-preview",
    "google/gemini-3.5-flash",
    "google/gemini-3.1-flash-lite",
];

pub const CLOUDFLARE_WORKERS_AI_MODELS: &[&str] = &[
    "@cf/moonshotai/kimi-k2.7-code",
    "@cf/zai-org/glm-5.2",
    "@cf/moonshotai/kimi-k2.6",
    "@cf/zai-org/glm-4.7-flash",
    "@cf/openai/gpt-oss-120b",
    "@cf/meta/llama-4-scout-17b-16e-instruct",
    "@cf/google/gemma-4-26b-a4b-it",
    "@cf/nvidia/nemotron-3-120b-a12b",
    "@cf/openai/gpt-oss-20b",
    "@cf/qwen/qwen3-30b-a3b-fp8",
    "@cf/mistralai/mistral-small-3.1-24b-instruct",
    "@cf/meta/llama-3.3-70b-instruct-fp8-fast",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelEntry {
    pub id: String,
    pub label: String,
}

pub fn supports_live_models(kind: ProviderKind, provider: &str) -> bool {
    match kind {
        ProviderKind::Llm => matches!(
            provider,
            "anarlog"
                | "openai"
                | "anthropic"
                | "google_generative_ai"
                | "openrouter"
                | "venice"
                | "moonshot"
                | "zai"
                | "deepseek"
                | "alibaba_cloud"
                | "siliconflow"
                | "google_vertex_ai"
                | "azure_ai"
                | "groq"
                | "ollama"
                | "xai"
                | "mistral"
                | "meta"
                | "together"
                | "cohere"
                | "fireworks"
                | "cloudflare_workers_ai"
                | "cerebras"
                | "lmstudio"
                | "unsloth"
        ),
        ProviderKind::Stt => matches!(
            provider,
            "deepgram"
                | "soniox"
                | "elevenlabs"
                | "openai"
                | "groq"
                | "mistral"
                | "fireworks"
                | "together"
                | "siliconflow"
                | "dashscope"
                | "google_generative_ai"
                | "cloudflare_workers_ai"
        ),
    }
}

pub fn fallback_models(kind: ProviderKind, provider: &str) -> Vec<ModelEntry> {
    match (kind, provider) {
        (ProviderKind::Llm, "anarlog") => provider_catalog::ANARLOG_LLM_MODELS
            .iter()
            .map(|(id, label)| ModelEntry {
                id: (*id).to_owned(),
                label: (*label).to_owned(),
            })
            .collect(),
        (ProviderKind::Llm, "google_vertex_ai") => GOOGLE_VERTEX_AI_MODELS
            .iter()
            .map(|id| ModelEntry {
                id: (*id).to_owned(),
                label: (*id).to_owned(),
            })
            .collect(),
        (ProviderKind::Llm, "cloudflare_workers_ai") => CLOUDFLARE_WORKERS_AI_MODELS
            .iter()
            .map(|id| ModelEntry {
                id: (*id).to_owned(),
                label: (*id).to_owned(),
            })
            .collect(),
        (ProviderKind::Stt, _) => provider_catalog::stt_models(provider)
            .iter()
            .map(|id| ModelEntry {
                id: (*id).to_owned(),
                label: provider_catalog::stt_model_label(id).to_owned(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub async fn fetch_models(
    kind: ProviderKind,
    provider: &str,
    base_url: &str,
    api_key: Option<&str>,
    cloud_token: Option<&str>,
) -> Vec<ModelEntry> {
    if !supports_live_models(kind, provider) {
        return Vec::new();
    }

    let trimmed_base = base_url.trim().trim_end_matches('/');
    let trimmed_key = api_key.map(str::trim).filter(|value| !value.is_empty());

    match kind {
        ProviderKind::Llm => match provider {
            "anarlog" => {
                if let (false, Some(token)) = (
                    trimmed_base.is_empty(),
                    cloud_token.map(str::trim).filter(|t| !t.is_empty()),
                ) {
                    let url = format!("{trimmed_base}/llm/models");
                    if let Some(body) =
                        get_json(&url, &[("authorization", &format!("Bearer {token}"))]).await
                    {
                        let live = parse_anarlog_models(&body);
                        if !live.is_empty() {
                            return live;
                        }
                    }
                }
                fallback_models(kind, provider)
            }
            "google_vertex_ai" => {
                if let (false, Some(key)) = (trimmed_base.is_empty(), trimmed_key) {
                    let normalized = normalize_vertex_base(trimmed_base);
                    let headers = [("authorization", format!("Bearer {key}"))];
                    let mut urls = vec![format!("{normalized}/models")];
                    if let Ok(parsed) = reqwest::Url::parse(trimmed_base) {
                        let origin_url = format!(
                            "{}/v1beta1/publishers/google/models",
                            parsed.origin().ascii_serialization()
                        );
                        if !urls.contains(&origin_url) {
                            urls.push(origin_url);
                        }
                    }
                    let fallback_url = format!("{normalized}/publishers/google/models");
                    if !urls.contains(&fallback_url) {
                        urls.push(fallback_url);
                    }
                    for url in urls {
                        if let Some(body) = get_json_owned(&url, &headers).await {
                            let live = parse_vertex_models(&body);
                            if !live.is_empty() {
                                return live;
                            }
                        }
                    }
                }
                fallback_models(kind, provider)
            }
            "cloudflare_workers_ai" => {
                if let (false, Some(key)) = (trimmed_base.is_empty(), trimmed_key) {
                    let headers = [("authorization", format!("Bearer {key}"))];
                    let openai_url = format!("{trimmed_base}/models");
                    if let Some(body) = get_json_owned(&openai_url, &headers).await {
                        let live = parse_cloudflare_llm_models(&body);
                        if !live.is_empty() {
                            return live;
                        }
                    }
                    let search_base = strip_suffix_ci(trimmed_base, "/v1");
                    let search_url = format!("{search_base}/models/search?task=Text%20Generation");
                    if let Some(body) = get_json_owned(&search_url, &headers).await {
                        let live = parse_cloudflare_llm_models(&body);
                        if !live.is_empty() {
                            return live;
                        }
                    }
                }
                fallback_models(kind, provider)
            }
            "anthropic" => {
                let (false, Some(key)) = (trimmed_base.is_empty(), trimmed_key) else {
                    return Vec::new();
                };
                let url = format!("{trimmed_base}/models");
                let Some(body) = get_json(
                    &url,
                    &[("x-api-key", key), ("anthropic-version", "2023-06-01")],
                )
                .await
                else {
                    return Vec::new();
                };
                parse_openai_like_models(&body, None)
            }
            "google_generative_ai" => {
                let (false, Some(key)) = (trimmed_base.is_empty(), trimmed_key) else {
                    return Vec::new();
                };
                let url = format!("{trimmed_base}/models");
                let Some(body) = get_json(&url, &[("x-goog-api-key", key)]).await else {
                    return Vec::new();
                };
                parse_google_gemini_llm_models(&body)
            }
            "azure_ai" => {
                let (false, Some(key)) = (trimmed_base.is_empty(), trimmed_key) else {
                    return Vec::new();
                };
                let url = format!("{trimmed_base}/models");
                let Some(body) = get_json(&url, &[("api-key", key)]).await else {
                    return Vec::new();
                };
                parse_openai_like_models(&body, None)
            }
            "venice" => {
                let (false, Some(key)) = (trimmed_base.is_empty(), trimmed_key) else {
                    return Vec::new();
                };
                let url = format!("{trimmed_base}/models?type=text");
                let auth = format!("Bearer {key}");
                let Some(body) = get_json(&url, &[("authorization", &auth)]).await else {
                    return Vec::new();
                };
                parse_openai_like_models(&body, None)
            }
            "lmstudio" => {
                if trimmed_base.is_empty() {
                    return Vec::new();
                }
                let mut headers = Vec::new();
                let auth = trimmed_key.map(|key| format!("Bearer {key}"));
                if let Some(auth) = auth.as_deref() {
                    headers.push(("authorization", auth));
                }
                let native_url = lmstudio_native_models_url(trimmed_base);
                let compat_url = format!("{trimmed_base}/models");
                let (native_body, compat_body) = tokio::join!(
                    get_json(&native_url, &headers),
                    get_json(&compat_url, &headers)
                );
                parse_lmstudio_models(native_body.as_ref(), compat_body.as_ref())
            }
            "ollama" => {
                if trimmed_base.is_empty() {
                    return Vec::new();
                }
                let mut headers = Vec::new();
                let auth = trimmed_key.map(|key| format!("Bearer {key}"));
                if let Some(auth) = auth.as_deref() {
                    headers.push(("authorization", auth));
                }
                fetch_ollama_models_within(REQUEST_TIMEOUT, trimmed_base, &headers).await
            }
            _ => {
                let (false, Some(key)) = (trimmed_base.is_empty(), trimmed_key) else {
                    return Vec::new();
                };
                let url = format!("{trimmed_base}/models");
                let auth = format!("Bearer {key}");
                let Some(body) = get_json(&url, &[("authorization", &auth)]).await else {
                    return Vec::new();
                };
                let filter = match provider {
                    "openai" => Some(is_openai_chat_item as fn(&str, &Value) -> bool),
                    "mistral" => Some(is_mistral_chat_item as fn(&str, &Value) -> bool),
                    _ => None,
                };
                parse_openai_like_models(&body, filter)
            }
        },
        ProviderKind::Stt => fetch_stt_models(provider, trimmed_base, trimmed_key).await,
    }
}

async fn fetch_stt_models(
    provider: &str,
    trimmed_base: &str,
    trimmed_key: Option<&str>,
) -> Vec<ModelEntry> {
    let (false, Some(key)) = (trimmed_base.is_empty(), trimmed_key) else {
        return Vec::new();
    };
    let origin = match reqwest::Url::parse(trimmed_base) {
        Ok(url) => url.origin().ascii_serialization(),
        Err(_) => return Vec::new(),
    };
    let bearer = format!("Bearer {key}");
    let deepgram_token = format!("Token {key}");

    type SttParser = fn(&Value) -> Vec<String>;
    type SttRequestSpec<'a> = (String, Vec<(&'static str, &'a str)>, SttParser);

    let (url, headers, parser): SttRequestSpec<'_> = match provider {
        "deepgram" => {
            let url = if trimmed_base.ends_with("/v1") {
                format!("{trimmed_base}/models")
            } else {
                format!("{trimmed_base}/v1/models")
            };
            (
                url,
                vec![("authorization", &deepgram_token)],
                parse_deepgram_stt_ids,
            )
        }
        "soniox" => (
            format!("{}/v1/models", strip_suffix_ci(trimmed_base, "/v1")),
            vec![("authorization", &bearer)],
            parse_soniox_stt_ids,
        ),
        "elevenlabs" => (
            format!("{}/v1/models", strip_suffix_ci(trimmed_base, "/v1")),
            vec![("xi-api-key", key)],
            parse_elevenlabs_stt_ids,
        ),
        "openai" => (
            format!("{trimmed_base}/models"),
            vec![("authorization", &bearer)],
            parse_openai_stt_ids,
        ),
        "groq" => (
            format!("{trimmed_base}/models"),
            vec![("authorization", &bearer)],
            parse_groq_stt_ids,
        ),
        "mistral" => (
            format!("{trimmed_base}/models"),
            vec![("authorization", &bearer)],
            parse_mistral_stt_ids,
        ),
        "fireworks" => (
            format!("{origin}/inference/v1/models"),
            vec![("authorization", &bearer)],
            parse_fireworks_stt_ids,
        ),
        "together" => (
            format!("{trimmed_base}/models"),
            vec![("authorization", &bearer)],
            parse_together_stt_ids,
        ),
        "siliconflow" => (
            format!("{trimmed_base}/models?sub_type=audio"),
            vec![("authorization", &bearer)],
            parse_all_openai_ids,
        ),
        "dashscope" => (
            format!(
                "{}/compatible-mode/v1/models",
                strip_suffix_ci(trimmed_base, "/compatible-mode/v1")
            ),
            vec![("authorization", &bearer)],
            parse_dashscope_stt_ids,
        ),
        "google_generative_ai" => (
            format!("{trimmed_base}/models"),
            vec![("x-goog-api-key", key)],
            parse_google_stt_ids,
        ),
        "cloudflare_workers_ai" => (
            format!(
                "{}/models/search?task=Automatic%20Speech%20Recognition",
                strip_suffix_ci(trimmed_base, "/v1")
            ),
            vec![("authorization", &bearer)],
            parse_cloudflare_stt_ids,
        ),
        _ => return Vec::new(),
    };

    let Some(body) = get_json(&url, &headers).await else {
        return Vec::new();
    };
    dedupe_ids(parser(&body))
        .into_iter()
        .map(|id| ModelEntry {
            label: provider_catalog::stt_model_label(&id).to_owned(),
            id,
        })
        .collect()
}

fn http_client() -> Option<&'static reqwest::Client> {
    static CLIENT: OnceLock<Option<reqwest::Client>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .ok()
        })
        .as_ref()
}

fn validate_url(raw: &str) -> Option<reqwest::Url> {
    let url = reqwest::Url::parse(raw).ok()?;
    if !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    let allowed = url.scheme() == "https"
        || (url.scheme() == "http"
            && matches!(
                url.host_str(),
                Some("localhost" | "127.0.0.1" | "::1" | "[::1]")
            ));
    allowed.then_some(url)
}

async fn get_json_owned(url: &str, headers: &[(&'static str, String)]) -> Option<Value> {
    let borrowed: Vec<(&'static str, &str)> =
        headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
    get_json(url, &borrowed).await
}

async fn get_json(url: &str, headers: &[(&'static str, &str)]) -> Option<Value> {
    let parsed = validate_url(url)?;
    let client = http_client()?;
    let mut request = client.get(parsed);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    read_limited_json(request.send().await.ok()?).await
}

async fn post_json(url: &str, headers: &[(&'static str, &str)], body: &Value) -> Option<Value> {
    let parsed = validate_url(url)?;
    let client = http_client()?;
    let mut request = client.post(parsed).json(body);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    read_limited_json(request.send().await.ok()?).await
}

async fn read_limited_json(mut response: reqwest::Response) -> Option<Value> {
    if !response.status().is_success() {
        return None;
    }
    if response
        .content_length()
        .is_some_and(|len| len > MAX_RESPONSE_BYTES as u64)
    {
        return None;
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).ok()
}

fn lmstudio_native_models_url(trimmed_base: &str) -> String {
    let base = trimmed_base.trim_end_matches('/');
    if base.to_ascii_lowercase().ends_with("/api/v1") {
        format!("{base}/models")
    } else {
        format!("{}/api/v1/models", strip_suffix_ci(base, "/v1"))
    }
}

fn parse_lmstudio_models(
    native_body: Option<&Value>,
    compat_body: Option<&Value>,
) -> Vec<ModelEntry> {
    let mut embedding_ids = HashSet::new();
    let mut native_entries = Vec::new();
    let mut has_native_catalog = false;

    if let Some(items) = native_body
        .and_then(|body| body.get("models"))
        .and_then(Value::as_array)
    {
        has_native_catalog = true;
        for item in items {
            let key = item
                .get("key")
                .or_else(|| item.get("id"))
                .or_else(|| item.get("name"))
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            let loaded_ids: Vec<&str> = item
                .get("loaded_instances")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|instance| instance.get("id").and_then(Value::as_str))
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .collect();

            if !is_lmstudio_chat_item(key, item) {
                if !key.is_empty() {
                    embedding_ids.insert(key.to_owned());
                }
                embedding_ids.extend(loaded_ids.into_iter().map(str::to_owned));
                continue;
            }

            let display = item
                .get("display_name")
                .or_else(|| item.get("name"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|label| !label.is_empty());
            if !loaded_ids.is_empty() {
                for id in loaded_ids {
                    let label = if id == key { display.unwrap_or(id) } else { id };
                    native_entries.push(ModelEntry {
                        id: id.to_owned(),
                        label: label.to_owned(),
                    });
                }
            } else if !key.is_empty() {
                native_entries.push(ModelEntry {
                    id: key.to_owned(),
                    label: display.unwrap_or(key).to_owned(),
                });
            }
        }
        native_entries = dedupe_entries(native_entries);
    }

    if let Some(body) = compat_body {
        let compat_entries: Vec<ModelEntry> =
            parse_openai_like_models(body, Some(is_lmstudio_chat_item))
                .into_iter()
                .filter(|entry| !embedding_ids.contains(&entry.id))
                .collect();
        if !compat_entries.is_empty() {
            return compat_entries;
        }
    }

    if has_native_catalog {
        native_entries
    } else {
        Vec::new()
    }
}

fn is_lmstudio_chat_item(_id: &str, item: &Value) -> bool {
    let Some(model_type) = item.get("type").and_then(Value::as_str) else {
        return true;
    };
    let lower = model_type.trim().to_ascii_lowercase();
    lower.is_empty() || matches!(lower.as_str(), "llm" | "vlm" | "chat" | "text")
}

async fn fetch_ollama_models_within(
    deadline: Duration,
    trimmed_base: &str,
    headers: &[(&'static str, &str)],
) -> Vec<ModelEntry> {
    tokio::time::timeout(deadline, async {
        let url = format!("{trimmed_base}/models");
        let mut entries = if let Some(body) = get_json(&url, headers).await {
            parse_openai_like_models(&body, None)
        } else {
            Vec::new()
        };
        if entries.is_empty() {
            let tags_base = strip_suffix_ci(trimmed_base, "/v1");
            let tags_url = format!("{tags_base}/api/tags");
            if let Some(body) = get_json(&tags_url, headers).await {
                entries = parse_ollama_tags(&body);
            }
        }
        if entries.is_empty() {
            return Vec::new();
        }
        let show_base = strip_suffix_ci(trimmed_base, "/v1");
        let show_url = format!("{show_base}/api/show");
        stream::iter(entries)
            .map(|entry| {
                let show_url = &show_url;
                async move {
                    let body = json!({ "model": entry.id });
                    let keep = match post_json(show_url, headers, &body).await {
                        Some(info) => info
                            .get("capabilities")
                            .and_then(Value::as_array)
                            .is_none_or(|caps| {
                                caps.iter().any(|cap| cap.as_str() == Some("completion"))
                            }),
                        None => true,
                    };
                    keep.then_some(entry)
                }
            })
            .buffered(4)
            .filter_map(|entry| async move { entry })
            .collect()
            .await
    })
    .await
    .unwrap_or_default()
}

fn strip_suffix_ci<'a>(value: &'a str, suffix: &str) -> &'a str {
    let lower = value.to_ascii_lowercase();
    if lower.ends_with(&suffix.to_ascii_lowercase()) {
        &value[..value.len() - suffix.len()]
    } else {
        value
    }
}

fn normalize_vertex_base(base_url: &str) -> String {
    strip_suffix_ci(strip_suffix_ci(base_url, "/chat/completions"), "/endpoints")
        .trim_end_matches('/')
        .to_owned()
}

fn dedupe_entries(entries: Vec<ModelEntry>) -> Vec<ModelEntry> {
    let mut seen = HashSet::new();
    entries
        .into_iter()
        .filter(|entry| !entry.id.is_empty() && seen.insert(entry.id.clone()))
        .collect()
}

fn dedupe_ids(ids: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    ids.into_iter()
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty() && seen.insert(id.clone()))
        .collect()
}

fn parse_anarlog_models(body: &Value) -> Vec<ModelEntry> {
    let Some(items) = body
        .get("data")
        .or_else(|| body.get("models"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let entries = items
        .iter()
        .filter_map(|item| {
            let id = item.get("id").and_then(Value::as_str)?.trim();
            if id.is_empty() {
                return None;
            }
            let label = item
                .get("label")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|label| !label.is_empty())
                .unwrap_or(id);
            Some(ModelEntry {
                id: id.to_owned(),
                label: label.to_owned(),
            })
        })
        .collect();
    dedupe_entries(entries)
}

fn is_vertex_non_chat_id(id: &str) -> bool {
    let lower = id.to_ascii_lowercase();
    [
        "embed",
        "veo",
        "imagen",
        "lyria",
        "sora",
        "tts",
        "whisper",
        "dall-e",
        "audio",
        "image",
        "imagine",
        "realtime",
        "transcribe",
        "moderation",
        "rerank",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn parse_vertex_models(body: &Value) -> Vec<ModelEntry> {
    let items = body
        .get("models")
        .or_else(|| body.get("publisherModels"))
        .and_then(Value::as_array);
    let Some(items) = items else {
        return Vec::new();
    };
    let entries = items
        .iter()
        .filter_map(|item| {
            let raw = item
                .get("name")
                .or_else(|| item.get("id"))
                .and_then(Value::as_str)?
                .trim();
            if raw.is_empty() {
                return None;
            }
            let model_name = if let Some(idx) = raw.find("/models/") {
                &raw[idx + "/models/".len()..]
            } else {
                raw
            };
            if is_vertex_non_chat_id(model_name) {
                return None;
            }
            let id = if model_name.starts_with("google/") {
                model_name.to_owned()
            } else {
                format!("google/{model_name}")
            };
            let label = item
                .get("displayName")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|label| !label.is_empty())
                .unwrap_or(&id)
                .to_owned();
            Some(ModelEntry { id, label })
        })
        .collect();
    dedupe_entries(entries)
}

fn parse_cloudflare_llm_models(body: &Value) -> Vec<ModelEntry> {
    if let Some(items) = body.get("data").and_then(Value::as_array) {
        let entries = items
            .iter()
            .filter_map(|item| {
                let id = item.get("id").and_then(Value::as_str)?.trim();
                (!id.is_empty() && !is_non_chat_cf_id(id)).then(|| ModelEntry {
                    id: id.to_owned(),
                    label: id.to_owned(),
                })
            })
            .collect();
        return dedupe_entries(entries);
    }
    if let Some(items) = body.get("result").and_then(Value::as_array) {
        let entries = items
            .iter()
            .filter_map(|item| {
                let task = item
                    .get("task")
                    .and_then(|task| task.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if !task.is_empty() && !task.contains("text generation") {
                    return None;
                }
                let id = item
                    .get("name")
                    .or_else(|| item.get("id"))
                    .and_then(Value::as_str)?
                    .trim();
                (!id.is_empty() && !is_non_chat_cf_id(id)).then(|| ModelEntry {
                    id: id.to_owned(),
                    label: id.to_owned(),
                })
            })
            .collect();
        return dedupe_entries(entries);
    }
    Vec::new()
}

fn is_non_chat_cf_id(id: &str) -> bool {
    let lower = id.to_ascii_lowercase();
    [
        "embed",
        "bge-",
        "rerank",
        "whisper",
        "tts",
        "flux",
        "stable-diffusion",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn parse_openai_like_models(
    body: &Value,
    filter: Option<fn(&str, &Value) -> bool>,
) -> Vec<ModelEntry> {
    let items = body
        .get("data")
        .or_else(|| body.get("models"))
        .and_then(Value::as_array)
        .or_else(|| body.as_array());
    let Some(items) = items else {
        return Vec::new();
    };
    let entries = items
        .iter()
        .filter_map(|item| {
            let id = item
                .get("id")
                .or_else(|| item.get("key"))
                .or_else(|| item.get("name"))
                .and_then(Value::as_str)?
                .trim();
            if id.is_empty() || filter.is_some_and(|pred| !pred(id, item)) {
                return None;
            }
            let label = item
                .get("display_name")
                .or_else(|| item.get("name"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|label| !label.is_empty())
                .unwrap_or(id);
            Some(ModelEntry {
                id: id.to_owned(),
                label: label.to_owned(),
            })
        })
        .collect();
    dedupe_entries(entries)
}

fn is_openai_chat_item(id: &str, _item: &Value) -> bool {
    let lower = id.to_ascii_lowercase();
    ![
        "whisper",
        "tts-",
        "dall-e",
        "gpt-image",
        "embedding",
        "moderation",
        "babbage",
        "davinci",
        "transcribe",
        "realtime",
        "audio-preview",
        "sora",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_mistral_chat_item(id: &str, item: &Value) -> bool {
    if let Some(flag) = item
        .get("capabilities")
        .and_then(|caps| caps.get("completion_chat"))
        .and_then(Value::as_bool)
    {
        return flag;
    }
    !is_vertex_non_chat_id(id)
}

fn parse_google_gemini_llm_models(body: &Value) -> Vec<ModelEntry> {
    let Some(items) = body.get("models").and_then(Value::as_array) else {
        return Vec::new();
    };
    let entries = items
        .iter()
        .filter_map(|item| {
            let name = item.get("name").and_then(Value::as_str)?.trim();
            let id = name.strip_prefix("models/").unwrap_or(name);
            let lower = id.to_ascii_lowercase();
            if !lower.starts_with("gemini") && !lower.starts_with("gemma") {
                return None;
            }
            if ["embedding", "imagen", "veo", "tts", "aqa", "bisociative"]
                .iter()
                .any(|needle| lower.contains(needle))
            {
                return None;
            }
            if let Some(methods) = item
                .get("supportedGenerationMethods")
                .and_then(Value::as_array)
            {
                let supports_generate = methods
                    .iter()
                    .any(|m| m.as_str() == Some("generateContent"));
                if !supports_generate {
                    return None;
                }
            }
            let label = item
                .get("displayName")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|label| !label.is_empty())
                .unwrap_or(id);
            Some(ModelEntry {
                id: id.to_owned(),
                label: label.to_owned(),
            })
        })
        .collect();
    dedupe_entries(entries)
}

fn parse_ollama_tags(body: &Value) -> Vec<ModelEntry> {
    let Some(items) = body.get("models").and_then(Value::as_array) else {
        return Vec::new();
    };
    let entries = items
        .iter()
        .filter_map(|item| {
            let name = item.get("name").and_then(Value::as_str)?.trim();
            (!name.is_empty()).then(|| ModelEntry {
                id: name.to_owned(),
                label: name.to_owned(),
            })
        })
        .collect();
    dedupe_entries(entries)
}

fn parse_deepgram_stt_ids(body: &Value) -> Vec<String> {
    body.get("stt")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("canonical_name")
                        .and_then(Value::as_str)
                        .filter(|s| !s.trim().is_empty())
                        .or_else(|| item.get("name").and_then(Value::as_str))
                        .map(str::to_owned)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_soniox_stt_ids(body: &Value) -> Vec<String> {
    body.get("models")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let id = item.get("id").and_then(Value::as_str)?.trim();
                    (!id.is_empty() && !id.to_ascii_lowercase().starts_with("stt-async-"))
                        .then(|| id.to_owned())
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_elevenlabs_stt_ids(body: &Value) -> Vec<String> {
    let items = body
        .as_array()
        .or_else(|| body.get("models").and_then(Value::as_array));
    let Some(items) = items else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let id = item.get("model_id").and_then(Value::as_str)?.trim();
            if id.is_empty() {
                return None;
            }
            let stt_capable = item.get("can_do_speech_to_text").and_then(Value::as_bool)
                == Some(true)
                || id.to_ascii_lowercase().contains("scribe");
            stt_capable.then(|| id.to_owned())
        })
        .collect()
}

fn parse_filtered_openai_ids(body: &Value, needles: &[&str]) -> Vec<String> {
    let items = body
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| body.as_array());
    let Some(items) = items else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let id = item.get("id").and_then(Value::as_str)?.trim();
            let lower = id.to_ascii_lowercase();
            (!id.is_empty() && needles.iter().any(|n| lower.contains(n))).then(|| id.to_owned())
        })
        .collect()
}

fn parse_openai_stt_ids(body: &Value) -> Vec<String> {
    parse_filtered_openai_ids(body, &["whisper", "transcribe"])
}

fn parse_groq_stt_ids(body: &Value) -> Vec<String> {
    parse_filtered_openai_ids(body, &["whisper", "asr", "transcribe"])
}

fn is_tts_id(lower_id: &str) -> bool {
    lower_id
        .split(['-', '_', '/'])
        .any(|segment| segment == "tts")
        || lower_id.contains("text-to-speech")
}

fn is_mistral_stt_item(item: &Value, lower_id: &str) -> bool {
    if let Some(caps) = item.get("audio_transcription_capabilities") {
        return match caps {
            Value::Null | Value::Bool(false) => false,
            Value::Array(arr) => !arr.is_empty(),
            _ => true,
        };
    }
    if let Some(flag) = item
        .get("capabilities")
        .and_then(|c| c.get("audio_transcription"))
        .and_then(Value::as_bool)
    {
        return flag;
    }
    !is_tts_id(lower_id)
        && (lower_id.contains("voxtral")
            || lower_id.contains("transcribe")
            || lower_id.contains("asr"))
}

fn parse_mistral_stt_ids(body: &Value) -> Vec<String> {
    let items = body
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| body.as_array());
    let Some(items) = items else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let id = item.get("id").and_then(Value::as_str)?.trim();
            let lower = id.to_ascii_lowercase();
            (!id.is_empty() && is_mistral_stt_item(item, &lower)).then(|| id.to_owned())
        })
        .collect()
}

fn parse_fireworks_stt_ids(body: &Value) -> Vec<String> {
    parse_filtered_openai_ids(body, &["whisper", "asr", "transcribe", "speech", "audio"])
}

fn parse_together_stt_ids(body: &Value) -> Vec<String> {
    let items = body
        .as_array()
        .or_else(|| body.get("data").and_then(Value::as_array));
    let Some(items) = items else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let id = item.get("id").and_then(Value::as_str)?.trim();
            if id.is_empty() {
                return None;
            }
            let model_type = item
                .get("type")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|t| !t.is_empty());
            if model_type.is_some_and(|t| t.eq_ignore_ascii_case("transcribe")) {
                return Some(id.to_owned());
            }
            if model_type.is_some_and(|t| !t.eq_ignore_ascii_case("audio")) {
                return None;
            }
            let lower = id.to_ascii_lowercase();
            (!is_tts_id(&lower)
                && ["whisper", "asr", "transcribe"]
                    .iter()
                    .any(|needle| lower.contains(needle)))
            .then(|| id.to_owned())
        })
        .collect()
}

fn parse_all_openai_ids(body: &Value) -> Vec<String> {
    body.get("data")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("id").and_then(Value::as_str).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_dashscope_stt_ids(body: &Value) -> Vec<String> {
    parse_filtered_openai_ids(
        body,
        &[
            "asr",
            "transcribe",
            "sensevoice",
            "paraformer",
            "whisper",
            "audio",
        ],
    )
}

fn parse_google_stt_ids(body: &Value) -> Vec<String> {
    body.get("models")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let raw = item.get("name").and_then(Value::as_str)?.trim();
                    let id = raw.strip_prefix("models/").unwrap_or(raw);
                    let lower = id.to_ascii_lowercase();
                    (lower.contains("gemini")
                        && (lower.contains("transcribe") || lower.contains("live")))
                    .then(|| id.to_owned())
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_cloudflare_stt_ids(body: &Value) -> Vec<String> {
    body.get("result")
        .or_else(|| body.get("data"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let id = item
                        .get("name")
                        .and_then(Value::as_str)
                        .filter(|s| !s.trim().is_empty())
                        .or_else(|| item.get("id").and_then(Value::as_str))?
                        .trim();
                    matches!(
                        id.to_ascii_lowercase().as_str(),
                        "@cf/deepgram/nova-3" | "@cf/deepgram/flux"
                    )
                    .then(|| id.to_owned())
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path},
    };

    #[tokio::test]
    async fn anarlog_llm_fetches_live_models_and_falls_back_on_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/llm/models"))
            .and(header("authorization", "Bearer tok-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": "list",
                "data": [
                    { "id": "Auto", "label": "Pro (Cloud)" },
                    { "id": "~openai/gpt-live-new", "label": "GPT Live New" }
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let live = fetch_models(
            ProviderKind::Llm,
            "anarlog",
            &server.uri(),
            None,
            Some("tok-1"),
        )
        .await;
        assert_eq!(
            live,
            vec![
                ModelEntry {
                    id: "Auto".into(),
                    label: "Pro (Cloud)".into()
                },
                ModelEntry {
                    id: "~openai/gpt-live-new".into(),
                    label: "GPT Live New".into()
                }
            ]
        );

        let fallback = fetch_models(
            ProviderKind::Llm,
            "anarlog",
            "http://127.0.0.1:9",
            None,
            Some("tok-1"),
        )
        .await;
        assert_eq!(fallback, fallback_models(ProviderKind::Llm, "anarlog"));
    }

    #[tokio::test]
    async fn openai_and_anthropic_llm_require_key_and_parse_models() {
        assert!(
            fetch_models(
                ProviderKind::Llm,
                "openai",
                "http://127.0.0.1:9/v1",
                None,
                None,
            )
            .await
            .is_empty()
        );

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .and(header("authorization", "Bearer oa-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    { "id": "gpt-5" },
                    { "id": "whisper-1" }
                ]
            })))
            .mount(&server)
            .await;

        let openai = fetch_models(
            ProviderKind::Llm,
            "openai",
            &format!("{}/v1", server.uri()),
            Some("oa-key"),
            None,
        )
        .await;
        assert_eq!(
            openai,
            vec![ModelEntry {
                id: "gpt-5".into(),
                label: "gpt-5".into(),
            }]
        );

        let anth_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .and(header("x-api-key", "sk-ant"))
            .and(header("anthropic-version", "2023-06-01"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    { "id": "claude-sonnet-4-6", "display_name": "Claude Sonnet 4.6" }
                ]
            })))
            .mount(&anth_server)
            .await;

        let anthropic = fetch_models(
            ProviderKind::Llm,
            "anthropic",
            &format!("{}/v1", anth_server.uri()),
            Some("sk-ant"),
            None,
        )
        .await;
        assert_eq!(
            anthropic,
            vec![ModelEntry {
                id: "claude-sonnet-4-6".into(),
                label: "Claude Sonnet 4.6".into(),
            }]
        );
    }

    #[tokio::test]
    async fn stt_discovery_supports_deepgram_elevenlabs_and_soniox() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .and(header("authorization", "Token dg-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "stt": [
                    { "name": "general", "canonical_name": "nova-3-general" }
                ]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .and(header("xi-api-key", "xi-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "model_id": "eleven_multilingual_v2", "can_do_text_to_speech": true },
                { "model_id": "eleven_english_sts_v2", "can_do_text_to_speech": false },
                { "model_id": "scribe_v2", "can_do_text_to_speech": false },
                { "model_id": "scribe_v2_realtime" }
            ])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .and(header("authorization", "Bearer sx-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "models": [{ "id": "stt-rt-v5" }, { "id": "stt-async-v5" }]
            })))
            .mount(&server)
            .await;

        assert_eq!(
            fetch_models(
                ProviderKind::Stt,
                "deepgram",
                &format!("{}/v1", server.uri()),
                Some("dg-key"),
                None,
            )
            .await
            .into_iter()
            .map(|m| m.id)
            .collect::<Vec<_>>(),
            vec!["nova-3-general"]
        );
        assert_eq!(
            fetch_models(
                ProviderKind::Stt,
                "elevenlabs",
                &server.uri(),
                Some("xi-key"),
                None,
            )
            .await
            .into_iter()
            .map(|m| m.id)
            .collect::<Vec<_>>(),
            vec!["scribe_v2", "scribe_v2_realtime"]
        );
        assert_eq!(
            fetch_models(
                ProviderKind::Stt,
                "soniox",
                &server.uri(),
                Some("sx-key"),
                None,
            )
            .await
            .into_iter()
            .map(|m| m.id)
            .collect::<Vec<_>>(),
            vec!["stt-rt-v5"]
        );
    }

    #[tokio::test]
    async fn rejects_non_https_remote_urls_and_redirects() {
        assert!(validate_url("http://api.openai.com/v1/models").is_none());
        assert!(validate_url("http://127.0.0.1:11434/v1/models").is_some());
        assert!(validate_url("https://api.openai.com/v1/models").is_some());

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(
                ResponseTemplate::new(302).insert_header("location", "http://127.0.0.1:9999/steal"),
            )
            .mount(&server)
            .await;

        assert!(
            fetch_models(
                ProviderKind::Llm,
                "openai",
                &format!("{}/v1", server.uri()),
                Some("oa-key"),
                None,
            )
            .await
            .is_empty()
        );
    }

    #[tokio::test]
    async fn vertex_prediction_base_falls_back_to_origin_publisher_models() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(
                "/v1/projects/p/locations/global/endpoints/openapi/models",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "models": []
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1beta1/publishers/google/models"))
            .and(header("authorization", "Bearer vtx-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "publisherModels": [
                    { "name": "publishers/google/models/gemini-2.5-pro" },
                    { "name": "publishers/google/models/veo-3.0-generate-001" },
                    { "name": "publishers/google/models/imagen-3.0-generate-002" }
                ]
            })))
            .mount(&server)
            .await;

        let models = fetch_models(
            ProviderKind::Llm,
            "google_vertex_ai",
            &format!(
                "{}/v1/projects/p/locations/global/endpoints/openapi",
                server.uri()
            ),
            Some("vtx-key"),
            None,
        )
        .await;
        assert_eq!(
            models,
            vec![ModelEntry {
                id: "google/gemini-2.5-pro".into(),
                label: "google/gemini-2.5-pro".into(),
            }]
        );
    }

    #[tokio::test]
    async fn ollama_filters_non_completion_models_via_show() {
        use wiremock::matchers::body_json;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    { "id": "llama3.2:latest" },
                    { "id": "nomic-embed-text:latest" }
                ]
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/show"))
            .and(body_json(serde_json::json!({ "model": "llama3.2:latest" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "capabilities": ["completion", "tools"]
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/show"))
            .and(body_json(
                serde_json::json!({ "model": "nomic-embed-text:latest" }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "capabilities": ["embedding"]
            })))
            .mount(&server)
            .await;

        let models = fetch_models(
            ProviderKind::Llm,
            "ollama",
            &format!("{}/v1", server.uri()),
            None,
            None,
        )
        .await;
        assert_eq!(
            models,
            vec![ModelEntry {
                id: "llama3.2:latest".into(),
                label: "llama3.2:latest".into(),
            }]
        );
    }

    #[test]
    fn stt_filters_exclude_mistral_tts_and_cloudflare_whisper() {
        let mistral = parse_mistral_stt_ids(&serde_json::json!({
            "data": [
                { "id": "voxtral-mini-2602" },
                { "id": "voxtral-mini-tts-3-0" },
                { "id": "custom-stt", "audio_transcription_capabilities": { "realtime": true } },
                { "id": "voxtral-disabled", "capabilities": { "audio_transcription": false } }
            ]
        }));
        assert_eq!(mistral, vec!["voxtral-mini-2602", "custom-stt"]);

        let cloudflare = parse_cloudflare_stt_ids(&serde_json::json!({
            "result": [
                { "name": "@cf/deepgram/nova-3" },
                { "name": "@cf/deepgram/flux" },
                { "name": "@cf/openai/whisper" },
                { "name": "@cf/openai/whisper-large-v3-turbo" }
            ]
        }));
        assert_eq!(cloudflare, vec!["@cf/deepgram/nova-3", "@cf/deepgram/flux"]);
    }

    #[tokio::test]
    async fn mistral_llm_filters_by_completion_chat_and_non_chat_ids() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {
                        "id": "mistral-large-latest",
                        "capabilities": { "completion_chat": true }
                    },
                    {
                        "id": "mistral-embed-capable",
                        "capabilities": { "completion_chat": false }
                    },
                    { "id": "mistral-embed" }
                ]
            })))
            .mount(&server)
            .await;

        let models = fetch_models(
            ProviderKind::Llm,
            "mistral",
            &server.uri(),
            Some("m-key"),
            None,
        )
        .await;
        assert_eq!(
            models,
            vec![ModelEntry {
                id: "mistral-large-latest".into(),
                label: "mistral-large-latest".into(),
            }]
        );
    }

    #[test]
    fn together_stt_filters_out_audio_tts_models() {
        let together = parse_together_stt_ids(&serde_json::json!([
            { "id": "openai/whisper-large-v3", "type": "transcribe" },
            { "id": "distil-whisper/distil-large-v3", "type": "audio" },
            { "id": "hexgrad/Kokoro-82M", "type": "audio" },
            { "id": "vendor/text-to-speech-v1", "type": "audio" },
            { "id": "meta-llama/Llama-3.3-70B", "type": "chat" }
        ]));
        assert_eq!(
            together,
            vec!["openai/whisper-large-v3", "distil-whisper/distil-large-v3"]
        );
    }

    #[tokio::test]
    async fn lmstudio_excludes_embeddings_and_keeps_untyped_openai_models() {
        assert!(!supports_live_models(ProviderKind::Llm, "azure_openai"));

        // 1. Native /api/v1/models keeps llm entries and drops embeddings.
        let native_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "models": [
                    { "type": "llm", "key": "qwen2.5-7b-instruct" },
                    { "type": "embeddings", "key": "text-embedding-nomic" }
                ]
            })))
            .mount(&native_server)
            .await;

        let native_models = fetch_models(
            ProviderKind::Llm,
            "lmstudio",
            &format!("{}/v1", native_server.uri()),
            None,
            None,
        )
        .await;
        assert_eq!(
            native_models,
            vec![ModelEntry {
                id: "qwen2.5-7b-instruct".into(),
                label: "qwen2.5-7b-instruct".into(),
            }]
        );

        // 2. A valid /api/v1/models response with only embeddings does not fall back to /v1/models.
        let embed_only_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "models": [
                    { "type": "embeddings", "key": "text-embedding-nomic" }
                ]
            })))
            .mount(&embed_only_server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{ "id": "text-embedding-nomic" }]
            })))
            .mount(&embed_only_server)
            .await;
        assert!(
            fetch_models(
                ProviderKind::Llm,
                "lmstudio",
                &format!("{}/v1", embed_only_server.uri()),
                None,
                None,
            )
            .await
            .is_empty()
        );

        // 3. When /api/v1/models is absent, /v1/models keeps untyped models and filters typed embeddings.
        let compat_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    { "id": "legacy-untyped-model" },
                    { "id": "nomic-embed", "type": "embedding" }
                ]
            })))
            .mount(&compat_server)
            .await;
        let compat_models = fetch_models(
            ProviderKind::Llm,
            "lmstudio",
            &format!("{}/v1", compat_server.uri()),
            None,
            None,
        )
        .await;
        assert_eq!(
            compat_models,
            vec![ModelEntry {
                id: "legacy-untyped-model".into(),
                label: "legacy-untyped-model".into(),
            }]
        );

        // 4. Custom loaded aliases from /v1/models are preserved while native-typed embeddings are excluded.
        let alias_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "models": [
                    {
                        "type": "llm",
                        "key": "qwen/qwen2.5-7b",
                        "loaded_instances": [{ "id": "meeting-assistant" }]
                    },
                    {
                        "type": "embedding",
                        "key": "bge-m3",
                        "loaded_instances": [{ "id": "vec-alias" }]
                    }
                ]
            })))
            .mount(&alias_server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    { "id": "meeting-assistant" },
                    { "id": "vec-alias" },
                    { "id": "bge-m3" }
                ]
            })))
            .mount(&alias_server)
            .await;
        let alias_models = fetch_models(
            ProviderKind::Llm,
            "lmstudio",
            &format!("{}/v1", alias_server.uri()),
            None,
            None,
        )
        .await;
        assert_eq!(
            alias_models,
            vec![ModelEntry {
                id: "meeting-assistant".into(),
                label: "meeting-assistant".into(),
            }]
        );
    }

    #[tokio::test]
    async fn ollama_discovery_enforces_single_overall_timeout() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(50))
                    .set_body_json(serde_json::json!({
                        "data": [
                            { "id": "m1" },
                            { "id": "m2" },
                            { "id": "m3" },
                            { "id": "m4" },
                            { "id": "m5" },
                            { "id": "m6" }
                        ]
                    })),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/show"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(250))
                    .set_body_json(serde_json::json!({ "capabilities": ["completion"] })),
            )
            .mount(&server)
            .await;

        let started = std::time::Instant::now();
        let models = fetch_ollama_models_within(
            Duration::from_millis(80),
            &format!("{}/v1", server.uri()),
            &[],
        )
        .await;
        assert!(models.is_empty());
        assert!(started.elapsed() < Duration::from_millis(300));
    }
}
