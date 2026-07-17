use axum::{Json, Router, extract::State, http::StatusCode, routing::post};

use crate::config::ResearchConfig;
use crate::mcp::mcp_service;
use crate::state::AppState;

pub fn router(config: ResearchConfig) -> Router {
    let state = AppState::new(config);
    let mcp = mcp_service(state.clone());

    Router::new().nest(
        "/research",
        Router::new()
            .route("/search", post(web_search))
            .nest_service("/mcp", mcp)
            .with_state(state),
    )
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchRequest {
    pub query: String,
    pub num_results: Option<u32>,
    pub include_domains: Option<Vec<String>>,
    pub exclude_domains: Option<Vec<String>>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchResponse {
    pub query: String,
    pub results: Vec<WebSearchResult>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub published_date: Option<String>,
    pub author: Option<String>,
}

async fn web_search(
    State(state): State<AppState>,
    Json(request): Json<WebSearchRequest>,
) -> Result<Json<WebSearchResponse>, (StatusCode, String)> {
    let query = request.query.trim().to_string();
    if query.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "query is required".to_string()));
    }

    let response = state
        .exa
        .search(hypr_exa::SearchRequest {
            query: query.clone(),
            additional_queries: None,
            stream: None,
            output_schema: None,
            system_prompt: None,
            r#type: Some(hypr_exa::SearchType::Auto),
            category: None,
            user_location: None,
            num_results: Some(request.num_results.unwrap_or(5).clamp(1, 10)),
            include_domains: normalize_domains(request.include_domains),
            exclude_domains: normalize_domains(request.exclude_domains),
            start_crawl_date: None,
            end_crawl_date: None,
            start_published_date: None,
            end_published_date: None,
            include_text: None,
            exclude_text: None,
            moderation: Some(true),
            contents: Some(hypr_exa::ContentsRequest {
                text: Some(hypr_exa::TextRequest::Options(hypr_exa::TextOptions {
                    max_characters: Some(1200),
                    include_html_tags: Some(false),
                    verbosity: Some(hypr_exa::TextVerbosity::Compact),
                    include_sections: None,
                    exclude_sections: None,
                })),
                highlights: Some(hypr_exa::HighlightsRequest::Options(
                    hypr_exa::HighlightsOptions {
                        max_characters: Some(400),
                        num_sentences: Some(2),
                        highlights_per_url: Some(1),
                        query: Some(query.clone()),
                    },
                )),
                summary: Some(hypr_exa::SummaryRequest {
                    query: Some(format!("Summarize the information relevant to: {query}")),
                    schema: None,
                }),
                livecrawl: Some(hypr_exa::Livecrawl::Preferred),
                livecrawl_timeout: None,
                max_age_hours: None,
                subpages: None,
                subpage_target: None,
                extras: None,
                context: None,
            }),
        })
        .await
        .map_err(|error| (StatusCode::BAD_GATEWAY, error.to_string()))?;

    Ok(Json(WebSearchResponse {
        query,
        results: response
            .results
            .into_iter()
            .map(normalize_search_result)
            .collect(),
    }))
}

fn normalize_domains(domains: Option<Vec<String>>) -> Option<Vec<String>> {
    domains
        .map(|domains| {
            domains
                .into_iter()
                .map(|domain| domain.trim().to_string())
                .filter(|domain| !domain.is_empty())
                .take(10)
                .collect::<Vec<_>>()
        })
        .filter(|domains| !domains.is_empty())
}

fn normalize_search_result(result: hypr_exa::SearchResult) -> WebSearchResult {
    let snippet = result
        .summary
        .as_deref()
        .filter(|text| !text.trim().is_empty())
        .or_else(|| {
            result
                .highlights
                .as_ref()
                .and_then(|highlights| highlights.iter().find(|text| !text.trim().is_empty()))
                .map(String::as_str)
        })
        .or_else(|| {
            result
                .text
                .as_deref()
                .filter(|text| !text.trim().is_empty())
        })
        .map(|text| trim_to_chars(text.trim(), 600))
        .unwrap_or_default();

    WebSearchResult {
        title: result
            .title
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| result.url.clone()),
        url: result.url,
        snippet,
        published_date: result.published_date,
        author: result.author,
    }
}

fn trim_to_chars(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let trimmed: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{trimmed}...")
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_domains_discards_empty_entries() {
        assert_eq!(
            normalize_domains(Some(vec![
                " char.com ".to_string(),
                "".to_string(),
                "openai.com".to_string(),
            ])),
            Some(vec!["char.com".to_string(), "openai.com".to_string()])
        );
    }

    #[test]
    fn normalize_search_result_prefers_summary_over_highlights() {
        let result = normalize_search_result(hypr_exa::SearchResult {
            id: "result-1".to_string(),
            url: "https://char.com".to_string(),
            title: Some("Char".to_string()),
            published_date: Some("2026-06-09".to_string()),
            author: Some("Char".to_string()),
            score: Some(0.9),
            text: Some("Full page text".to_string()),
            highlights: Some(vec!["Relevant highlight".to_string()]),
            highlight_scores: None,
            summary: Some("Relevant summary".to_string()),
            image: None,
            favicon: None,
            subpages: None,
            extras: None,
        });

        assert_eq!(result.title, "Char");
        assert_eq!(result.url, "https://char.com");
        assert_eq!(result.snippet, "Relevant summary");
        assert_eq!(result.published_date.as_deref(), Some("2026-06-09"));
    }
}
