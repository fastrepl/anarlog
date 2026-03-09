use crate::client::{NangoClient, append_query, check_response, parse_response};
use crate::common_derives;
use crate::connect_session::EndUser;

common_derives! {
    pub struct ConnectionError {
        pub r#type: String,
        pub log_id: String,
    }
}

common_derives! {
    pub struct ConnectionEndUser {
        pub id: String,
        pub email: Option<String>,
        pub display_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub tags: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub organization: Option<ConnectionEndUserOrganization>,
    }
}

common_derives! {
    pub struct ConnectionEndUserOrganization {
        pub id: String,
        pub display_name: Option<String>,
    }
}

common_derives! {
    #[derive(Default)]
    pub struct ListConnectionsParams {
        #[serde(skip)]
        pub connection_id: Option<String>,
        #[serde(skip)]
        pub search: Option<String>,
        #[serde(skip)]
        pub end_user_id: Option<String>,
        #[serde(skip)]
        pub end_user_organization_id: Option<String>,
        #[serde(skip)]
        pub limit: Option<i32>,
        #[serde(skip)]
        pub page: Option<i32>,
    }
}

common_derives! {
    pub struct ConnectionListItem {
        pub id: i64,
        pub connection_id: String,
        pub provider: String,
        pub provider_config_key: String,
        pub created: String,
        pub metadata: Option<serde_json::Value>,
        pub errors: Vec<ConnectionError>,
        pub end_user: Option<ConnectionEndUser>,
    }
}

common_derives! {
    pub struct Connection {
        pub id: i64,
        pub connection_id: String,
        pub provider_config_key: String,
        pub provider: String,
        pub errors: Vec<ConnectionError>,
        pub end_user: Option<ConnectionEndUser>,
        pub metadata: serde_json::Value,
        pub connection_config: serde_json::Value,
        pub created_at: String,
        pub updated_at: String,
        pub last_fetched_at: String,
        pub credentials: serde_json::Value,
    }
}

common_derives! {
    pub struct CreateConnectionRequest {
        pub provider_config_key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub connection_id: Option<String>,
        pub credentials: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub metadata: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub connection_config: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub end_user: Option<EndUser>,
    }
}

common_derives! {
    pub struct PatchConnectionRequest {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub end_user: Option<EndUser>,
    }
}

common_derives! {
    pub struct MetadataRequest {
        pub connection_id: serde_json::Value,
        pub provider_config_key: String,
        pub metadata: serde_json::Value,
    }
}

common_derives! {
    pub struct MetadataResponse {
        pub connection_id: serde_json::Value,
        pub provider_config_key: String,
        pub metadata: serde_json::Value,
    }
}

#[derive(serde::Deserialize)]
struct ConnectionsWrapper {
    connections: Vec<ConnectionListItem>,
}

impl NangoClient {
    pub async fn create_connection(
        &self,
        req: CreateConnectionRequest,
    ) -> Result<Connection, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/connections");

        let response = self.client.post(url).json(&req).send().await?;
        parse_response(response).await
    }

    pub async fn list_connections(
        &self,
        params: ListConnectionsParams,
    ) -> Result<Vec<ConnectionListItem>, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/connections");

        if let Some(v) = &params.connection_id {
            append_query(&mut url, "connectionId", v);
        }
        if let Some(v) = &params.search {
            append_query(&mut url, "search", v);
        }
        if let Some(v) = &params.end_user_id {
            append_query(&mut url, "endUserId", v);
        }
        if let Some(v) = &params.end_user_organization_id {
            append_query(&mut url, "endUserOrganizationId", v);
        }
        if let Some(v) = params.limit {
            append_query(&mut url, "limit", &v.to_string());
        }
        if let Some(v) = params.page {
            append_query(&mut url, "page", &v.to_string());
        }

        let response = self.client.get(url).send().await?;
        let wrapper: ConnectionsWrapper = parse_response(response).await?;
        Ok(wrapper.connections)
    }

    pub async fn get_connection(
        &self,
        connection_id: impl std::fmt::Display,
        provider_config_key: impl std::fmt::Display,
    ) -> Result<Connection, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("/connections/{}", connection_id));
        append_query(
            &mut url,
            "provider_config_key",
            &provider_config_key.to_string(),
        );

        let response = self.client.get(url).send().await?;
        parse_response(response).await
    }

    pub async fn patch_connection(
        &self,
        connection_id: impl std::fmt::Display,
        provider_config_key: impl std::fmt::Display,
        req: PatchConnectionRequest,
    ) -> Result<(), crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("/connections/{}", connection_id));
        append_query(
            &mut url,
            "provider_config_key",
            &provider_config_key.to_string(),
        );

        let response = self.client.patch(url).json(&req).send().await?;
        check_response(response).await?;
        Ok(())
    }

    pub async fn set_metadata(
        &self,
        req: MetadataRequest,
    ) -> Result<MetadataResponse, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/connections/metadata");

        let response = self.client.post(url).json(&req).send().await?;
        parse_response(response).await
    }

    pub async fn update_metadata(
        &self,
        req: MetadataRequest,
    ) -> Result<MetadataResponse, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/connections/metadata");

        let response = self.client.patch(url).json(&req).send().await?;
        parse_response(response).await
    }

    pub async fn delete_connection(
        &self,
        connection_id: impl std::fmt::Display,
        provider_config_key: impl std::fmt::Display,
    ) -> Result<(), crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("/connections/{}", connection_id));
        append_query(
            &mut url,
            "provider_config_key",
            &provider_config_key.to_string(),
        );

        let response = self.client.delete(url).send().await?;
        check_response(response).await?;
        Ok(())
    }
}
