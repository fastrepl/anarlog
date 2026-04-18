use crate::common_derives;

use crate::float_derives;

float_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct SearchResult {
        pub id: String,
        pub url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub published_date: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub author: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub score: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub highlights: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub highlight_scores: Option<Vec<f64>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub summary: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub image: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub favicon: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub subpages: Option<Vec<SearchResult>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub extras: Option<ResultExtras>,
    }
}

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct ResultExtras {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub links: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub image_links: Option<Vec<String>>,
    }
}

float_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct ContentsRequest {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub text: Option<TextRequest>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub highlights: Option<HighlightsRequest>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub summary: Option<SummaryRequest>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub livecrawl: Option<Livecrawl>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub livecrawl_timeout: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub max_age_hours: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub subpages: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub subpage_target: Option<SubpageTarget>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub extras: Option<ExtrasRequest>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub context: Option<ContextRequest>,
    }
}

common_derives! {
    #[serde(untagged)]
    pub enum TextRequest {
        Bool(bool),
        Options(TextOptions),
    }
}

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct TextOptions {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub max_characters: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub include_html_tags: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub verbosity: Option<TextVerbosity>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub include_sections: Option<Vec<ContentSection>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub exclude_sections: Option<Vec<ContentSection>>,
    }
}

common_derives! {
    pub enum TextVerbosity {
        #[serde(rename = "compact")]
        Compact,
        #[serde(rename = "standard")]
        Standard,
        #[serde(rename = "full")]
        Full,
    }
}

common_derives! {
    pub enum ContentSection {
        #[serde(rename = "header")]
        Header,
        #[serde(rename = "navigation")]
        Navigation,
        #[serde(rename = "banner")]
        Banner,
        #[serde(rename = "body")]
        Body,
        #[serde(rename = "sidebar")]
        Sidebar,
        #[serde(rename = "footer")]
        Footer,
        #[serde(rename = "metadata")]
        Metadata,
    }
}

common_derives! {
    #[serde(untagged)]
    pub enum HighlightsRequest {
        Bool(bool),
        Options(HighlightsOptions),
    }
}

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct HighlightsOptions {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub max_characters: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub num_sentences: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub highlights_per_url: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub query: Option<String>,
    }
}

float_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct SummaryRequest {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub query: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub schema: Option<serde_json::Value>,
    }
}

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct ExtrasRequest {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub links: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub image_links: Option<u32>,
    }
}

common_derives! {
    #[serde(untagged)]
    pub enum SubpageTarget {
        String(String),
        Array(Vec<String>),
    }
}

common_derives! {
    #[serde(untagged)]
    pub enum ContextRequest {
        Bool(bool),
        Options(ContextOptions),
    }
}

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct ContextOptions {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub max_characters: Option<u32>,
    }
}

common_derives! {
    pub enum Livecrawl {
        #[serde(rename = "never")]
        Never,
        #[serde(rename = "fallback")]
        Fallback,
        #[serde(rename = "preferred")]
        Preferred,
        #[serde(rename = "always")]
        Always,
    }
}

common_derives! {
    pub enum SearchType {
        #[serde(rename = "neural")]
        Neural,
        #[serde(rename = "fast")]
        Fast,
        #[serde(rename = "auto")]
        Auto,
        #[serde(rename = "deep-lite")]
        DeepLite,
        #[serde(rename = "deep")]
        Deep,
        #[serde(rename = "deep-reasoning")]
        DeepReasoning,
        #[serde(rename = "instant")]
        Instant,
    }
}

common_derives! {
    pub enum Category {
        #[serde(rename = "company")]
        Company,
        #[serde(rename = "research paper")]
        ResearchPaper,
        #[serde(rename = "news")]
        News,
        #[serde(rename = "personal site")]
        PersonalSite,
        #[serde(rename = "financial report")]
        FinancialReport,
        #[serde(rename = "people")]
        People,
    }
}

float_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct CostDollars {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub total: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub break_down: Option<Vec<CostBreakdownItem>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub per_request_prices: Option<PerRequestPrices>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub per_page_prices: Option<PerPagePrices>,
    }
}

float_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct CostBreakdownItem {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub search: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub contents: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub breakdown: Option<CostBreakdownDetails>,
    }
}

float_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct CostBreakdownDetails {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub neural_search: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub deep_search: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub content_text: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub content_highlight: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub content_summary: Option<f64>,
    }
}

float_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct PerRequestPrices {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub neural_search_1_10_results: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub neural_search_additional_result: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub deep_search: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub deep_reasoning_search: Option<f64>,
    }
}

float_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct PerPagePrices {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub content_text: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub content_highlight: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub content_summary: Option<f64>,
    }
}

common_derives! {
    pub enum SearchResponseType {
        #[serde(rename = "neural")]
        Neural,
        #[serde(rename = "deep")]
        Deep,
        #[serde(rename = "deep-reasoning")]
        DeepReasoning,
    }
}

float_derives! {
    pub struct SearchOutput {
        pub content: serde_json::Value,
        pub grounding: Vec<OutputGrounding>,
    }
}

common_derives! {
    pub struct OutputGrounding {
        pub field: String,
        pub citations: Vec<OutputGroundingCitation>,
        pub confidence: GroundingConfidence,
    }
}

common_derives! {
    pub struct OutputGroundingCitation {
        pub url: String,
        pub title: String,
    }
}

common_derives! {
    pub enum GroundingConfidence {
        #[serde(rename = "low")]
        Low,
        #[serde(rename = "medium")]
        Medium,
        #[serde(rename = "high")]
        High,
    }
}

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct ContentStatus {
        pub id: String,
        pub status: ContentStatusState,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub error: Option<ContentStatusError>,
    }
}

common_derives! {
    pub enum ContentStatusState {
        #[serde(rename = "success")]
        Success,
        #[serde(rename = "error")]
        Error,
    }
}

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct ContentStatusError {
        pub tag: CrawlErrorTag,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub http_status_code: Option<u16>,
    }
}

common_derives! {
    pub enum CrawlErrorTag {
        #[serde(rename = "CRAWL_NOT_FOUND")]
        CrawlNotFound,
        #[serde(rename = "CRAWL_TIMEOUT")]
        CrawlTimeout,
        #[serde(rename = "CRAWL_LIVECRAWL_TIMEOUT")]
        CrawlLivecrawlTimeout,
        #[serde(rename = "SOURCE_NOT_AVAILABLE")]
        SourceNotAvailable,
        #[serde(rename = "UNSUPPORTED_URL")]
        UnsupportedUrl,
        #[serde(rename = "CRAWL_UNKNOWN_ERROR")]
        CrawlUnknownError,
    }
}

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct AnswerCitation {
        pub id: String,
        pub url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub published_date: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub author: Option<String>,
    }
}
