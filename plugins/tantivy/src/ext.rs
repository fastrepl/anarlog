use tauri_plugin_settings::SettingsPluginExt;

use crate::{CollectionConfig, IndexState, SearchDocument, SearchRequest, SearchResult};

pub use anlg_search_index::detect_language;

pub struct Tantivy<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Tantivy<'a, R, M> {
    fn collections(&self) -> tauri::State<'_, IndexState> {
        self.manager.state::<IndexState>()
    }

    /// Registers the collection under the vault base (`<vault>/<config.path>`).
    pub async fn register_collection(&self, config: CollectionConfig) -> Result<(), crate::Error> {
        let vault_base = self.manager.app_handle().settings().vault_base()?;
        let state = self.collections();
        state
            .collections
            .register_collection(vault_base.as_std_path(), config)
            .await?;
        Ok(())
    }

    pub async fn search(&self, request: SearchRequest) -> Result<SearchResult, crate::Error> {
        Ok(self.collections().collections.search(request).await?)
    }

    pub async fn reindex(&self, collection: Option<String>) -> Result<(), crate::Error> {
        Ok(self.collections().collections.reindex(collection).await?)
    }

    pub async fn add_document(
        &self,
        collection: Option<String>,
        document: SearchDocument,
    ) -> Result<(), crate::Error> {
        Ok(self
            .collections()
            .collections
            .add_document(collection, document)
            .await?)
    }

    pub async fn update_document(
        &self,
        collection: Option<String>,
        document: SearchDocument,
    ) -> Result<(), crate::Error> {
        Ok(self
            .collections()
            .collections
            .update_document(collection, document)
            .await?)
    }

    pub async fn update_documents(
        &self,
        collection: Option<String>,
        documents: Vec<SearchDocument>,
    ) -> Result<(), crate::Error> {
        Ok(self
            .collections()
            .collections
            .update_documents(collection, documents)
            .await?)
    }

    pub async fn apply_document_batch(
        &self,
        collection: Option<String>,
        upserts: Vec<SearchDocument>,
        removals: Vec<String>,
    ) -> Result<(), crate::Error> {
        Ok(self
            .collections()
            .collections
            .apply_document_batch(collection, upserts, removals)
            .await?)
    }

    pub async fn remove_document(
        &self,
        collection: Option<String>,
        id: String,
    ) -> Result<(), crate::Error> {
        Ok(self
            .collections()
            .collections
            .remove_document(collection, id)
            .await?)
    }

    pub async fn document_count(&self, collection: Option<String>) -> Result<usize, crate::Error> {
        Ok(self
            .collections()
            .collections
            .document_count(collection)
            .await?)
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
