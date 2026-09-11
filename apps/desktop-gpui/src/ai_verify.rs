//! `packages/provider-validation`: proving an API key before a provider
//! counts as configured — the provider's own authenticated probe, an
//! invalid-key control request for gateways that would accept anything, and
//! a one-minute memory of what was just verified.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

/// `ProviderCredentialError`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialError {
    pub message: String,
    pub retryable: bool,
}

impl CredentialError {
    fn fixed(message: &str) -> Self {
        Self {
            message: message.to_string(),
            retryable: false,
        }
    }

    fn retryable(message: &str) -> Self {
        Self {
            message: message.to_string(),
            retryable: true,
        }
    }
}

const REJECTED: &str =
    "The provider rejected this key or its permissions. Check the key and try again.";
const RATE_LIMITED: &str = "The provider is rate limiting verification. Try again shortly.";
const UNREACHABLE: &str =
    "Couldn’t verify this key with the provider. Check the connection and try again.";
const UNCONFIRMED: &str = "The provider did not confirm this key. Check the key and connection.";
const NO_VERIFICATION: &str =
    "This endpoint doesn’t support API key verification. Use an authenticated model-list endpoint.";
const NETWORK: &str = "Couldn’t verify this key. Check your connection and try again.";
const CONTROL_KEY: &str = "anarlog-invalid-key-verification";
const RESPONSE_LIMIT: usize = 8 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(8);

/// `verified`: identities proven within the last minute.
static RECENT: LazyLock<Mutex<HashMap<String, Instant>>> = LazyLock::new(Default::default);

/// `providerCredentialIdentity`: sha256 of the JSON credential.
pub fn credential_identity(provider: &str, base_url: &str, api_key: &str) -> String {
    use sha2::Digest as _;
    let json = serde_json::json!({ "provider": provider, "baseUrl": base_url, "apiKey": api_key });
    let digest = sha2::Sha256::digest(json.to_string().as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

struct Request {
    url: String,
    method: reqwest::Method,
    headers: Vec<(&'static str, String)>,
    body: Option<String>,
    check_authentication: bool,
    plain_text: bool,
    accept: fn(&Value) -> bool,
}

fn record(value: &Value) -> &serde_json::Map<String, Value> {
    static EMPTY: LazyLock<serde_json::Map<String, Value>> = LazyLock::new(Default::default);
    value.as_object().unwrap_or(&EMPTY)
}

fn is_truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64() != Some(0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

fn accept_model_list(value: &Value) -> bool {
    let data = record(value);
    !is_truthy(data.get("error"))
        && (data.get("data").is_some_and(Value::is_array)
            || data.get("models").is_some_and(Value::is_array))
}

/// `credentialRequest`
fn credential_request(provider: &str, base_url: &str, api_key: &str) -> Option<Request> {
    let base = base_url.trim_end_matches('/').to_string();
    let parsed = url::Url::parse(&base).ok()?;
    let origin = parsed.origin().ascii_serialization();
    let hostname = parsed.host_str().unwrap_or_default().to_string();
    let mut request = Request {
        url: format!("{base}/models"),
        method: reqwest::Method::GET,
        headers: vec![("Authorization", format!("Bearer {api_key}"))],
        body: None,
        check_authentication: false,
        plain_text: false,
        accept: accept_model_list,
    };
    match provider {
        "anthropic" => {
            request.check_authentication = hostname != "api.anthropic.com";
            request.headers = vec![
                ("x-api-key", api_key.to_string()),
                ("anthropic-version", "2023-06-01".to_string()),
            ];
        }
        "google_generative_ai" => {
            request.check_authentication = hostname != "generativelanguage.googleapis.com";
            request.headers = vec![("x-goog-api-key", api_key.to_string())];
        }
        "azure_openai" => {
            let re = regex::Regex::new(r"/openai(?:/v1)?$").expect("valid pattern");
            request.url = format!(
                "{}/openai/models?api-version=2024-10-21",
                re.replace(&base, "")
            );
            request.headers = vec![("api-key", api_key.to_string())];
        }
        "azure_ai" => request.headers = vec![("api-key", api_key.to_string())],
        "azure_speech" => {
            request.url = format!("{origin}/sts/v1.0/issueToken");
            request.method = reqwest::Method::POST;
            request.headers = vec![("Ocp-Apim-Subscription-Key", api_key.to_string())];
            request.plain_text = true;
            request.accept = |value| value.as_str().is_some_and(|s| s.split('.').count() == 3);
        }
        "google_cloud" => {
            request.url = format!("{base}/operations?pageSize=1");
            request.check_authentication = hostname != "speech.googleapis.com";
            request.accept = |value| {
                value.is_object()
                    && !is_truthy(record(value).get("error"))
                    && record(value).get("operations").is_none_or(Value::is_array)
            };
        }
        "google_vertex_ai" => {
            request.url = format!("{origin}/v1beta1/publishers/google/models?pageSize=1");
            request.accept = |value| {
                record(value)
                    .get("publisherModels")
                    .is_some_and(Value::is_array)
            };
            request.check_authentication = true;
        }
        "openrouter" => {
            request.url = format!("{base}/key");
            request.accept = |value| {
                record(value)
                    .get("data")
                    .and_then(|data| record(data).get("label"))
                    .is_some_and(Value::is_string)
            };
        }
        "deepgram" => {
            request.url = format!("{base}/projects");
            request.headers = vec![("Authorization", format!("Token {api_key}"))];
            request.accept = |value| record(value).get("projects").is_some_and(Value::is_array);
        }
        "assemblyai" => {
            request.url = format!("{base}/v2/transcript?limit=1");
            request.headers = vec![("Authorization", api_key.to_string())];
            request.accept = |value| {
                record(value)
                    .get("transcripts")
                    .is_some_and(Value::is_array)
            };
        }
        "siliconflow" => {
            request.url = format!("{base}/user/info");
            request.accept = |value| record(value).get("status") == Some(&Value::Bool(true));
        }
        "xai" => {
            request.url = format!("{base}/api-key");
            request.accept = |value| {
                record(value).get("api_key_blocked") == Some(&Value::Bool(false))
                    && record(value).get("api_key_disabled") == Some(&Value::Bool(false))
            };
        }
        "cloudflare_workers_ai" => {
            request.url = format!("{origin}/client/v4/user/tokens/verify");
            request.accept = |value| {
                record(value).get("success") == Some(&Value::Bool(true))
                    && record(value)
                        .get("result")
                        .and_then(|result| record(result).get("status"))
                        .and_then(Value::as_str)
                        == Some("active")
            };
        }
        "cartesia" => {
            request.url = format!("{base}/voices?limit=1");
            request
                .headers
                .push(("Cartesia-Version", "2025-04-16".to_string()));
            request.check_authentication = true;
            request.accept =
                |value| value.is_array() || record(value).get("data").is_some_and(Value::is_array);
        }
        "smallestai" => {
            request.url = format!("{base}/waves/v1/voice-cloning");
            request.check_authentication = true;
            request.accept = |value| record(value).get("data").is_some_and(Value::is_array);
        }
        "fireworks" => {
            request.url = format!("{origin}/inference/v1/models");
            request.check_authentication = true;
        }
        "deepseek" => {
            request.url = format!("{base}/user/balance");
            request.accept = |value| {
                record(value)
                    .get("is_available")
                    .is_some_and(Value::is_boolean)
            };
        }
        "cohere" => {
            request.url = format!("{origin}/v1/check-api-key");
            request.method = reqwest::Method::POST;
            request
                .headers
                .push(("Content-Type", "application/json".to_string()));
            request.body = Some("{}".to_string());
            request.accept = |value| record(value).get("valid") == Some(&Value::Bool(true));
        }
        "soniox" => {
            request.url = format!("{base}/v1/transcriptions?limit=1");
            request.accept = |value| {
                record(value)
                    .get("transcriptions")
                    .is_some_and(Value::is_array)
            };
        }
        "speechmatics" => {
            request.url = format!("{base}/jobs");
            request.accept = |value| record(value).get("jobs").is_some_and(Value::is_array);
        }
        "revai" => {
            request.url = format!("{base}/account");
            request.accept = |value| record(value).get("id").is_some_and(Value::is_string);
        }
        "elevenlabs" => {
            request.url = format!("{base}/v1/user");
            request.headers = vec![("xi-api-key", api_key.to_string())];
            request.accept = |value| record(value).get("user_id").is_some_and(Value::is_string);
        }
        "gladia" => {
            request.url = format!("{base}/v2/live?limit=1");
            request.headers = vec![("x-gladia-key", api_key.to_string())];
            request.accept = |value| record(value).get("items").is_some_and(Value::is_array);
        }
        "pyannote" => {
            request.url = format!("{base}/v1/test");
            request.accept =
                |value| record(value).get("status").and_then(Value::as_str) == Some("OK");
        }
        "dashscope" => {
            request.url = format!("{base}/compatible-mode/v1/models");
            request.check_authentication = true;
        }
        "openai" => request.check_authentication = hostname != "api.openai.com",
        "groq" => request.check_authentication = hostname != "api.groq.com",
        "mistral" => request.check_authentication = hostname != "api.mistral.ai",
        _ => request.check_authentication = true,
    }
    Some(request)
}

/// The static checks `verifyProviderCredentials` runs before any request.
pub fn validate_credential(base_url: &str, api_key: &str) -> Result<(), CredentialError> {
    let key = api_key.trim();
    if key.is_empty() || key.len() > 8192 || key.contains(['\r', '\n']) {
        return Err(CredentialError::fixed("Enter a valid API key."));
    }
    let base =
        url::Url::parse(base_url).map_err(|_| CredentialError::fixed("Enter a valid base URL."))?;
    if !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
    {
        return Err(CredentialError::fixed(
            "Enter a base URL without credentials or query parameters.",
        ));
    }
    let local = matches!(base.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if base.scheme() != "https" && !(base.scheme() == "http" && local) {
        return Err(CredentialError::fixed(
            "Use HTTPS for provider credentials.",
        ));
    }
    Ok(())
}

async fn send(
    client: &reqwest::Client,
    request: &Request,
    api_key_override: Option<&str>,
    provider: &str,
    base_url: &str,
) -> Result<reqwest::Response, reqwest::Error> {
    // The control request rebuilds the headers with the invalid key.
    let rebuilt = api_key_override.and_then(|key| credential_request(provider, base_url, key));
    let request = rebuilt.as_ref().unwrap_or(request);
    let mut builder = client.request(request.method.clone(), &request.url);
    for (name, value) in &request.headers {
        builder = builder.header(*name, value);
    }
    if let Some(body) = &request.body {
        builder = builder.body(body.clone());
    }
    builder.send().await
}

/// `ProviderCredential.type`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialKind {
    Stt,
    Llm,
}

/// `verifyProviderCredentials`
pub async fn verify_provider_credentials(
    kind: CredentialKind,
    provider: &str,
    base_url: &str,
    api_key: &str,
) -> Result<(), CredentialError> {
    validate_credential(base_url, api_key)?;
    // Deepgram-compatible listen servers need not expose a model catalog or
    // a credential-probe endpoint; their credentials are checked when
    // transcribing (#7446).
    if kind == CredentialKind::Stt && provider == "custom" {
        return Ok(());
    }
    let api_key = api_key.trim();
    let identity = credential_identity(provider, base_url, api_key);
    if RECENT
        .lock()
        .map(|recent| {
            recent
                .get(&identity)
                .is_some_and(|until| *until > Instant::now())
        })
        .unwrap_or(false)
    {
        return Ok(());
    }
    let request = credential_request(provider, base_url, api_key)
        .ok_or_else(|| CredentialError::fixed("Enter a valid base URL."))?;
    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| CredentialError::retryable(NETWORK))?;

    let response = send(&client, &request, None, provider, base_url)
        .await
        .map_err(|_| CredentialError::retryable(NETWORK))?;
    let status = response.status();
    if status == 401 || status == 403 {
        return Err(CredentialError::fixed(REJECTED));
    }
    if status.is_redirection() {
        // `redirect: "error"`
        return Err(CredentialError::retryable(NETWORK));
    }
    if !status.is_success() {
        return Err(CredentialError::retryable(if status == 429 {
            RATE_LIMITED
        } else {
            UNREACHABLE
        }));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| CredentialError::retryable(NETWORK))?;
    if bytes.len() > RESPONSE_LIMIT {
        return Err(CredentialError::retryable(NETWORK));
    }
    let body: Value = if request.plain_text {
        Value::String(String::from_utf8_lossy(&bytes).into_owned())
    } else {
        serde_json::from_slice(&bytes).map_err(|_| CredentialError::retryable(NETWORK))?
    };
    if !(request.accept)(&body) {
        return Err(CredentialError::fixed(UNCONFIRMED));
    }

    // A public model catalog cannot prove that credentials work. Gateways must
    // also reject an invalid credential before their model response is trusted.
    if request.check_authentication {
        let rejected = send(&client, &request, Some(CONTROL_KEY), provider, base_url)
            .await
            .map_err(|_| CredentialError::retryable(NETWORK))?;
        let status = rejected.status();
        if status != 401 && status != 403 {
            if !status.is_success() {
                return Err(CredentialError::retryable(if status == 429 {
                    RATE_LIMITED
                } else {
                    UNREACHABLE
                }));
            }
            return Err(CredentialError::fixed(NO_VERIFICATION));
        }
    }
    if let Ok(mut recent) = RECENT.lock() {
        recent.insert(identity, Instant::now() + Duration::from_secs(60));
        if recent.len() > 128 {
            let oldest = recent
                .iter()
                .min_by_key(|(_, until)| **until)
                .map(|(key, _)| key.clone());
            if let Some(key) = oldest {
                recent.remove(&key);
            }
        }
    }
    Ok(())
}

/// `checkOllamaAvailability` / `checkLMStudioAvailability` /
/// `checkUnslothAvailability`: any of the endpoints answering 2xx within the
/// listing timeout.
pub async fn check_local_availability(provider: &str, base_url: &str, api_key: &str) -> bool {
    let requests: Vec<(String, Vec<(&'static str, String)>)> = match provider {
        "ollama" => {
            let Ok(mut url) = url::Url::parse(base_url) else {
                return false;
            };
            let path = url.path().trim_end_matches('/').to_string();
            let path = path.strip_suffix("/v1").unwrap_or(&path).to_string();
            url.set_path(&format!("{path}/api/version"));
            let origin = url.origin().ascii_serialization();
            vec![(url.to_string(), vec![("Origin", origin)])]
        }
        "lmstudio" => {
            let headers = optional_bearer(api_key);
            let mut requests = Vec::new();
            if let Some(native) = crate::ai_models::lmstudio_native_models_url(base_url) {
                requests.push((native, headers.clone()));
            }
            requests.push((
                format!("{}/models", base_url.trim_end_matches('/')),
                headers,
            ));
            requests
        }
        "unsloth" => vec![(
            format!("{}/models", base_url.trim_end_matches('/')),
            optional_bearer(api_key),
        )],
        // `checkAppleFoundationModelAvailability` needs macOS.
        _ => return false,
    };
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    else {
        return false;
    };
    for (url, headers) in requests {
        let mut builder = client.get(&url);
        for (name, value) in &headers {
            builder = builder.header(*name, value);
        }
        if let Ok(response) = builder.send().await
            && response.status().is_success()
        {
            return true;
        }
    }
    false
}

fn optional_bearer(api_key: &str) -> Vec<(&'static str, String)> {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        Vec::new()
    } else {
        vec![("Authorization", format!("Bearer {trimmed}"))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_validation_matches_the_package() {
        assert_eq!(
            validate_credential("https://api.openai.com/v1", "")
                .unwrap_err()
                .message,
            "Enter a valid API key."
        );
        assert_eq!(
            validate_credential("https://api.openai.com/v1", "a\nb")
                .unwrap_err()
                .message,
            "Enter a valid API key."
        );
        assert_eq!(
            validate_credential("not a url", "key").unwrap_err().message,
            "Enter a valid base URL."
        );
        assert_eq!(
            validate_credential("https://u:p@api.openai.com/v1", "key")
                .unwrap_err()
                .message,
            "Enter a base URL without credentials or query parameters."
        );
        assert_eq!(
            validate_credential("https://api.openai.com/v1?x=1", "key")
                .unwrap_err()
                .message,
            "Enter a base URL without credentials or query parameters."
        );
        assert_eq!(
            validate_credential("http://example.com/v1", "key")
                .unwrap_err()
                .message,
            "Use HTTPS for provider credentials."
        );
        assert!(validate_credential("http://127.0.0.1:8765/v1", "key").is_ok());
        assert!(validate_credential("http://localhost:1234/v1", "key").is_ok());
        assert!(validate_credential("https://api.openai.com/v1", "key").is_ok());
    }

    #[test]
    fn requests_follow_credential_request() {
        let openai = credential_request("openai", "https://api.openai.com/v1/", "k").unwrap();
        assert_eq!(openai.url, "https://api.openai.com/v1/models");
        assert!(!openai.check_authentication);
        let proxied = credential_request("openai", "https://proxy.example/v1", "k").unwrap();
        assert!(proxied.check_authentication);
        let custom = credential_request("custom", "http://127.0.0.1:8765/v1", "k").unwrap();
        assert!(custom.check_authentication);
        assert_eq!(
            custom.headers,
            vec![("Authorization", "Bearer k".to_string())]
        );
        let azure = credential_request("azure_openai", "https://a.openai.azure.com/openai/v1", "k")
            .unwrap();
        assert_eq!(
            azure.url,
            "https://a.openai.azure.com/openai/models?api-version=2024-10-21"
        );
        let cohere = credential_request("cohere", "https://api.cohere.com/v2", "k").unwrap();
        assert_eq!(cohere.url, "https://api.cohere.com/v1/check-api-key");
        assert_eq!(cohere.method, reqwest::Method::POST);
        assert_eq!(cohere.body.as_deref(), Some("{}"));
        let deepgram = credential_request("deepgram", "https://api.deepgram.com/v1", "k").unwrap();
        assert_eq!(deepgram.url, "https://api.deepgram.com/v1/projects");
        assert_eq!(
            deepgram.headers,
            vec![("Authorization", "Token k".to_string())]
        );
        let speech = credential_request(
            "azure_speech",
            "https://eastus.api.cognitive.microsoft.com/sts",
            "k",
        )
        .unwrap();
        assert_eq!(
            speech.url,
            "https://eastus.api.cognitive.microsoft.com/sts/v1.0/issueToken"
        );
        assert!(speech.plain_text);
    }

    #[test]
    fn accept_predicates_read_the_provider_shapes() {
        let list = credential_request("custom", "https://x/v1", "k").unwrap();
        assert!((list.accept)(&serde_json::json!({ "data": [] })));
        assert!((list.accept)(&serde_json::json!({ "models": [] })));
        assert!(!(list.accept)(
            &serde_json::json!({ "data": [], "error": { "message": "x" } })
        ));
        assert!(!(list.accept)(&serde_json::json!({ "object": "list" })));
        let speech = credential_request("azure_speech", "https://x/sts", "k").unwrap();
        assert!((speech.accept)(&Value::String("a.b.c".into())));
        assert!(!(speech.accept)(&Value::String("a.b".into())));
        let openrouter =
            credential_request("openrouter", "https://openrouter.ai/api/v1", "k").unwrap();
        assert!((openrouter.accept)(
            &serde_json::json!({ "data": { "label": "sk-…" } })
        ));
        assert!(!(openrouter.accept)(&serde_json::json!({ "data": {} })));
        let cloudflare = credential_request(
            "cloudflare_workers_ai",
            "https://api.cloudflare.com/client/v4/accounts/a/ai/v1",
            "k",
        )
        .unwrap();
        assert_eq!(
            cloudflare.url,
            "https://api.cloudflare.com/client/v4/user/tokens/verify"
        );
        assert!((cloudflare.accept)(
            &serde_json::json!({ "success": true, "result": { "status": "active" } })
        ));
        assert!(!(cloudflare.accept)(
            &serde_json::json!({ "success": true, "result": { "status": "expired" } })
        ));
    }

    #[test]
    fn identities_are_stable_sha256_digests() {
        let a = credential_identity("openai", "https://api.openai.com/v1", "k");
        let b = credential_identity("openai", "https://api.openai.com/v1", "k");
        let c = credential_identity("openai", "https://api.openai.com/v1", "other");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 64);
    }

    /// #7446: a Custom STT endpoint is accepted without a probe once the
    /// static checks pass; the same credential on the LLM side still probes
    /// (and fails against a closed port).
    #[tokio::test]
    async fn custom_stt_credentials_skip_the_probe() {
        let stt = verify_provider_credentials(
            CredentialKind::Stt,
            "custom",
            "http://127.0.0.1:9/v1",
            "key",
        )
        .await;
        assert!(stt.is_ok());
        let https = verify_provider_credentials(
            CredentialKind::Stt,
            "custom",
            "http://example.com/v1",
            "key",
        )
        .await;
        assert_eq!(
            https.unwrap_err().message,
            "Use HTTPS for provider credentials."
        );
        let llm = verify_provider_credentials(
            CredentialKind::Llm,
            "custom",
            "http://127.0.0.1:9/v1",
            "key",
        )
        .await;
        assert!(llm.is_err());
    }
}
