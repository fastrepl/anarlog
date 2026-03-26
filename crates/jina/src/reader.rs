use crate::client::{JinaClient, check_response};
use crate::common_derives;
use crate::types::{ReaderResponseEnvelope, RespondWith, RetainImages};

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct ReadUrlRequest {
        #[schemars(description = "The URL to read and convert to markdown")]
        pub url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Response format")]
        pub respond_with: Option<RespondWith>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Whether to bypass the cache")]
        pub no_cache: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "CSS selectors to focus on specific elements")]
        pub target_selector: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "CSS selectors for elements to wait for before reading")]
        pub wait_for_selector: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "CSS selectors for elements to remove from the output")]
        pub remove_selector: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Maximum number of tokens in the output")]
        pub token_budget: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Include a summary of all links at the end")]
        pub with_links_summary: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Include a summary of all images at the end")]
        pub with_images_summary: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "How to handle images in the output")]
        pub retain_images: Option<RetainImages>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Generate alt text for images")]
        pub with_generated_alt: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Include iframe content")]
        pub with_iframe: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Include shadow DOM content")]
        pub with_shadow_dom: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[schemars(description = "Timeout in milliseconds")]
        pub timeout: Option<u32>,
    }
}

impl JinaClient {
    pub async fn read_url(&self, req: ReadUrlRequest) -> Result<String, crate::Error> {
        let response = self
            .client
            .post("https://r.jina.ai/")
            .json(&req)
            .send()
            .await?;
        let response = check_response(response).await?;
        let envelope: ReaderResponseEnvelope = response.json().await?;
        Ok(envelope.data.content)
    }
}
