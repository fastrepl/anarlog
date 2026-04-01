use crate::client::{ExaClient, parse_response};
use crate::common_derives;
use crate::types::SearchResult;

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct GetContentsRequest {
        pub urls: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub text: Option<crate::types::TextRequest>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub highlights: Option<crate::types::HighlightsRequest>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub summary: Option<crate::types::SummaryRequest>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub livecrawl: Option<crate::types::Livecrawl>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub livecrawl_timeout: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub max_age_hours: Option<u32>,
    }
}

crate::float_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct GetContentsResponse {
        pub results: Vec<SearchResult>,
    }
}

impl ExaClient {
    pub async fn get_contents(
        &self,
        req: GetContentsRequest,
    ) -> Result<GetContentsResponse, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/contents");

        let response = self.client.post(url).json(&req).send().await?;
        parse_response(response).await
    }
}
