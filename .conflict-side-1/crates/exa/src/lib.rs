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
    use serde_json::json;

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
                additional_queries: None,
                stream: None,
                output_schema: None,
                system_prompt: None,
                r#type: Some(SearchType::Auto),
                category: None,
                user_location: None,
                num_results: Some(10),
                include_domains: None,
                exclude_domains: None,
                start_crawl_date: None,
                end_crawl_date: None,
                start_published_date: None,
                end_published_date: None,
                include_text: None,
                exclude_text: None,
                moderation: None,
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
                ids: None,
                text: None,
                highlights: None,
                summary: None,
                livecrawl: None,
                livecrawl_timeout: None,
                max_age_hours: None,
                subpages: None,
                subpage_target: None,
                extras: None,
                context: None,
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

    #[test]
    fn test_search_request_serializes_new_fields() {
        let request = SearchRequest {
            query: "Who is the CEO of OpenAI?".to_string(),
            additional_queries: Some(vec!["OpenAI leadership".to_string()]),
            stream: Some(true),
            output_schema: Some(json!({
                "type": "object",
                "properties": { "leader": { "type": "string" } },
            })),
            system_prompt: Some("Prefer official sources".to_string()),
            r#type: Some(SearchType::DeepReasoning),
            category: Some(Category::News),
            user_location: Some("US".to_string()),
            num_results: Some(5),
            include_domains: Some(vec!["openai.com".to_string()]),
            exclude_domains: None,
            start_crawl_date: None,
            end_crawl_date: None,
            start_published_date: None,
            end_published_date: None,
            include_text: None,
            exclude_text: None,
            moderation: Some(true),
            contents: Some(ContentsRequest {
                text: Some(TextRequest::Bool(true)),
                highlights: Some(HighlightsRequest::Options(HighlightsOptions {
                    max_characters: Some(500),
                    num_sentences: None,
                    highlights_per_url: None,
                    query: Some("official announcement".to_string()),
                })),
                summary: Some(SummaryRequest {
                    query: Some("role and source".to_string()),
                    schema: Some(json!({ "type": "object" })),
                }),
                livecrawl: None,
                livecrawl_timeout: None,
                max_age_hours: Some(-1),
                subpages: Some(1),
                subpage_target: Some(SubpageTarget::String("sources".to_string())),
                extras: Some(ExtrasRequest {
                    links: Some(1),
                    image_links: Some(1),
                }),
                context: None,
            }),
        };

        let value = serde_json::to_value(request).unwrap();
        assert_eq!(value["additionalQueries"][0], "OpenAI leadership");
        assert_eq!(value["type"], "deep-reasoning");
        assert_eq!(value["contents"]["text"], true);
        assert_eq!(value["contents"]["maxAgeHours"], -1);
        assert_eq!(value["contents"]["extras"]["imageLinks"], 1);
    }

    #[test]
    fn test_search_response_deserializes_output_and_cost() {
        let payload = json!({
            "requestId": "req_123",
            "results": [
                {
                    "id": "doc_1",
                    "url": "https://example.com",
                    "title": "Example",
                    "image": "https://example.com/image.png",
                    "favicon": "https://example.com/favicon.ico"
                }
            ],
            "searchType": "deep-reasoning",
            "output": {
                "content": {
                    "leader": "Sam Altman"
                },
                "grounding": [
                    {
                        "field": "leader",
                        "citations": [
                            { "url": "https://openai.com", "title": "OpenAI" }
                        ],
                        "confidence": "high"
                    }
                ]
            },
            "costDollars": {
                "total": 0.007
            }
        });

        let parsed: SearchResponse = serde_json::from_value(payload).unwrap();
        assert_eq!(parsed.request_id.as_deref(), Some("req_123"));
        assert_eq!(parsed.search_type, Some(SearchResponseType::DeepReasoning));
        assert!(parsed.output.is_some());
        assert_eq!(parsed.cost_dollars.and_then(|c| c.total), Some(0.007));
    }

    #[test]
    fn test_contents_response_deserializes_statuses_and_cost() {
        let payload = json!({
            "requestId": "req_456",
            "results": [
                {
                    "id": "https://example.com",
                    "url": "https://example.com",
                    "text": "hello"
                }
            ],
            "statuses": [
                {
                    "id": "https://example.com",
                    "status": "success",
                    "error": null
                }
            ],
            "costDollars": {
                "total": 0.001
            }
        });

        let parsed: GetContentsResponse = serde_json::from_value(payload).unwrap();
        assert_eq!(parsed.request_id.as_deref(), Some("req_456"));
        assert_eq!(
            parsed
                .statuses
                .and_then(|s| s.first().cloned())
                .map(|s| s.status),
            Some(ContentStatusState::Success)
        );
        assert_eq!(parsed.cost_dollars.and_then(|c| c.total), Some(0.001));
    }
}
