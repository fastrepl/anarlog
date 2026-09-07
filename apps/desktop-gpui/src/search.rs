//! The full-text search index shared with the Tauri app (`anlg-search-index`):
//! the same `<vault>/search_index` directory, schema, tokenizers, and
//! projection worker, so `@` mention queries and search results match the
//! web app's `tantivy.search(...)`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anlg_search_index::worker::{DrainOutcome, RETRY_INTERVAL};
use anlg_search_index::{
    CollectionConfig, Collections, DEFAULT_COLLECTION, DEFAULT_INDEX_PATH, SCHEMA_VERSION,
    SearchFilters, SearchHit, SearchOptions, SearchRequest, build_schema,
};

use crate::db::Store;

/// How often the projection worker looks for dirty rows between database
/// change notifications (the Tauri worker also wakes on this interval).
const POLL_INTERVAL: Duration = Duration::from_secs(1);

pub struct SearchIndex {
    collections: Arc<Collections>,
    ready: Arc<std::sync::atomic::AtomicBool>,
}

/// The app-wide index handle.
pub struct Search(pub Arc<SearchIndex>);

impl gpui::Global for Search {}

impl SearchIndex {
    /// Registers the default collection under the vault (the directory that
    /// holds `app.db`) and starts the projection worker. While another
    /// process (the Tauri app running alongside) holds the index writer lock,
    /// registration retries until it is free and searches report not ready.
    pub fn start(store: &Arc<Store>) -> Arc<SearchIndex> {
        let collections = Arc::new(Collections::default());
        let ready = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let index = Arc::new(SearchIndex {
            collections: collections.clone(),
            ready: ready.clone(),
        });
        let vault_base: PathBuf = store
            .path()
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default();
        let pool = store.pool().clone();
        store.runtime().spawn(async move {
            loop {
                let config = CollectionConfig {
                    name: DEFAULT_COLLECTION.to_string(),
                    path: DEFAULT_INDEX_PATH.to_string(),
                    schema_builder: build_schema,
                    schema_version: SCHEMA_VERSION,
                };
                match collections.register_collection(&vault_base, config).await {
                    Ok(()) => break,
                    Err(error) => {
                        tracing::info!(%error, "search index is held by another process; retrying");
                        tokio::time::sleep(RETRY_INTERVAL).await;
                    }
                }
            }
            loop {
                match anlg_search_index::worker::initialize(&collections, &pool).await {
                    Ok(DrainOutcome::Complete) => break,
                    Ok(DrainOutcome::Deferred) => tokio::time::sleep(RETRY_INTERVAL).await,
                    Err(error) => {
                        tracing::error!(%error, "failed to initialize search index projection");
                        tokio::time::sleep(RETRY_INTERVAL).await;
                    }
                }
            }
            ready.store(true, std::sync::atomic::Ordering::Release);
            tracing::info!("search index ready");
            loop {
                if let Err(error) =
                    anlg_search_index::worker::drain_queue(&collections, &pool).await
                {
                    tracing::error!(%error, "failed to update search index projection");
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        });
        index
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(std::sync::atomic::Ordering::Acquire)
    }

    /// `SearchEngineProvider.search(query)`: the normalised query with no
    /// filters and the default options and limit.
    pub fn search(
        &self,
        runtime: &tokio::runtime::Handle,
        query: &str,
    ) -> Result<Vec<SearchHit>, anlg_search_index::Error> {
        let request = SearchRequest {
            query: normalize_query(query),
            collection: None,
            filters: SearchFilters::default(),
            limit: 100,
            options: SearchOptions::default(),
        };
        let collections = self.collections.clone();
        runtime
            .block_on(async move { collections.search(request).await })
            .map(|result| result.hits)
    }
}

/// `normalizeQuery`: trim and collapse whitespace runs.
pub fn normalize_query(query: &str) -> String {
    query.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_whitespace_like_the_web_engine() {
        assert_eq!(normalize_query("  hello   world\t"), "hello world");
        assert_eq!(normalize_query("   "), "");
    }
}
