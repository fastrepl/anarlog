use crate::client::{ExaClient, parse_response};
use crate::common_derives;
use crate::types::{Category, ContentsRequest, SearchResult, SearchType};

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct SearchRequest {
        pub query: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub r#type: Option<SearchType>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub category: Option<Category>,
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
    pub struct SearchResponse {
        pub results: Vec<SearchResult>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub autoprompt_string: Option<String>,
    }
}

impl ExaClient {
    pub async fn search(&self, req: SearchRequest) -> Result<SearchResponse, crate::Error> {
        let mut url = self.api_base.clone();
        url.set_path("/search");

        let response = self.client.post(url).json(&req).send().await?;
        parse_response(response).await
    }
}
