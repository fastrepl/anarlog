use anlg_nango::OwnedNangoProxy;

#[derive(serde::Deserialize)]
struct GoogleCalendarIdentity {
    id: Option<String>,
    summary: Option<String>,
}

#[derive(serde::Deserialize)]
struct GoogleUserInfo {
    email: Option<String>,
    name: Option<String>,
}

#[derive(serde::Deserialize)]
struct OutlookMe {
    mail: Option<String>,
    #[serde(rename = "userPrincipalName")]
    user_principal_name: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

#[derive(serde::Deserialize)]
struct ZoomUser {
    email: Option<String>,
    display_name: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
}

#[derive(serde::Deserialize)]
struct WebexMe {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(default)]
    emails: Vec<String>,
}

#[derive(serde::Deserialize)]
struct FathomMeetings {
    #[serde(default)]
    items: Vec<FathomMeeting>,
}

#[derive(serde::Deserialize)]
struct FathomMeeting {
    recorded_by: Option<FathomUser>,
}

#[derive(serde::Deserialize)]
struct FathomUser {
    email: Option<String>,
    name: Option<String>,
}

#[derive(serde::Deserialize)]
struct NotionUser {
    name: Option<String>,
    person: Option<NotionPerson>,
    bot: Option<NotionBot>,
}

#[derive(serde::Deserialize)]
struct NotionPerson {
    email: Option<String>,
}

#[derive(serde::Deserialize)]
struct NotionBot {
    owner: Option<NotionBotOwner>,
}

#[derive(serde::Deserialize)]
struct NotionBotOwner {
    user: Option<NotionOwnerUser>,
}

#[derive(serde::Deserialize)]
struct NotionOwnerUser {
    person: Option<NotionPerson>,
    name: Option<String>,
}

pub(crate) async fn fetch_identity(
    nango: &anlg_nango::NangoClient,
    integration_id: &str,
    connection_id: &str,
) -> std::result::Result<(Option<String>, Option<String>), String> {
    let proxy = OwnedNangoProxy::new(nango, integration_id.to_string(), connection_id.to_string());

    match integration_id {
        // Calendar access does not include Google profile scopes. The primary
        // calendar ID is the connected account email and is available with the
        // existing calendar.readonly scope.
        "google-calendar" => {
            let resp = proxy
                .get("/calendar/v3/calendars/primary")
                .map_err(|e| e.to_string())?
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;

            let calendar: GoogleCalendarIdentity = resp.json().await.map_err(|e| e.to_string())?;
            Ok((calendar.id, calendar.summary))
        }

        // https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/UserInfo
        "google-drive" | "google-meet" => {
            let request = if integration_id == "google-meet" {
                proxy
                    .base_url_override("https://www.googleapis.com")
                    .get("/oauth2/v1/userinfo?alt=json")
            } else {
                proxy.get("/oauth2/v1/userinfo?alt=json")
            };
            let resp = request
                .map_err(|e| e.to_string())?
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;

            let me: GoogleUserInfo = resp.json().await.map_err(|e| e.to_string())?;
            Ok((me.email, me.name))
        }

        // https://learn.microsoft.com/en-us/graph/api/user-get
        "outlook" | "microsoft-teams" => {
            let resp = proxy
                .get("/v1.0/me?$select=mail,userPrincipalName,displayName")
                .map_err(|e| e.to_string())?
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;

            let me: OutlookMe = resp.json().await.map_err(|e| e.to_string())?;
            Ok((me.mail.or(me.user_principal_name), me.display_name))
        }

        "zoom" => {
            let resp = proxy
                .get("/users/me")
                .map_err(|e| e.to_string())?
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;

            let me: ZoomUser = resp.json().await.map_err(|e| e.to_string())?;
            let name = me
                .display_name
                .or_else(|| match (me.first_name, me.last_name) {
                    (Some(first), Some(last)) => Some(format!("{first} {last}").trim().to_string()),
                    (Some(first), None) => Some(first),
                    (None, Some(last)) => Some(last),
                    (None, None) => None,
                });
            Ok((me.email, name))
        }

        "webex" => {
            let resp = proxy
                .get("/v1/people/me")
                .map_err(|e| e.to_string())?
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;
            let me: WebexMe = resp.json().await.map_err(|e| e.to_string())?;
            Ok((
                me.emails.into_iter().find(|email| !email.is_empty()),
                me.display_name,
            ))
        }

        "fathom" => {
            let resp = proxy
                .get("/external/v1/meetings?limit=1")
                .map_err(|e| e.to_string())?
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;
            let page: FathomMeetings = resp.json().await.map_err(|e| e.to_string())?;
            let recorded_by = page
                .items
                .into_iter()
                .next()
                .and_then(|item| item.recorded_by);
            Ok((
                recorded_by.as_ref().and_then(|user| user.email.clone()),
                recorded_by.and_then(|user| user.name),
            ))
        }

        "notion" => {
            let resp = proxy
                .get("/v1/users/me")
                .map_err(|e| e.to_string())?
                .header("Notion-Version", "2022-06-28")
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;
            let me: NotionUser = resp.json().await.map_err(|e| e.to_string())?;
            let email = me.person.and_then(|person| person.email).or_else(|| {
                me.bot
                    .as_ref()
                    .and_then(|bot| bot.owner.as_ref())
                    .and_then(|owner| owner.user.as_ref())
                    .and_then(|user| user.person.as_ref())
                    .and_then(|person| person.email.clone())
            });
            let name = me.name.or_else(|| {
                me.bot
                    .and_then(|bot| bot.owner)
                    .and_then(|owner| owner.user)
                    .and_then(|user| user.name)
            });
            Ok((email, name))
        }

        "slack" => {
            let resp = proxy
                .post(
                    "/auth.test",
                    Vec::new(),
                    "application/x-www-form-urlencoded",
                )
                .map_err(|e| e.to_string())?
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;
            let auth: SlackAuthTest = resp.json().await.map_err(|e| e.to_string())?;
            if auth.ok == Some(false) {
                return Err(auth
                    .error
                    .unwrap_or_else(|| "slack auth.test failed".to_string()));
            }
            Ok((nonempty(auth.team), nonempty(auth.user)))
        }

        "linear" => {
            let body = serde_json::to_vec(&serde_json::json!({
                "query": "query OrganizationName { organization { name } }"
            }))
            .map_err(|e| e.to_string())?;
            let resp = proxy
                .post("/graphql", body, "application/json")
                .map_err(|e| e.to_string())?
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;
            let payload: LinearOrganizationResponse =
                resp.json().await.map_err(|e| e.to_string())?;
            Ok((
                payload
                    .data
                    .and_then(|data| data.organization)
                    .and_then(|organization| nonempty(organization.name)),
                None,
            ))
        }

        "github" => {
            let resp = proxy
                .get("/user")
                .map_err(|e| e.to_string())?
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?;
            let me: GithubUser = resp.json().await.map_err(|e| e.to_string())?;
            Ok((
                nonempty(me.email).or_else(|| nonempty(me.login)),
                nonempty(me.name),
            ))
        }

        other => Err(format!("unsupported integration: {other}")),
    }
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub(crate) fn account_identity_from_tags(
    tags: Option<&std::collections::HashMap<String, String>>,
) -> Option<String> {
    nonempty(tags.and_then(|tags| tags.get("account_identity").cloned()))
}

pub(crate) async fn fetch_and_store_account_identity(
    nango: &anlg_nango::NangoClient,
    integration_id: &str,
    connection_id: &str,
) -> Option<String> {
    let identity = match fetch_identity(nango, integration_id, connection_id).await {
        Ok((identity, _display_name)) => identity?,
        Err(e) => {
            tracing::warn!(
                anarlog.connection.id = %connection_id,
                anarlog.integration.id = %integration_id,
                error = %e,
                "failed to fetch identity for account_identity tag"
            );
            return None;
        }
    };

    let mut tags = match nango.get_connection(connection_id, integration_id).await {
        Ok(connection) => connection.tags.unwrap_or_default(),
        Err(e) => {
            tracing::warn!(
                anarlog.connection.id = %connection_id,
                anarlog.integration.id = %integration_id,
                error = %e,
                "failed to fetch connection before patching account_identity tag"
            );
            return Some(identity);
        }
    };
    tags.insert("account_identity".to_string(), identity.clone());

    let req = anlg_nango::PatchConnectionRequest {
        end_user: None,
        tags: Some(tags),
    };

    match nango
        .patch_connection(connection_id, integration_id, req)
        .await
    {
        Ok(()) => {
            tracing::info!(
                anarlog.connection.id = %connection_id,
                anarlog.integration.id = %integration_id,
                account_identity = %identity,
                "account_identity tag set"
            );
        }
        Err(e) => {
            tracing::warn!(
                anarlog.connection.id = %connection_id,
                anarlog.integration.id = %integration_id,
                error = %e,
                "failed to patch account_identity tag"
            );
        }
    }

    Some(identity)
}

pub(crate) fn spawn_identity_task(
    nango: anlg_nango::NangoClient,
    integration_id: String,
    connection_id: String,
) {
    tokio::spawn(async move {
        let _ = fetch_and_store_account_identity(&nango, &integration_id, &connection_id).await;
    });
}

#[derive(serde::Deserialize)]
struct SlackAuthTest {
    ok: Option<bool>,
    team: Option<String>,
    user: Option<String>,
    error: Option<String>,
}

#[derive(serde::Deserialize)]
struct LinearOrganizationResponse {
    data: Option<LinearOrganizationData>,
}

#[derive(serde::Deserialize)]
struct LinearOrganizationData {
    organization: Option<LinearOrganization>,
}

#[derive(serde::Deserialize)]
struct LinearOrganization {
    name: Option<String>,
}

#[derive(serde::Deserialize)]
struct GithubUser {
    login: Option<String>,
    email: Option<String>,
    name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NangoConfig;
    use crate::state::AppState;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn nango_client(nango_mock: &MockServer) -> anlg_nango::NangoClient {
        let supabase = MockServer::start().await;
        AppState::new(NangoConfig::for_test(&nango_mock.uri(), &supabase.uri())).nango
    }

    #[test]
    fn account_identity_from_tags_trims_and_ignores_empty() {
        let mut tags = std::collections::HashMap::new();
        tags.insert(
            "account_identity".to_string(),
            "  john@fastrepl.com  ".to_string(),
        );
        assert_eq!(
            account_identity_from_tags(Some(&tags)).as_deref(),
            Some("john@fastrepl.com")
        );

        tags.insert("account_identity".to_string(), "   ".to_string());
        assert_eq!(account_identity_from_tags(Some(&tags)), None);
        assert_eq!(account_identity_from_tags(None), None);
    }

    #[tokio::test]
    async fn google_calendar_identity_uses_primary_calendar_email() {
        let nango_mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/proxy/calendar/v3/calendars/primary"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "john@fastrepl.com",
                "summary": "John Jeong"
            })))
            .mount(&nango_mock)
            .await;

        let nango = nango_client(&nango_mock).await;
        let (identity, display_name) = fetch_identity(&nango, "google-calendar", "conn-google")
            .await
            .unwrap();
        assert_eq!(identity.as_deref(), Some("john@fastrepl.com"));
        assert_eq!(display_name.as_deref(), Some("John Jeong"));
    }

    #[tokio::test]
    async fn slack_identity_uses_workspace_name() {
        let nango_mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/proxy/auth.test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "ok": true,
                "team": "Fastrepl",
                "user": "john"
            })))
            .mount(&nango_mock)
            .await;

        let nango = nango_client(&nango_mock).await;
        let (identity, user) = fetch_identity(&nango, "slack", "conn-slack").await.unwrap();
        assert_eq!(identity.as_deref(), Some("Fastrepl"));
        assert_eq!(user.as_deref(), Some("john"));
    }

    #[tokio::test]
    async fn linear_identity_uses_organization_name() {
        let nango_mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/proxy/graphql"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "organization": { "name": "Anarlog" } }
            })))
            .mount(&nango_mock)
            .await;

        let nango = nango_client(&nango_mock).await;
        let (identity, display_name) = fetch_identity(&nango, "linear", "conn-linear")
            .await
            .unwrap();
        assert_eq!(identity.as_deref(), Some("Anarlog"));
        assert_eq!(display_name, None);
    }
}
