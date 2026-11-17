use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use desktop_runtime::Result;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{
    config::{ProviderKind, ProviderServices},
    model::failure,
};

pub type SecretWriter =
    Arc<dyn Fn(ProviderKind, String, String) -> BoxFuture<'static, Result<()>> + Send + Sync>;

#[derive(Deserialize, Serialize)]
pub(super) struct Credential {
    #[serde(rename = "type")]
    kind: String,
    refresh: String,
    pub access: String,
    expires: u64,
    #[serde(rename = "accountId", skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
}

pub(super) fn parse(secret: &str) -> Option<Credential> {
    serde_json::from_str::<Credential>(secret)
        .ok()
        .filter(|c| c.kind == "oauth" && !c.refresh.is_empty())
}

static REFRESH: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub(super) async fn resolve(
    services: &ProviderServices,
    provider: &str,
    secret: String,
) -> Result<String> {
    if !matches!(provider, "claude" | "chatgpt" | "grok" | "github_copilot") {
        return Ok(secret);
    }
    let Some(mut credential) = parse(&secret) else {
        return Ok(secret);
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(failure)?
        .as_millis() as u64;
    if !credential.access.is_empty() && credential.expires > now + 120_000 {
        return Ok(secret);
    }
    let _lock = REFRESH.lock().await;
    if let Some(latest) = (services.secret)(ProviderKind::Llm, provider.into())
        .await?
        .and_then(|s| parse(&s))
    {
        credential = latest;
        if !credential.access.is_empty() && credential.expires > now + 120_000 {
            return serde_json::to_string(&credential).map_err(failure);
        }
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(failure)?;
    let request = match provider {
        "claude" => client.post("https://platform.claude.com/v1/oauth/token").json(&json!({
            "grant_type":"refresh_token", "client_id":"9d1c250a-e61b-44d9-88ed-5944d1962f5e", "refresh_token":credential.refresh
        })),
        "chatgpt" => client.post("https://auth.openai.com/oauth/token").form(&[
            ("grant_type","refresh_token"),("client_id","app_EMoamEEZ73f0CkXaXp7hrann"),("refresh_token", &credential.refresh)
        ]),
        "grok" => client.post("https://auth.x.ai/oauth2/token").form(&[
            ("grant_type","refresh_token"),("client_id","b1a00492-073a-47ea-816f-4c329264a828"),("refresh_token", &credential.refresh)
        ]),
        "github_copilot" => client.get("https://api.github.com/copilot_internal/v2/token")
            .bearer_auth(&credential.refresh).header("Accept", "application/json")
            .header("User-Agent", "GitHubCopilotChat/0.26.7").header("Editor-Version", "vscode/1.99.3")
            .header("Editor-Plugin-Version","copilot-chat/0.26.7").header("Copilot-Integration-Id","vscode-chat"),
        _ => return Err(failure("Unknown subscription provider.")),
    };
    let mut response = request
        .send()
        .await
        .map_err(|_| failure("Subscription refresh failed; sign in again."))?;
    if !response.status().is_success() {
        return Err(failure(format!(
            "Subscription refresh returned {}; sign in again.",
            response.status()
        )));
    }
    if response.content_length().is_some_and(|len| len > 65536) {
        return Err(failure("Oversized subscription response."));
    }
    let mut data = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| failure("Invalid subscription response."))?
    {
        if data.len() + chunk.len() > 65536 {
            return Err(failure("Oversized subscription response."));
        }
        data.extend_from_slice(&chunk);
    }
    let body: Value =
        serde_json::from_slice(&data).map_err(|_| failure("Invalid subscription response."))?;
    credential.access = body[if provider == "github_copilot" {
        "token"
    } else {
        "access_token"
    }]
    .as_str()
    .filter(|s| !s.is_empty())
    .ok_or_else(|| failure("Subscription refresh did not return access."))?
    .into();
    if let Some(refresh) = body["refresh_token"].as_str() {
        credential.refresh = refresh.into();
    }
    credential.expires = if provider == "github_copilot" {
        body["expires_at"]
            .as_u64()
            .unwrap_or(now / 1000 + 1500)
            .saturating_mul(1000)
    } else {
        now.saturating_add(
            body["expires_in"]
                .as_u64()
                .unwrap_or(3600)
                .saturating_mul(1000),
        )
    };
    if let Some(account) = body["account_id"].as_str() {
        credential.account_id = Some(account.into());
    }
    let encoded = serde_json::to_string(&credential).map_err(failure)?;
    (services.secret_write)(ProviderKind::Llm, provider.into(), encoded.clone()).await?;
    Ok(encoded)
}

pub fn keyring_writer(identifier: String) -> SecretWriter {
    Arc::new(move |kind, provider, value| {
        let identifier = identifier.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let canonical = super::config::canonical_identifier(&identifier);
                let account = format!("ai-provider-api-keys:{}:{provider}", kind.key());
                let account = if identifier == "com.hyprnote.dev" {
                    format!("v2:{account}")
                } else {
                    account
                };
                keyring::Entry::new(&format!("{canonical}.secure-store"), &account)
                    .map_err(|_| failure("Could not open the secure provider store."))?
                    .set_password(&value)
                    .map_err(|_| {
                        failure("Could not save refreshed subscription access to the keyring.")
                    })
            })
            .await
            .map_err(failure)?
        })
    })
}
