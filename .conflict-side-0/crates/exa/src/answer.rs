use crate::client::{ExaClient, parse_response};
use crate::common_derives;
use crate::types::AnswerCitation;

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct AnswerRequest {
        pub query: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub text: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub output_schema: Option<serde_json::Value>,
    }
}

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct AnswerResponse {
        pub answer: serde_json::Value,
        pub citations: Vec<AnswerCitation>,
    }
}

impl ExaClient {
    pub async fn answer(&self, req: AnswerRequest) -> Result<AnswerResponse, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/answer");

        let response = self.client.post(url).json(&req).send().await?;
        parse_response(response).await
    }
}
