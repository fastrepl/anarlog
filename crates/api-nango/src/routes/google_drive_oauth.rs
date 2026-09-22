use super::google_drive::{DriveConnectionRequest, DriveError, DriveFolder, folder, proxy};
use crate::{GoogleDrive, NangoIntegrationId, state::AppState};
use anlg_api_auth::AuthContext;
use anlg_nango::{CreateConnectionRequest, EndUser, IntegrationCredentials};
use axum::{
    Extension, Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const SCOPE: &str = "https://www.googleapis.com/auth/drive.file";

fn invalid(message: &'static str) -> DriveError {
    DriveError::Request(StatusCode::BAD_REQUEST, message)
}

#[derive(Serialize, Deserialize)]
struct SelectionState {
    user_id: String,
    connection_id: String,
    redirect_uri: String,
    nonce: String,
    expires_at: i64,
}

fn mac(secret: &str, value: &str) -> Hmac<Sha256> {
    let mut result =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    result.update(b"anarlog-drive-selection-v1:");
    result.update(value.as_bytes());
    result
}

fn encode_state(secret: &str, state: &SelectionState) -> String {
    let body = URL_SAFE_NO_PAD.encode(serde_json::to_vec(state).expect("serializable state"));
    let signature = URL_SAFE_NO_PAD.encode(mac(secret, &body).finalize().into_bytes());
    format!("{body}.{signature}")
}

fn decode_state(
    secret: &str,
    value: &str,
    user: &str,
    now: i64,
) -> Result<SelectionState, DriveError> {
    if value.len() > 8192 {
        return Err(invalid("Invalid folder selection state"));
    }
    let (body, signature) = value
        .split_once('.')
        .ok_or_else(|| invalid("Invalid folder selection state"))?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| invalid("Invalid folder selection state"))?;
    mac(secret, body)
        .verify_slice(&signature)
        .map_err(|_| invalid("Invalid folder selection state"))?;
    let state: SelectionState = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(body)
            .map_err(|_| invalid("Invalid folder selection state"))?,
    )
    .map_err(|_| invalid("Invalid folder selection state"))?;
    if state.user_id != user || state.expires_at <= now || state.expires_at > now + 600 {
        return Err(invalid(
            "Folder selection expired or belongs to another user. Try again.",
        ));
    }
    Ok(state)
}

fn verifier(secret: &str, state: &str) -> String {
    URL_SAFE_NO_PAD.encode(
        mac(secret, &format!("pkce:{state}"))
            .finalize()
            .into_bytes(),
    )
}

async fn credentials(state: &AppState) -> Result<(String, String), DriveError> {
    let integration = state
        .nango
        .get_integration(GoogleDrive::ID, &["credentials"])
        .await?;
    match integration.credentials {
        Some(IntegrationCredentials::OAuth2 {
            client_id,
            client_secret,
            ..
        }) if !client_id.is_empty() && !client_secret.is_empty() => Ok((client_id, client_secret)),
        _ => Err(invalid("Configure a custom Google OAuth client in Nango.")),
    }
}

fn authorization_url(client_id: &str, redirect_uri: &str, state: &str, verifier: &str) -> String {
    let mut url = reqwest::Url::parse("https://accounts.google.com/o/oauth2/v2/auth").unwrap();
    url.query_pairs_mut().extend_pairs([
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
        ("response_type", "code"),
        ("scope", SCOPE),
        ("access_type", "offline"),
        ("prompt", "consent"),
        ("include_granted_scopes", "false"),
        ("trigger_onepick", "true"),
        ("allow_folder_selection", "true"),
        ("allow_multiple", "false"),
        ("mimetypes", "application/vnd.google-apps.folder"),
        ("state", state),
        ("code_challenge_method", "S256"),
        (
            "code_challenge",
            &URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
        ),
    ]);
    url.to_string()
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct PickerStart {
    pub authorization_url: String,
    pub state: String,
}

#[utoipa::path(post, path = "/google-drive/picker-start", operation_id = "google_drive_picker_start", request_body = DriveConnectionRequest, responses((status = 200, body = PickerStart)), tag = "nango")]
pub async fn start(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<DriveConnectionRequest>,
) -> Result<Response, DriveError> {
    proxy(&state, &auth, &req.connection_id).await?;
    let redirect_uri = std::env::var("GOOGLE_DRIVE_PICKER_REDIRECT_URI")
        .map_err(|_| invalid("Google Drive callback is not configured."))?;
    let redirect = reqwest::Url::parse(&redirect_uri)
        .map_err(|_| invalid("Invalid Google Drive callback configuration"))?;
    if redirect.scheme() != "https"
        && !(redirect.scheme() == "http"
            && matches!(redirect.host_str(), Some("localhost" | "127.0.0.1")))
    {
        return Err(invalid("Google Drive callback must use HTTPS"));
    }
    let (client_id, _) = credentials(&state).await?;
    let secret = &state.config.nango.nango_api_key;
    let signed = encode_state(
        secret,
        &SelectionState {
            user_id: auth.claims.sub,
            connection_id: req.connection_id,
            redirect_uri: redirect_uri.clone(),
            nonce: uuid::Uuid::new_v4().to_string(),
            expires_at: chrono::Utc::now().timestamp() + 600,
        },
    );
    let authorization_url = authorization_url(
        &client_id,
        &redirect_uri,
        &signed,
        &verifier(secret, &signed),
    );
    Ok((
        [("Cache-Control", "no-store")],
        Json(PickerStart {
            authorization_url,
            state: signed,
        }),
    )
        .into_response())
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PickerComplete {
    pub state: String,
    pub code: String,
    pub folder_id: String,
}

async fn permission_id(
    client: &reqwest::Client,
    token: &str,
    about_url: &str,
) -> Result<String, DriveError> {
    let response = client.get(about_url).bearer_auth(token).send().await?;
    if !response.status().is_success() {
        return Err(invalid(
            "Could not verify the Google account. Reconnect and try again.",
        ));
    }
    let value: Value = response.json().await?;
    value["user"]["permissionId"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid("Could not verify the Google account"))
}

#[utoipa::path(post, path = "/google-drive/picker-complete", operation_id = "google_drive_picker_complete", request_body = PickerComplete, responses((status = 200, body = DriveFolder)), tag = "nango")]
pub async fn complete(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<PickerComplete>,
) -> Result<Response, DriveError> {
    complete_with_urls(
        state,
        auth,
        req,
        "https://oauth2.googleapis.com/token",
        "https://www.googleapis.com/drive/v3/about?fields=user(permissionId)",
    )
    .await
}

async fn complete_with_urls(
    state: AppState,
    auth: AuthContext,
    req: PickerComplete,
    token_url: &str,
    about_url: &str,
) -> Result<Response, DriveError> {
    let secret = &state.config.nango.nango_api_key;
    let selection = decode_state(
        secret,
        &req.state,
        &auth.claims.sub,
        chrono::Utc::now().timestamp(),
    )?;
    if req.code.is_empty()
        || req.code.len() > 8192
        || req.folder_id.is_empty()
        || req.folder_id.len() > 256
        || !req
            .folder_id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(invalid("Invalid Google folder selection"));
    }
    proxy(&state, &auth, &selection.connection_id).await?;
    let existing = state
        .nango
        .get_connection(&selection.connection_id, GoogleDrive::ID)
        .await?;
    let (client_id, client_secret) = credentials(&state).await?;
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let response = http
        .post(token_url)
        .form(&[
            ("code", req.code.as_str()),
            ("client_id", &client_id),
            ("client_secret", &client_secret),
            ("redirect_uri", &selection.redirect_uri),
            ("grant_type", "authorization_code"),
            ("code_verifier", &verifier(secret, &req.state)),
        ])
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(invalid(
            "Google authorization expired or was already used. Choose a folder again.",
        ));
    }
    let tokens: Value = response.json().await?;
    if tokens["scope"]
        .as_str()
        .unwrap_or("")
        .split_whitespace()
        .collect::<Vec<_>>()
        != [SCOPE]
    {
        return Err(invalid("Google must grant only selected-file access."));
    }
    let access_token = tokens["access_token"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid("Google did not return an access token"))?;
    let old_token = existing.credentials["access_token"]
        .as_str()
        .ok_or_else(|| invalid("Reconnect Google Drive first"))?;
    if permission_id(&http, access_token, about_url).await?
        != permission_id(&http, old_token, about_url).await?
    {
        return Err(invalid(
            "Choose a folder using the same Google account as the connected account.",
        ));
    }
    let refresh_token = tokens["refresh_token"]
        .as_str()
        .or_else(|| existing.credentials["refresh_token"].as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| invalid("Reconnect Google Drive to allow automatic uploads."))?;
    state.nango.create_connection(CreateConnectionRequest {
        provider_config_key: GoogleDrive::ID.to_owned(), connection_id: Some(selection.connection_id.clone()),
        credentials: json!({"type":"OAUTH2", "access_token":access_token, "refresh_token":refresh_token, "expires_at":(chrono::Utc::now() + chrono::Duration::seconds(tokens["expires_in"].as_i64().unwrap_or(3600).clamp(1, 86400))).to_rfc3339()}),
        metadata: existing.metadata.as_object().cloned().map(Value::Object), connection_config: existing.connection_config.as_object().cloned().map(Value::Object),
        end_user: Some(EndUser { id: auth.claims.sub.clone(), email: existing.end_user.as_ref().and_then(|u| u.email.clone()), display_name: existing.end_user.and_then(|u| u.display_name), tags: existing.tags }),
    }).await.map_err(|error| {
        let (kind, status) = match &error {
            anlg_nango::Error::Api(status, _) => ("provider_response", Some(*status)),
            anlg_nango::Error::Request(error) if error.is_decode() => ("response_decode", None),
            anlg_nango::Error::Request(error) if error.is_timeout() => ("timeout", None),
            _ => ("request", None),
        };
        tracing::error!(stage = "drive_connection_import", error.type = kind, upstream_status = status, "google_drive_selection_failed");
        DriveError::from(error)
    })?;
    let proxy = proxy(&state, &auth, &selection.connection_id).await?;
    Ok((
        [("Cache-Control", "no-store")],
        Json(folder(&proxy, &req.folder_id).await?),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_binds_owner_connection_expiry_and_pkce() {
        let state = SelectionState {
            user_id: "user".into(),
            connection_id: "connection".into(),
            redirect_uri: "http://localhost:3000/app/google-drive-picker".into(),
            nonce: "nonce".into(),
            expires_at: 1500,
        };
        let signed = encode_state("secret", &state);
        assert_eq!(
            decode_state("secret", &signed, "user", 1000)
                .unwrap()
                .connection_id,
            "connection"
        );
        assert!(decode_state("secret", &signed, "other", 1000).is_err());
        assert!(decode_state("secret", &signed, "user", 1500).is_err());
        assert!(decode_state("other", &signed, "user", 1000).is_err());
        assert!(decode_state("secret", &(signed.clone() + "x"), "user", 1000).is_err());
        assert_ne!(verifier("secret", &signed), verifier("other", &signed));
        let url = reqwest::Url::parse(&authorization_url(
            "client",
            &state.redirect_uri,
            &signed,
            &verifier("secret", &signed),
        ))
        .unwrap();
        let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
        assert_eq!(params["scope"], SCOPE);
        assert_eq!(params["trigger_onepick"], "true");
        assert_eq!(params["allow_folder_selection"], "true");
        assert_eq!(params["code_challenge_method"], "S256");
        assert!(!url.as_str().contains("access_token"));
    }
    #[tokio::test]
    async fn completion_imports_only_matching_account_and_narrow_scope() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{header, method, path},
        };
        for (scope, matching_account, should_succeed, metadata) in [
            (SCOPE, true, true, json!({"preserve":true})),
            (SCOPE, true, true, Value::Null),
            (SCOPE, false, false, Value::Null),
            (
                "https://www.googleapis.com/auth/drive",
                true,
                false,
                Value::Null,
            ),
        ] {
            let server = MockServer::start().await;
            let connection = json!({"id":1,"connection_id":"connection","provider_config_key":"google-drive","provider":"google-drive","errors":[],"metadata":metadata,"connection_config":{},"created_at":"","updated_at":"","last_fetched_at":"","credentials":{"access_token":"old-token","refresh_token":"old-refresh"}});
            Mock::given(method("GET"))
                .and(path("/rest/v1/nango_connections"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(json!([{"status":"connected"}])),
                )
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .and(path("/connections/connection"))
                .respond_with(ResponseTemplate::new(200).set_body_json(connection.clone()))
                .mount(&server)
                .await;
            Mock::given(method("GET")).and(path("/integrations/google-drive")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"data":{"unique_key":"google-drive","display_name":"Google Drive","provider":"google-drive","created_at":"","updated_at":"","credentials":{"type":"OAUTH2","client_id":"client","client_secret":"client-secret"}}}))).mount(&server).await;
            Mock::given(method("POST")).and(path("/token")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"access_token":"new-token","refresh_token":"new-refresh","scope":scope,"expires_in":3600}))).mount(&server).await;
            for (token, id) in [
                ("old-token", "account-1"),
                (
                    "new-token",
                    if matching_account {
                        "account-1"
                    } else {
                        "account-2"
                    },
                ),
            ] {
                Mock::given(method("GET"))
                    .and(path("/about"))
                    .and(header("Authorization", format!("Bearer {token}")))
                    .respond_with(
                        ResponseTemplate::new(200)
                            .set_body_json(json!({"user":{"permissionId":id}})),
                    )
                    .mount(&server)
                    .await;
            }
            Mock::given(method("POST"))
                .and(path("/connections"))
                .respond_with(ResponseTemplate::new(200).set_body_json(connection))
                .expect(if should_succeed { 1 } else { 0 })
                .mount(&server)
                .await;
            Mock::given(method("GET")).and(path("/proxy/drive/v3/files/folder")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"folder","name":"회의 노트","mimeType":"application/vnd.google-apps.folder","capabilities":{"canAddChildren":true},"driveId":"shared-drive"}))).mount(&server).await;
            let config = crate::NangoConfig::for_test(&server.uri(), &server.uri());
            let signed = encode_state(
                &config.nango.nango_api_key,
                &SelectionState {
                    user_id: "user".into(),
                    connection_id: "connection".into(),
                    redirect_uri: "http://localhost:3000/app/google-drive-picker".into(),
                    nonce: "nonce".into(),
                    expires_at: chrono::Utc::now().timestamp() + 600,
                },
            );
            let auth = AuthContext {
                token: "user-token".into(),
                claims: serde_json::from_value(json!({"sub":"user"})).unwrap(),
            };
            let result = complete_with_urls(
                AppState::new(config),
                auth,
                PickerComplete {
                    state: signed,
                    code: "code".into(),
                    folder_id: "folder".into(),
                },
                &format!("{}/token", server.uri()),
                &format!("{}/about", server.uri()),
            )
            .await;
            assert_eq!(result.is_ok(), should_succeed);
            if should_succeed {
                let response = result.unwrap();
                assert_eq!(response.headers()["cache-control"], "no-store");
                let requests = server.received_requests().await.unwrap();
                let import = requests
                    .iter()
                    .find(|r| r.method == "POST" && r.url.path() == "/connections")
                    .unwrap();
                let body: Value = serde_json::from_slice(&import.body).unwrap();
                assert_eq!(body["connection_id"], "connection");
                assert_eq!(body["credentials"]["refresh_token"], "new-refresh");
                assert!(body["credentials"]["expires_at"].is_string());
                if metadata.is_null() {
                    assert!(
                        body.get("metadata").is_none(),
                        "Nango rejects explicit null metadata"
                    );
                } else {
                    assert_eq!(body["metadata"], metadata);
                }
                assert!(body["credentials"].get("scope").is_none());
                let bytes = axum::body::to_bytes(response.into_body(), 4096)
                    .await
                    .unwrap();
                assert!(!String::from_utf8_lossy(&bytes).contains("token"));
            }
        }
    }
}
