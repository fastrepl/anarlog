use hypr_nango::OwnedNangoProxy;

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

pub(crate) async fn fetch_identity(
    nango: &hypr_nango::NangoClient,
    integration_id: &str,
    connection_id: &str,
) -> std::result::Result<(Option<String>, Option<String>), String> {
    let proxy = OwnedNangoProxy::new(nango, integration_id.to_string(), connection_id.to_string());

    match integration_id {
        // https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/UserInfo
        "google-calendar" | "google-drive" => {
            let resp = proxy
                .get("/oauth2/v1/userinfo?alt=json")
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
        "outlook" => {
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

        other => Err(format!("unsupported integration: {other}")),
    }
}
