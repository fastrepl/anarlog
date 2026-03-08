use std::collections::HashMap;

use crate::client::NangoClient;
use crate::common_derives;

common_derives! {
    pub struct CreateConnectSessionRequest {
        pub end_user: EndUser,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub organization: Option<Organization>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub allowed_integrations: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub integrations_config_defaults: Option<HashMap<String, IntegrationConfigDefault>>,
    }
}

common_derives! {
    pub struct EndUser {
        pub id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub email: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub tags: Option<HashMap<String, String>>,
    }
}

common_derives! {
    pub struct Organization {
        pub id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
    }
}

common_derives! {
    pub struct IntegrationConfigDefault {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub user_scopes: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub connection_config: Option<ConnectionConfigOverride>,
    }
}

common_derives! {
    pub struct ConnectionConfigOverride {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub oauth_scopes_override: Option<String>,
    }
}

common_derives! {
    pub struct ConnectSession {
        pub token: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub connect_link: Option<String>,
        pub expires_at: String,
    }
}

common_derives! {
    pub struct ReconnectSessionRequest {
        pub connection_id: String,
        pub integration_id: String,
    }
}

#[derive(serde::Deserialize)]
pub(crate) struct DataWrapper<T> {
    pub data: T,
}

impl NangoClient {
    pub async fn create_connect_session(
        &self,
        req: CreateConnectSessionRequest,
    ) -> Result<ConnectSession, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/connect/sessions");

        let response = self.client.post(url).json(&req).send().await?;
        let wrapper: DataWrapper<ConnectSession> = crate::client::parse_response(response).await?;
        Ok(wrapper.data)
    }

    pub async fn reconnect_session(
        &self,
        req: ReconnectSessionRequest,
    ) -> Result<ConnectSession, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/connect/sessions/reconnect");

        let response = self.client.post(url).json(&req).send().await?;
        let wrapper: DataWrapper<ConnectSession> = crate::client::parse_response(response).await?;
        Ok(wrapper.data)
    }

    pub async fn get_connect_session(&self) -> Result<ConnectSession, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/connect/session");

        let response = self.client.get(url).send().await?;
        let wrapper: DataWrapper<ConnectSession> = crate::client::parse_response(response).await?;
        Ok(wrapper.data)
    }

    pub async fn delete_connect_session(&self) -> Result<(), crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/connect/session");

        let response = self.client.delete(url).send().await?;
        crate::client::check_response(response).await?;
        Ok(())
    }
}
