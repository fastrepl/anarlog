use anlg_nango::OwnedNangoProxy;

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
        // https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/UserInfo
        "google-calendar" | "google-drive" | "google-meet" => {
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

        other => Err(format!("unsupported integration: {other}")),
    }
}
