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
    }
}

common_derives! {
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
        pub max_age_hours: Option<u32>,
    }
}

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct TextRequest {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub max_characters: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub include_html_tags: Option<bool>,
    }
}

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct HighlightsRequest {
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

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct SummaryRequest {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub query: Option<String>,
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
        #[serde(rename = "deep")]
        Deep,
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
        #[serde(rename = "tweet")]
        Tweet,
        #[serde(rename = "personal site")]
        PersonalSite,
        #[serde(rename = "financial report")]
        FinancialReport,
        #[serde(rename = "people")]
        People,
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
