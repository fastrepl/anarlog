use tantivy::collector::{Count, TopDocs};
use tantivy::query::{
    BooleanQuery, BoostQuery, FuzzyTermQuery, Occur, PhraseQuery, Query, QueryParser, TermQuery,
};
use tantivy::schema::{Facet, IndexRecordOption};
use tantivy::snippet::SnippetGenerator;
use tantivy::{Index, ReloadPolicy, TantivyDocument, Term};
use tauri_plugin_settings::SettingsPluginExt;

use crate::query::build_created_at_range_query;
use crate::schema::{extract_search_document, get_fields};
use crate::tokenizer::register_tokenizers;
use crate::{
    CollectionConfig, CollectionIndex, HighlightRange, IndexState, SearchDocument, SearchHit,
    SearchRequest, SearchResult, Snippet,
};

pub fn detect_language(text: &str) -> hypr_language::Language {
    hypr_language::detect(text)
}

fn parse_query_parts(query: &str) -> (Vec<&str>, Vec<&str>) {
    let mut phrases = Vec::new();
    let mut regular_terms = Vec::new();
    let mut in_quote = false;
    let mut quote_start = 0;
    let mut current_start = 0;

    let chars: Vec<char> = query.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '"' {
            if in_quote {
                let phrase = &query[quote_start..i];
                if !phrase.trim().is_empty() {
                    phrases.push(phrase.trim());
                }
                in_quote = false;
                current_start = i + 1;
            } else {
                let before = &query[current_start..i];
                for term in before.split_whitespace() {
                    if !term.is_empty() {
                        regular_terms.push(term);
                    }
                }
                in_quote = true;
                quote_start = i + 1;
            }
        }
        i += 1;
    }

    if in_quote {
        let phrase = &query[quote_start..];
        if !phrase.trim().is_empty() {
            phrases.push(phrase.trim());
        }
    } else {
        let remaining = &query[current_start..];
        for term in remaining.split_whitespace() {
            if !term.is_empty() {
                regular_terms.push(term);
            }
        }
    }

    (phrases, regular_terms)
}

pub struct Tantivy<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Tantivy<'a, R, M> {
    pub async fn register_collection(&self, config: CollectionConfig) -> Result<(), crate::Error> {
        let base = self.manager.app_handle().settings().global_base()?;
        let index_path = base.join(&config.path).into_std_path_buf();
        let version_path = index_path.join("schema_version");

        std::fs::create_dir_all(&index_path)?;

        let state = self.manager.state::<IndexState>();
        let mut guard = state.inner.write().await;

        if guard.collections.contains_key(&config.name) {
            tracing::debug!("Collection '{}' already registered", config.name);
            return Ok(());
        }

        let schema = (config.schema_builder)();

        let needs_reindex = if index_path.join("meta.json").exists() {
            let stored_version = std::fs::read_to_string(&version_path)
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
                .unwrap_or(0);
            stored_version != config.schema_version
        } else {
            false
        };

        let index = if index_path.join("meta.json").exists() && !needs_reindex {
            Index::open_in_dir(&index_path)?
        } else {
            if needs_reindex {
                tracing::info!(
                    "Schema version changed for collection '{}', re-creating index",
                    config.name
                );
                std::fs::remove_dir_all(&index_path)?;
                std::fs::create_dir_all(&index_path)?;
            }
            Index::create_in_dir(&index_path, schema.clone())?
        };

        std::fs::write(&version_path, config.schema_version.to_string())?;

        register_tokenizers(&index);

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        let writer = index.writer(50_000_000)?;

        let collection_index = CollectionIndex {
            schema,
            index,
            reader,
            writer,
        };

        guard
            .collections
            .insert(config.name.clone(), collection_index);

        tracing::info!(
            "Tantivy collection '{}' registered at {:?} (version: {})",
            config.name,
            index_path,
            config.schema_version
        );
        Ok(())
    }

    fn get_collection_name(collection: Option<String>) -> String {
        collection.unwrap_or_else(|| "default".to_string())
    }

    pub async fn search(&self, request: SearchRequest) -> Result<SearchResult, crate::Error> {
        let collection_name = Self::get_collection_name(request.collection);
        let state = self.manager.state::<IndexState>();
        let guard = state.inner.read().await;

        let collection_index = guard
            .collections
            .get(&collection_name)
            .ok_or_else(|| crate::Error::CollectionNotFound(collection_name.clone()))?;

        let schema = &collection_index.schema;
        let index = &collection_index.index;
        let reader = &collection_index.reader;

        let fields = get_fields(schema);
        let searcher = reader.searcher();

        let use_fuzzy = request.options.fuzzy.unwrap_or(false);
        let phrase_slop = request.options.phrase_slop.unwrap_or(0);

        // Title boost factor (3x) to match Orama's title:3, content:1 behavior
        const TITLE_BOOST: f32 = 3.0;

        let mut combined_query: Box<dyn Query> = if use_fuzzy {
            let distance = request.options.distance.unwrap_or(1);

            // Parse query to extract phrases (quoted) and regular terms
            let (phrases, regular_terms) = parse_query_parts(&request.query);

            let mut term_queries: Vec<(Occur, Box<dyn Query>)> = Vec::new();

            // Handle quoted phrases with PhraseQuery
            for phrase in phrases {
                let words: Vec<&str> = phrase.split_whitespace().collect();
                if words.len() > 1 {
                    // Create phrase query for title field
                    let title_terms: Vec<Term> = words
                        .iter()
                        .map(|w| Term::from_field_text(fields.title, w))
                        .collect();
                    let mut title_phrase = PhraseQuery::new(title_terms);
                    title_phrase.set_slop(phrase_slop);

                    // Create phrase query for content field
                    let content_terms: Vec<Term> = words
                        .iter()
                        .map(|w| Term::from_field_text(fields.content, w))
                        .collect();
                    let mut content_phrase = PhraseQuery::new(content_terms);
                    content_phrase.set_slop(phrase_slop);

                    // Boost title matches by 3x
                    let boosted_title: Box<dyn Query> =
                        Box::new(BoostQuery::new(Box::new(title_phrase), TITLE_BOOST));
                    let content_query: Box<dyn Query> = Box::new(content_phrase);

                    // Phrase must match in at least one field (title OR content)
                    let phrase_field_query = BooleanQuery::new(vec![
                        (Occur::Should, boosted_title),
                        (Occur::Should, content_query),
                    ]);

                    term_queries.push((Occur::Must, Box::new(phrase_field_query)));
                } else if !words.is_empty() {
                    // Single word "phrase" - treat as regular term
                    let word = words[0];
                    let title_fuzzy = FuzzyTermQuery::new(
                        Term::from_field_text(fields.title, word),
                        distance,
                        true,
                    );
                    let content_fuzzy = FuzzyTermQuery::new(
                        Term::from_field_text(fields.content, word),
                        distance,
                        true,
                    );

                    let boosted_title: Box<dyn Query> =
                        Box::new(BoostQuery::new(Box::new(title_fuzzy), TITLE_BOOST));
                    let content_query: Box<dyn Query> = Box::new(content_fuzzy);

                    let term_field_query = BooleanQuery::new(vec![
                        (Occur::Should, boosted_title),
                        (Occur::Should, content_query),
                    ]);

                    term_queries.push((Occur::Must, Box::new(term_field_query)));
                }
            }

            // Handle regular (unquoted) terms with fuzzy matching
            for term in regular_terms {
                let title_fuzzy =
                    FuzzyTermQuery::new(Term::from_field_text(fields.title, term), distance, true);
                let content_fuzzy = FuzzyTermQuery::new(
                    Term::from_field_text(fields.content, term),
                    distance,
                    true,
                );

                // Boost title matches by 3x
                let boosted_title: Box<dyn Query> =
                    Box::new(BoostQuery::new(Box::new(title_fuzzy), TITLE_BOOST));
                let content_query: Box<dyn Query> = Box::new(content_fuzzy);

                // Each term must match in at least one field (title OR content)
                let term_field_query = BooleanQuery::new(vec![
                    (Occur::Should, boosted_title),
                    (Occur::Should, content_query),
                ]);

                // All terms must be present (Must for each term)
                term_queries.push((Occur::Must, Box::new(term_field_query)));
            }

            Box::new(BooleanQuery::new(term_queries))
        } else {
            let query_parser = QueryParser::for_index(index, vec![fields.title, fields.content]);
            query_parser.parse_query(&request.query)?
        };

        // Apply created_at filter
        if let Some(ref created_at_filter) = request.filters.created_at {
            let range_query = build_created_at_range_query(fields.created_at, created_at_filter);
            if let Some(rq) = range_query {
                combined_query = Box::new(BooleanQuery::new(vec![
                    (Occur::Must, combined_query),
                    (Occur::Must, rq),
                ]));
            }
        }

        // Apply doc_type filter
        if let Some(ref doc_type) = request.filters.doc_type {
            let doc_type_term = Term::from_field_text(fields.doc_type, doc_type);
            let doc_type_query = TermQuery::new(doc_type_term, IndexRecordOption::Basic);
            combined_query = Box::new(BooleanQuery::new(vec![
                (Occur::Must, combined_query),
                (Occur::Must, Box::new(doc_type_query)),
            ]));
        }

        // Apply facet filter
        if let Some(ref facet_path) = request.filters.facet
            && let Ok(facet) = Facet::from_text(facet_path)
        {
            let facet_term = Term::from_facet(fields.facets, &facet);
            let facet_query = TermQuery::new(facet_term, IndexRecordOption::Basic);
            combined_query = Box::new(BooleanQuery::new(vec![
                (Occur::Must, combined_query),
                (Occur::Must, Box::new(facet_query)),
            ]));
        }

        // Use tuple collector to get both top docs and total count
        let (top_docs, count) = searcher.search(
            &combined_query,
            &(TopDocs::with_limit(request.limit), Count),
        )?;

        let generate_snippets = request.options.snippets.unwrap_or(false);
        let snippet_max_chars = request.options.snippet_max_chars.unwrap_or(150);

        let (title_snippet_gen, content_snippet_gen) = if generate_snippets {
            let mut title_gen =
                SnippetGenerator::create(&searcher, &*combined_query, fields.title)?;
            title_gen.set_max_num_chars(snippet_max_chars);

            let mut content_gen =
                SnippetGenerator::create(&searcher, &*combined_query, fields.content)?;
            content_gen.set_max_num_chars(snippet_max_chars);

            (Some(title_gen), Some(content_gen))
        } else {
            (None, None)
        };

        let mut hits = Vec::new();
        for (score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;

            if let Some(search_doc) = extract_search_document(schema, &fields, &retrieved_doc) {
                let title_snippet = title_snippet_gen.as_ref().map(|generator| {
                    let snippet = generator.snippet_from_doc(&retrieved_doc);
                    Snippet {
                        fragment: snippet.fragment().to_string(),
                        highlights: snippet
                            .highlighted()
                            .iter()
                            .map(|range| HighlightRange {
                                start: range.start,
                                end: range.end,
                            })
                            .collect(),
                    }
                });

                let content_snippet = content_snippet_gen.as_ref().map(|generator| {
                    let snippet = generator.snippet_from_doc(&retrieved_doc);
                    Snippet {
                        fragment: snippet.fragment().to_string(),
                        highlights: snippet
                            .highlighted()
                            .iter()
                            .map(|range| HighlightRange {
                                start: range.start,
                                end: range.end,
                            })
                            .collect(),
                    }
                });

                hits.push(SearchHit {
                    score,
                    document: search_doc,
                    title_snippet,
                    content_snippet,
                });
            }
        }

        Ok(SearchResult { hits, count })
    }

    pub async fn reindex(&self, collection: Option<String>) -> Result<(), crate::Error> {
        let collection_name = Self::get_collection_name(collection);
        let state = self.manager.state::<IndexState>();
        let mut guard = state.inner.write().await;

        let collection_index = guard
            .collections
            .get_mut(&collection_name)
            .ok_or_else(|| crate::Error::CollectionNotFound(collection_name.clone()))?;

        let schema = &collection_index.schema;
        let writer = &mut collection_index.writer;

        writer.delete_all_documents()?;

        let fields = get_fields(schema);

        writer.commit()?;

        tracing::info!(
            "Reindex completed for collection '{}'. Index cleared and ready for new documents. Fields: {:?}",
            collection_name,
            fields.id
        );

        Ok(())
    }

    pub async fn add_document(
        &self,
        collection: Option<String>,
        document: SearchDocument,
    ) -> Result<(), crate::Error> {
        let collection_name = Self::get_collection_name(collection);
        let state = self.manager.state::<IndexState>();
        let mut guard = state.inner.write().await;

        let collection_index = guard
            .collections
            .get_mut(&collection_name)
            .ok_or_else(|| crate::Error::CollectionNotFound(collection_name.clone()))?;

        let schema = &collection_index.schema;
        let writer = &mut collection_index.writer;
        let fields = get_fields(schema);

        let mut doc = TantivyDocument::new();
        doc.add_text(fields.id, &document.id);
        doc.add_text(fields.doc_type, &document.doc_type);
        doc.add_text(fields.language, document.language.as_deref().unwrap_or(""));
        doc.add_text(fields.title, &document.title);
        doc.add_text(fields.content, &document.content);
        doc.add_i64(fields.created_at, document.created_at);

        for facet_path in &document.facets {
            if let Ok(facet) = Facet::from_text(facet_path) {
                doc.add_facet(fields.facets, facet);
            }
        }

        writer.add_document(doc)?;
        writer.commit()?;

        tracing::debug!(
            "Added document '{}' to collection '{}'",
            document.id,
            collection_name
        );

        Ok(())
    }

    pub async fn update_document(
        &self,
        collection: Option<String>,
        document: SearchDocument,
    ) -> Result<(), crate::Error> {
        let collection_name = Self::get_collection_name(collection);
        let state = self.manager.state::<IndexState>();
        let mut guard = state.inner.write().await;

        let collection_index = guard
            .collections
            .get_mut(&collection_name)
            .ok_or_else(|| crate::Error::CollectionNotFound(collection_name.clone()))?;

        let schema = &collection_index.schema;
        let writer = &mut collection_index.writer;
        let fields = get_fields(schema);

        let id_term = Term::from_field_text(fields.id, &document.id);
        writer.delete_term(id_term);

        let mut doc = TantivyDocument::new();
        doc.add_text(fields.id, &document.id);
        doc.add_text(fields.doc_type, &document.doc_type);
        doc.add_text(fields.language, document.language.as_deref().unwrap_or(""));
        doc.add_text(fields.title, &document.title);
        doc.add_text(fields.content, &document.content);
        doc.add_i64(fields.created_at, document.created_at);

        for facet_path in &document.facets {
            if let Ok(facet) = Facet::from_text(facet_path) {
                doc.add_facet(fields.facets, facet);
            }
        }

        writer.add_document(doc)?;
        writer.commit()?;

        tracing::debug!(
            "Updated document '{}' in collection '{}'",
            document.id,
            collection_name
        );

        Ok(())
    }

    pub async fn update_documents(
        &self,
        collection: Option<String>,
        documents: Vec<SearchDocument>,
    ) -> Result<(), crate::Error> {
        let collection_name = Self::get_collection_name(collection);
        let state = self.manager.state::<IndexState>();
        let mut guard = state.inner.write().await;

        let collection_index = guard
            .collections
            .get_mut(&collection_name)
            .ok_or_else(|| crate::Error::CollectionNotFound(collection_name.clone()))?;

        let schema = &collection_index.schema;
        let writer = &mut collection_index.writer;
        let fields = get_fields(schema);

        let count = documents.len();

        for document in documents {
            let id_term = Term::from_field_text(fields.id, &document.id);
            writer.delete_term(id_term);

            let mut doc = TantivyDocument::new();
            doc.add_text(fields.id, &document.id);
            doc.add_text(fields.doc_type, &document.doc_type);
            doc.add_text(fields.language, document.language.as_deref().unwrap_or(""));
            doc.add_text(fields.title, &document.title);
            doc.add_text(fields.content, &document.content);
            doc.add_i64(fields.created_at, document.created_at);

            for facet_path in &document.facets {
                if let Ok(facet) = Facet::from_text(facet_path) {
                    doc.add_facet(fields.facets, facet);
                }
            }

            writer.add_document(doc)?;
        }

        writer.commit()?;

        tracing::debug!(
            "Updated {} documents in collection '{}'",
            count,
            collection_name
        );

        Ok(())
    }

    pub async fn remove_document(
        &self,
        collection: Option<String>,
        id: String,
    ) -> Result<(), crate::Error> {
        let collection_name = Self::get_collection_name(collection);
        let state = self.manager.state::<IndexState>();
        let mut guard = state.inner.write().await;

        let collection_index = guard
            .collections
            .get_mut(&collection_name)
            .ok_or_else(|| crate::Error::CollectionNotFound(collection_name.clone()))?;

        let schema = &collection_index.schema;
        let writer = &mut collection_index.writer;
        let fields = get_fields(schema);

        let id_term = Term::from_field_text(fields.id, &id);
        writer.delete_term(id_term);
        writer.commit()?;

        tracing::debug!(
            "Removed document '{}' from collection '{}'",
            id,
            collection_name
        );

        Ok(())
    }
}

pub trait TantivyPluginExt<R: tauri::Runtime> {
    fn tantivy(&self) -> Tantivy<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> TantivyPluginExt<R> for T {
    fn tantivy(&self) -> Tantivy<'_, R, Self>
    where
        Self: Sized,
    {
        Tantivy {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::get_tokenizer_name_for_language;

    #[test]
    fn test_detect_language_tokenizer_integration() {
        let text = "The quick brown fox jumps over the lazy dog.";
        let lang = detect_language(text);
        let tokenizer_name = get_tokenizer_name_for_language(&lang);
        assert_eq!(tokenizer_name, "lang_en");
    }
}
