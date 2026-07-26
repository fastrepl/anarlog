use crate::client::{NangoClient, parse_response};
use crate::common_derives;

common_derives! {
    pub struct TriggerActionRequest {
        pub action_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub input: Option<serde_json::Value>,
    }
}

common_derives! {
    pub struct TriggerActionAsyncResponse {
        #[serde(rename = "statusUrl")]
        pub status_url: String,
        pub id: String,
    }
}

impl NangoClient {
    pub async fn trigger_action(
        &self,
        connection_id: impl std::fmt::Display,
        provider_config_key: impl std::fmt::Display,
        req: TriggerActionRequest,
    ) -> Result<serde_json::Value, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/action/trigger");

        let response = self
            .client
            .post(url)
            .header("Connection-Id", connection_id.to_string())
            .header("Provider-Config-Key", provider_config_key.to_string())
            .json(&req)
            .send()
            .await?;
        parse_response(response).await
    }

    pub async fn trigger_action_async(
        &self,
        connection_id: impl std::fmt::Display,
        provider_config_key: impl std::fmt::Display,
        max_retries: Option<u32>,
        req: TriggerActionRequest,
    ) -> Result<TriggerActionAsyncResponse, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/action/trigger");

        let mut request = self
            .client
            .post(url)
            .header("Connection-Id", connection_id.to_string())
            .header("Provider-Config-Key", provider_config_key.to_string())
            .header("X-Async", "true");

        if let Some(retries) = max_retries {
            request = request.header("X-Max-Retries", retries.to_string());
        }

        let response = request.json(&req).send().await?;
        parse_response(response).await
    }
}
