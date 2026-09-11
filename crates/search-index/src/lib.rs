//! The desktop's full-text search index: the tantivy schema, per-language
//! tokenizers, query construction, and collection management shared by the
//! Tauri plugin (`plugins/tantivy`) and the GPUI shell, so both apps search
//! and maintain the same index the same way.

mod collections;
mod error;
pub mod query;
pub mod schema;
pub mod tokenizer;
pub mod worker;

use serde::{Deserialize, Serialize};
use tantivy::schema::Schema;
use tantivy::{Index, IndexReader, IndexWriter};

pub use collections::{Collections, detect_language};
pub use error::{Error, Result};
pub use schema::build_schema;
pub use tokenizer::get_tokenizer_name_for_language;

/// The default collection the desktop registers.
pub const DEFAULT_COLLECTION: &str = "default";
/// Its directory under the vault base.
pub const DEFAULT_INDEX_PATH: &str = "search_index";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SearchDocument {
    pub id: String,
    pub doc_type: String,
    pub language: Option<String>,
    pub title: String,
    pub content: String,
    pub created_at: i64,
    #[serde(default)]
    pub facets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Snippet {
    pub fragment: String,
    pub highlights: Vec<HighlightRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct HighlightRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SearchHit {
    pub score: f32,
    pub document: SearchDocument,
    pub title_snippet: Option<Snippet>,
    pub content_snippet: Option<Snippet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SearchResult {
    pub hits: Vec<SearchHit>,
    pub count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CreatedAtFilter {
    pub gte: Option<i64>,
    pub lte: Option<i64>,
    pub gt: Option<i64>,
    pub lt: Option<i64>,
    pub eq: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SearchFilters {
    pub created_at: Option<CreatedAtFilter>,
    pub doc_type: Option<String>,
    pub facet: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SearchOptions {
    pub fuzzy: Option<bool>,
    pub distance: Option<u8>,
    pub snippets: Option<bool>,
    pub snippet_max_chars: Option<usize>,
    pub phrase_slop: Option<u32>,
}

fn default_limit() -> usize {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct SearchRequest {
    pub query: String,
    #[serde(default)]
    pub collection: Option<String>,
    #[serde(default)]
    pub filters: SearchFilters,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub options: SearchOptions,
}

pub const SCHEMA_VERSION: u32 = 2;

pub struct CollectionConfig {
    pub name: String,
    pub path: String,
    pub schema_builder: fn() -> Schema,
    pub schema_version: u32,
}

pub struct CollectionIndex {
    pub schema: Schema,
    pub index: Index,
    pub reader: IndexReader,
    pub writer: IndexWriter,
}
