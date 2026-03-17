mod answer;
mod client;
mod contents;
mod error;
mod find_similar;
mod search;
mod types;

pub use answer::*;
pub use client::*;
pub use contents::*;
pub use error::*;
pub use find_similar::*;
pub use search::*;
pub use types::*;

macro_rules! common_derives {
    ($item:item) => {
        #[derive(
            Debug,
            Eq,
            PartialEq,
            Clone,
            serde::Serialize,
            serde::Deserialize,
            specta::Type,
            schemars::JsonSchema,
        )]
        $item
    };
}

macro_rules! float_derives {
    ($item:item) => {
        #[derive(
            Debug,
            PartialEq,
            Clone,
            serde::Serialize,
            serde::Deserialize,
            specta::Type,
            schemars::JsonSchema,
        )]
        $item
    };
}

pub(crate) use common_derives;
pub(crate) use float_derives;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_search() {
        let client = ExaClientBuilder::default()
            .api_key("test-key")
            .build()
            .unwrap();

        let _ = client
            .search(SearchRequest {
                query: "Latest AI developments".to_string(),
                r#type: Some(SearchType::Auto),
                category: None,
                num_results: Some(10),
                include_domains: None,
                exclude_domains: None,
                start_crawl_date: None,
                end_crawl_date: None,
                start_published_date: None,
                end_published_date: None,
                include_text: None,
                exclude_text: None,
                contents: None,
            })
            .await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_contents() {
        let client = ExaClientBuilder::default()
            .api_key("test-key")
            .build()
            .unwrap();

        let _ = client
            .get_contents(GetContentsRequest {
                urls: vec!["https://arxiv.org/pdf/2307.06435".to_string()],
                text: None,
                highlights: None,
                summary: None,
                livecrawl: None,
                livecrawl_timeout: None,
                max_age_hours: None,
            })
            .await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_find_similar() {
        let client = ExaClientBuilder::default()
            .api_key("test-key")
            .build()
            .unwrap();

        let _ = client
            .find_similar(FindSimilarRequest {
                url: "https://arxiv.org/abs/2307.06435".to_string(),
                num_results: Some(5),
                include_domains: None,
                exclude_domains: None,
                start_crawl_date: None,
                end_crawl_date: None,
                start_published_date: None,
                end_published_date: None,
                include_text: None,
                exclude_text: None,
                contents: None,
            })
            .await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_answer() {
        let client = ExaClientBuilder::default()
            .api_key("test-key")
            .build()
            .unwrap();

        let _ = client
            .answer(AnswerRequest {
                query: "What is the latest valuation of SpaceX?".to_string(),
                text: None,
                output_schema: None,
            })
            .await;
    }

    #[test]
    fn test_build_missing_api_key() {
        let result = ExaClientBuilder::default().build();
        assert!(result.is_err());
    }

    #[test]
    fn test_build_defaults_api_base() {
        let client = ExaClientBuilder::default().api_key("key").build().unwrap();
        assert_eq!(client.api_base.as_str(), "https://api.exa.ai/");
    }

    #[test]
    fn test_build_custom_api_base() {
        let client = ExaClientBuilder::default()
            .api_key("key")
            .api_base("https://custom.exa.ai")
            .build()
            .unwrap();
        assert_eq!(client.api_base.as_str(), "https://custom.exa.ai/");
    }
}
