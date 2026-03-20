use crate::client::{ExaClient, parse_response};
use crate::common_derives;
use crate::types::{ContentsRequest, SearchResult};

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct FindSimilarRequest {
        pub url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub num_results: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub include_domains: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub exclude_domains: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub start_crawl_date: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub end_crawl_date: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub start_published_date: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub end_published_date: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub include_text: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub exclude_text: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub contents: Option<ContentsRequest>,
    }
}

crate::float_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct FindSimilarResponse {
        pub results: Vec<SearchResult>,
    }
}

impl ExaClient {
    pub async fn find_similar(
        &self,
        req: FindSimilarRequest,
    ) -> Result<FindSimilarResponse, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/findSimilar");

        let response = self.client.post(url).json(&req).send().await?;
        parse_response(response).await
    }
}
