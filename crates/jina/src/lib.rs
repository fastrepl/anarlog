mod client;
mod error;
mod reader;
mod search;
mod types;

pub use client::*;
pub use error::*;
pub use reader::*;
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

pub(crate) use common_derives;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_read_url() {
        let client = JinaClientBuilder::default()
            .api_key("test-key")
            .build()
            .unwrap();

        let _ = client
            .read_url(ReadUrlRequest {
                url: "https://example.com".to_string(),
                respond_with: None,
                no_cache: None,
                target_selector: None,
                wait_for_selector: None,
                remove_selector: None,
                token_budget: None,
                with_links_summary: None,
                with_images_summary: None,
                retain_images: None,
                with_generated_alt: None,
                with_iframe: None,
                with_shadow_dom: None,
                timeout: None,
            })
            .await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_search() {
        let client = JinaClientBuilder::default()
            .api_key("test-key")
            .build()
            .unwrap();

        let _ = client
            .search(SearchRequest {
                q: "latest AI developments".to_string(),
                search_type: None,
                num: Some(5),
                engine: None,
                gl: None,
                hl: None,
                location: None,
                page: None,
                site: None,
                no_cache: None,
                token_budget: None,
            })
            .await;
    }

    #[test]
    fn test_build_missing_api_key() {
        let result = JinaClientBuilder::default().build();
        assert!(result.is_err());
    }
}
