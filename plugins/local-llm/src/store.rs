use tauri_plugin_store2::ScopedStoreKey;

#[derive(serde::Deserialize, specta::Type, PartialEq, Eq, Hash)]
pub enum StoreKey {
    Model,
    ModelSelection,
    DefaultModelMigrated,
}

impl std::fmt::Display for StoreKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreKey::Model => write!(f, "Model"),
            StoreKey::ModelSelection => write!(f, "ModelSelection"),
            StoreKey::DefaultModelMigrated => write!(f, "DefaultModelMigrated"),
        }
    }
}

impl ScopedStoreKey for StoreKey {}

pub struct TauriModelStore<'a, R: tauri::Runtime> {
    store: &'a tauri_plugin_store2::ScopedStore<R, StoreKey>,
}

impl<'a, R: tauri::Runtime> TauriModelStore<'a, R> {
    pub fn new(store: &'a tauri_plugin_store2::ScopedStore<R, StoreKey>) -> Self {
        Self { store }
    }
}

impl<R: tauri::Runtime> hypr_local_llm_core::ModelStore for TauriModelStore<'_, R> {
    fn get_model(
        &self,
    ) -> Result<Option<hypr_local_llm_core::SupportedModel>, hypr_local_llm_core::Error> {
        self.store
            .get(StoreKey::Model)
            .map_err(|e| hypr_local_llm_core::Error::StoreError(e.to_string()))
    }

    fn set_model(
        &self,
        model: &hypr_local_llm_core::SupportedModel,
    ) -> Result<(), hypr_local_llm_core::Error> {
        self.store
            .set(StoreKey::Model, model)
            .map_err(|e| hypr_local_llm_core::Error::StoreError(e.to_string()))
    }

    fn get_model_selection(
        &self,
    ) -> Result<Option<hypr_local_llm_core::ModelSelection>, hypr_local_llm_core::Error> {
        self.store
            .get(StoreKey::ModelSelection)
            .map_err(|e| hypr_local_llm_core::Error::StoreError(e.to_string()))
    }

    fn set_model_selection(
        &self,
        selection: &hypr_local_llm_core::ModelSelection,
    ) -> Result<(), hypr_local_llm_core::Error> {
        self.store
            .set(StoreKey::ModelSelection, selection)
            .map_err(|e| hypr_local_llm_core::Error::StoreError(e.to_string()))
    }

    fn is_default_model_migrated(&self) -> Result<bool, hypr_local_llm_core::Error> {
        self.store
            .get::<bool>(StoreKey::DefaultModelMigrated)
            .map(|opt| opt.unwrap_or(false))
            .map_err(|e| hypr_local_llm_core::Error::StoreError(e.to_string()))
    }

    fn set_default_model_migrated(&self, val: bool) -> Result<(), hypr_local_llm_core::Error> {
        self.store
            .set(StoreKey::DefaultModelMigrated, val)
            .map_err(|e| hypr_local_llm_core::Error::StoreError(e.to_string()))
    }
}
