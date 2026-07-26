use crate::client::{NangoClient, append_query, check_response, parse_response};
use crate::common_derives;
use crate::connect_session::DataWrapper;

common_derives! {
    pub struct Integration {
        pub unique_key: String,
        pub display_name: String,
        pub provider: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub logo: Option<String>,
        pub created_at: String,
        pub updated_at: String,
    }
}

common_derives! {
    pub struct IntegrationFull {
        pub unique_key: String,
        pub display_name: String,
        pub provider: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub logo: Option<String>,
        pub created_at: String,
        pub updated_at: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub webhook_url: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub credentials: Option<IntegrationCredentials>,
    }
}

common_derives! {
    #[serde(tag = "type")]
    pub enum IntegrationCredentials {
        #[serde(rename = "OAUTH1")]
        OAuth1 {
            client_id: String,
            client_secret: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            scopes: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            webhook_secret: Option<String>,
        },
        #[serde(rename = "OAUTH2")]
        OAuth2 {
            client_id: String,
            client_secret: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            scopes: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            webhook_secret: Option<String>,
        },
        #[serde(rename = "TBA")]
        Tba {
            client_id: String,
            client_secret: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            scopes: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            webhook_secret: Option<String>,
        },
        #[serde(rename = "APP")]
        App {
            app_id: String,
            app_link: String,
            private_key: String,
        },
        #[serde(rename = "CUSTOM")]
        Custom {
            client_id: String,
            client_secret: String,
            app_id: String,
            app_link: String,
            private_key: String,
        },
    }
}

common_derives! {
    pub struct CreateIntegrationRequest {
        pub unique_key: String,
        pub provider: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub credentials: Option<IntegrationCredentials>,
    }
}

common_derives! {
    #[derive(Default)]
    pub struct UpdateIntegrationRequest {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub unique_key: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub credentials: Option<IntegrationCredentials>,
    }
}

impl NangoClient {
    pub async fn list_integrations(&self) -> Result<Vec<Integration>, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/integrations");

        let response = self.client.get(url).send().await?;
        let wrapper: DataWrapper<Vec<Integration>> = parse_response(response).await?;
        Ok(wrapper.data)
    }

    pub async fn get_integration(
        &self,
        unique_key: impl std::fmt::Display,
        include: &[&str],
    ) -> Result<IntegrationFull, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("/integrations/{}", unique_key));

        for item in include {
            append_query(&mut url, "include", item);
        }

        let response = self.client.get(url).send().await?;
        let wrapper: DataWrapper<IntegrationFull> = parse_response(response).await?;
        Ok(wrapper.data)
    }

    pub async fn create_integration(
        &self,
        req: CreateIntegrationRequest,
    ) -> Result<Vec<Integration>, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/integrations");

        let response = self.client.post(url).json(&req).send().await?;
        let wrapper: DataWrapper<Vec<Integration>> = parse_response(response).await?;
        Ok(wrapper.data)
    }

    pub async fn update_integration(
        &self,
        unique_key: impl std::fmt::Display,
        req: UpdateIntegrationRequest,
    ) -> Result<Integration, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("/integrations/{}", unique_key));

        let response = self.client.patch(url).json(&req).send().await?;
        let wrapper: DataWrapper<Integration> = parse_response(response).await?;
        Ok(wrapper.data)
    }

    pub async fn delete_integration(
        &self,
        unique_key: impl std::fmt::Display,
    ) -> Result<(), crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path(&format!("/integrations/{}", unique_key));

        let response = self.client.delete(url).send().await?;
        check_response(response).await?;
        Ok(())
    }
}
