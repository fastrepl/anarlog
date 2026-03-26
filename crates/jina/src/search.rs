use crate::client::{JinaClient, check_response};
use crate::common_derives;
use crate::types::{SearchEngine, SearchResponseEnvelope, SearchResultItem, SearchType};

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct SearchRequest {
        #[schemars(description = "The search query")]
        pub q: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "type")]
        #[schemars(description = "Type of search: web, images, or news")]
        pub search_type: Option<SearchType>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Number of results to return (0-20)")]
        pub num: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Search engine to use")]
        pub engine: Option<SearchEngine>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Country code for geolocation")]
        pub gl: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Language code")]
        pub hl: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Location for search results")]
        pub location: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Page number for pagination")]
        pub page: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Limit results to specific sites")]
        pub site: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Whether to bypass the cache")]
        pub no_cache: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Maximum number of tokens in the output")]
        pub token_budget: Option<u32>,
    }
}

impl JinaClient {
    pub async fn search(&self, req: SearchRequest) -> Result<Vec<SearchResultItem>, crate::Error> {
        let response = self
            .client
            .post("https://s.jina.ai/")
            .json(&req)
            .send()
            .await?;
        let response = check_response(response).await?;
        let envelope: SearchResponseEnvelope = response.json().await?;
        Ok(envelope.data)
    }
}
