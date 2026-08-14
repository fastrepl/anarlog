mod storage;

use std::io::{IsTerminal, Read};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anlg_supabase_auth::session::{AppMetadata, Session, SessionUser, UserMetadata};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use url::Url;

use crate::cli::AuthCommand;
use crate::{Error, Result, output};

use self::storage::{AuthStore, Backend};

const WEB_APP_URL: &str = "https://anarlog.so";
const API_URL: &str = "https://api.anarlog.so";
const MAX_CALLBACK_BYTES: usize = 32 * 1024;

pub async fn run(command: &AuthCommand, json: bool) -> Result<()> {
    let environment = Environment::current();
    let store = AuthStore::new(environment.bundle_id)?;

    match command {
        AuthCommand::Login => login(&store, environment, json).await,
        AuthCommand::Status => status(&store, json),
        AuthCommand::Logout => logout(&store, json),
    }
}

async fn login(store: &AuthStore, environment: Environment, json: bool) -> Result<()> {
    let login_url = login_url(environment.scheme)?;
    if json {
        eprintln!("Open this URL in any browser:\n{login_url}\n");
        eprintln!("After signing in, choose Copy URL and paste the callback link below.");
    } else {
        output::emit(&format!(
            "Open this URL in any browser:\n\n{login_url}\n\nAfter signing in, choose Copy URL and paste the callback link below."
        ));
    }

    let callback = read_callback_url()?;
    let tokens = CallbackTokens::parse(&callback, environment.scheme)?;
    let session = verify_and_build_session(tokens).await?;
    let user_id = session
        .user
        .as_ref()
        .map(|user| user.id.clone())
        .ok_or_else(|| Error::operation("complete login", "account identity is missing"))?;
    let email = session.user.as_ref().and_then(|user| user.email.clone());
    let storage_key = storage_key(&session.access_token)?;
    let mut data = store.load()?.data;
    data.retain(|key, _| !key.ends_with("-auth-token"));
    data.insert(
        storage_key,
        serde_json::to_string(&session)
            .map_err(|error| Error::operation("serialize login", error.to_string()))?,
    );
    let backend = store.save(&data)?;

    emit_status(
        "auth.login",
        AuthStatus::signed_in(user_id, email, session.expires_at, backend, store),
        json,
    )
}

fn status(store: &AuthStore, json: bool) -> Result<()> {
    let loaded = store.load()?;
    let Some(session) = storage::find_session(&loaded.data)? else {
        return emit_status("auth.status", AuthStatus::signed_out(store), json);
    };
    let user = session
        .user
        .ok_or_else(|| Error::operation("read login", "stored account identity is missing"))?;

    emit_status(
        "auth.status",
        AuthStatus::signed_in(
            user.id,
            user.email,
            session.expires_at,
            loaded.backend,
            store,
        ),
        json,
    )
}

fn logout(store: &AuthStore, json: bool) -> Result<()> {
    store.remove_sessions()?;
    emit_status("auth.logout", AuthStatus::signed_out(store), json)
}

fn emit_status(command: &'static str, status: AuthStatus, json: bool) -> Result<()> {
    if json {
        output::emit(&output::json(command, &status, None)?);
        return Ok(());
    }

    if !status.signed_in {
        output::emit("Not signed in.");
        return Ok(());
    }

    let identity = status.email.as_deref().unwrap_or_else(|| {
        status
            .user_id
            .as_deref()
            .expect("signed-in status has an identity")
    });
    if status.expired {
        output::emit(&format!(
            "Signed in as {identity}. The access token has expired; open Anarlog or run the login command again to refresh it."
        ));
    } else {
        output::emit(&format!("Signed in as {identity}."));
    }
    Ok(())
}

fn login_url(scheme: &str) -> Result<Url> {
    let mut url = Url::parse(WEB_APP_URL)
        .map_err(|error| Error::operation("build login URL", error.to_string()))?;
    url.set_path("/auth");
    url.query_pairs_mut()
        .append_pair("flow", "desktop")
        .append_pair("scheme", scheme);
    Ok(url)
}

fn read_callback_url() -> Result<String> {
    let input = if std::io::stdin().is_terminal() {
        rpassword::prompt_password("Callback URL (input hidden): ")
            .map_err(|error| Error::operation("read callback URL", error.to_string()))?
    } else {
        let mut input = String::new();
        std::io::stdin()
            .lock()
            .take((MAX_CALLBACK_BYTES + 1) as u64)
            .read_to_string(&mut input)
            .map_err(|error| Error::operation("read callback URL", error.to_string()))?;
        input
    };
    let input = input.trim();
    if input.is_empty() || input.len() > MAX_CALLBACK_BYTES {
        return Err(Error::operation(
            "read callback URL",
            "callback URL is missing or too large",
        ));
    }
    Ok(input.to_string())
}

struct CallbackTokens {
    access_token: String,
    refresh_token: String,
}

impl CallbackTokens {
    fn parse(value: &str, expected_scheme: &str) -> Result<Self> {
        let url = Url::parse(value)
            .map_err(|_| Error::operation("read callback URL", "callback URL is invalid"))?;
        if url.scheme() != expected_scheme
            || url.host_str() != Some("auth")
            || url.path() != "/callback"
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
        {
            return Err(Error::operation(
                "read callback URL",
                format!("expected {expected_scheme}://auth/callback"),
            ));
        }

        let mut access_token = None;
        let mut refresh_token = None;
        for (key, value) in url.query_pairs() {
            let slot = match key.as_ref() {
                "access_token" => &mut access_token,
                "refresh_token" => &mut refresh_token,
                _ => continue,
            };
            if slot.replace(value.into_owned()).is_some() {
                return Err(Error::operation(
                    "read callback URL",
                    "callback URL contains duplicate tokens",
                ));
            }
        }

        let access_token = access_token.filter(|token| !token.is_empty());
        let refresh_token = refresh_token.filter(|token| !token.is_empty());
        match (access_token, refresh_token) {
            (Some(access_token), Some(refresh_token)) => Ok(Self {
                access_token,
                refresh_token,
            }),
            _ => Err(Error::operation(
                "read callback URL",
                "callback URL does not contain a complete session",
            )),
        }
    }
}

async fn verify_and_build_session(tokens: CallbackTokens) -> Result<Session> {
    let claims = decode_token_claims(&tokens.access_token)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(format!("anarlog-cli/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| Error::operation("verify login", error.to_string()))?;
    let response = client
        .get(format!("{API_URL}/v1/cloud-api/settings"))
        .bearer_auth(&tokens.access_token)
        .send()
        .await
        .map_err(|error| Error::operation("verify login", error.to_string()))?;
    if !response.status().is_success() {
        let reason = if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            "Anarlog rejected the callback session".to_string()
        } else {
            format!(
                "Anarlog could not verify the callback session (HTTP {})",
                response.status().as_u16()
            )
        };
        return Err(Error::operation("verify login", reason));
    }

    let now = unix_timestamp();
    if claims.exp <= now {
        return Err(Error::operation(
            "verify login",
            "callback session has expired",
        ));
    }
    validate_issuer(&claims.iss)?;

    Ok(Session {
        access_token: tokens.access_token,
        refresh_token: Some(tokens.refresh_token),
        token_type: "bearer".to_string(),
        expires_in: Some(claims.exp - now),
        expires_at: Some(claims.exp),
        user: Some(SessionUser {
            id: claims.sub,
            aud: claims.aud.as_str().map(str::to_string),
            role: claims.role,
            email: claims.email,
            phone: claims.phone,
            email_confirmed_at: None,
            confirmed_at: None,
            recovery_sent_at: None,
            last_sign_in_at: None,
            app_metadata: claims
                .app_metadata
                .and_then(|value| serde_json::from_value::<AppMetadata>(value).ok()),
            user_metadata: claims
                .user_metadata
                .and_then(|value| serde_json::from_value::<UserMetadata>(value).ok()),
            identities: Vec::new(),
            created_at: None,
            updated_at: None,
            is_anonymous: claims.is_anonymous,
            extra: Map::new(),
        }),
        extra: Map::new(),
    })
}

#[derive(Deserialize)]
struct TokenClaims {
    sub: String,
    exp: u64,
    iss: String,
    #[serde(default)]
    aud: Value,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    phone: Option<String>,
    #[serde(default)]
    app_metadata: Option<Value>,
    #[serde(default)]
    user_metadata: Option<Value>,
    #[serde(default)]
    is_anonymous: Option<bool>,
}

fn decode_token_claims(token: &str) -> Result<TokenClaims> {
    let mut parts = token.split('.');
    let _header = parts.next();
    let payload = parts.next();
    let signature = parts.next();
    if payload.is_none() || signature.is_none() || parts.next().is_some() {
        return Err(Error::operation(
            "verify login",
            "callback session is invalid",
        ));
    }
    let payload = URL_SAFE_NO_PAD
        .decode(payload.expect("payload checked above"))
        .map_err(|_| Error::operation("verify login", "callback session is invalid"))?;
    let claims: TokenClaims = serde_json::from_slice(&payload)
        .map_err(|_| Error::operation("verify login", "callback session is invalid"))?;
    if claims.sub.trim().is_empty() {
        return Err(Error::operation(
            "verify login",
            "callback account identity is missing",
        ));
    }
    Ok(claims)
}

fn validate_issuer(issuer: &str) -> Result<Url> {
    let url = Url::parse(issuer)
        .map_err(|_| Error::operation("verify login", "callback issuer is invalid"))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || url.path() != "/auth/v1"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return Err(Error::operation(
            "verify login",
            "callback issuer is invalid",
        ));
    }
    Ok(url)
}

fn storage_key(access_token: &str) -> Result<String> {
    let issuer = validate_issuer(&decode_token_claims(access_token)?.iss)?;
    let project = issuer
        .host_str()
        .and_then(|host| host.split('.').next())
        .filter(|project| !project.is_empty())
        .ok_or_else(|| Error::operation("complete login", "callback issuer is invalid"))?;
    Ok(format!("sb-{project}-auth-token"))
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[derive(Serialize)]
struct AuthStatus {
    signed_in: bool,
    user_id: Option<String>,
    email: Option<String>,
    expires_at: Option<u64>,
    expired: bool,
    storage: Option<&'static str>,
    storage_path: String,
}

impl AuthStatus {
    fn signed_in(
        user_id: String,
        email: Option<String>,
        expires_at: Option<u64>,
        backend: Backend,
        store: &AuthStore,
    ) -> Self {
        Self {
            signed_in: true,
            user_id: Some(user_id),
            email,
            expires_at,
            expired: expires_at.is_none_or(|expires_at| expires_at <= unix_timestamp()),
            storage: Some(backend.as_str()),
            storage_path: store.path().display().to_string(),
        }
    }

    fn signed_out(store: &AuthStore) -> Self {
        Self {
            signed_in: false,
            user_id: None,
            email: None,
            expires_at: None,
            expired: false,
            storage: None,
            storage_path: store.path().display().to_string(),
        }
    }
}

#[derive(Clone, Copy)]
struct Environment {
    scheme: &'static str,
    bundle_id: &'static str,
}

impl Environment {
    fn current() -> Self {
        let executable = std::env::current_exe()
            .ok()
            .and_then(|path| {
                path.file_stem()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_default();
        if executable.contains("staging") {
            return Self {
                scheme: "anarlog-staging",
                bundle_id: "com.hyprnote.staging",
            };
        }
        if executable.contains("dev") {
            return Self {
                scheme: "anarlog-dev",
                bundle_id: "com.hyprnote.dev",
            };
        }
        Self {
            scheme: "anarlog",
            bundle_id: "com.hyprnote.stable",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(payload: Value) -> String {
        format!(
            "header.{}.signature",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap())
        )
    }

    #[test]
    fn builds_headless_browser_login_url() {
        assert_eq!(
            login_url("anarlog").unwrap().as_str(),
            "https://anarlog.so/auth?flow=desktop&scheme=anarlog"
        );
    }

    #[test]
    fn parses_desktop_callback_without_logging_tokens() {
        let callback = CallbackTokens::parse(
            "anarlog://auth/callback?access_token=access%2Etoken&refresh_token=refresh-token",
            "anarlog",
        )
        .unwrap();

        assert_eq!(callback.access_token, "access.token");
        assert_eq!(callback.refresh_token, "refresh-token");
    }

    #[test]
    fn rejects_wrong_or_incomplete_callback_urls() {
        assert!(
            CallbackTokens::parse(
                "https://auth/callback?access_token=access&refresh_token=refresh",
                "anarlog"
            )
            .is_err()
        );
        assert!(
            CallbackTokens::parse("anarlog://auth/callback?access_token=access", "anarlog")
                .is_err()
        );
        assert!(
            CallbackTokens::parse(
                "anarlog://auth/callback?access_token=one&access_token=two&refresh_token=refresh",
                "anarlog"
            )
            .is_err()
        );
    }

    #[test]
    fn derives_desktop_storage_key_from_validated_issuer_shape() {
        let access_token = token(serde_json::json!({
            "sub": "user-1",
            "exp": 4_000_000_000_u64,
            "iss": "https://project-ref.supabase.co/auth/v1"
        }));

        assert_eq!(
            storage_key(&access_token).unwrap(),
            "sb-project-ref-auth-token"
        );
    }

    #[test]
    fn rejects_non_https_or_malformed_issuers() {
        for issuer in [
            "http://project.supabase.co/auth/v1",
            "https://project.supabase.co/other",
            "https://user:secret@project.supabase.co/auth/v1",
            "https://project.supabase.co/auth/v1?key=value",
        ] {
            assert!(validate_issuer(issuer).is_err(), "accepted {issuer}");
        }
    }
}
