use crate::types::{
    ImportDataResult, ImportSource, ImportSourceInfo, ImportSourceKind, ImportStats,
};
use hypr_importer_core::output::to_tinybase_json;

pub struct Importer<R: tauri::Runtime> {
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<R: tauri::Runtime> Importer<R> {
    pub fn list_available_sources(&self) -> Vec<ImportSourceInfo> {
        crate::sources::list_available_sources()
    }

    pub async fn run_import(
        &self,
        source_kind: ImportSourceKind,
        user_id: String,
    ) -> Result<ImportDataResult, crate::Error> {
        let source = ImportSource::from(source_kind.clone());
        self.run_import_from_source(&source, user_id).await
    }

    pub async fn run_import_from_source(
        &self,
        source: &ImportSource,
        user_id: String,
    ) -> Result<ImportDataResult, crate::Error> {
        if !source.is_available() {
            return Err(crate::Error::SourceNotAvailable(source.name.clone()));
        }

        let data = crate::sources::import_all(source).await?;
        let stats = ImportStats::from_data(&data);
        let tinybase_json = to_tinybase_json(&data, &user_id);

        Ok(ImportDataResult {
            stats,
            data: tinybase_json,
        })
    }

    pub async fn run_import_dry(
        &self,
        source_kind: ImportSourceKind,
    ) -> Result<ImportStats, crate::Error> {
        let source = ImportSource::from(source_kind.clone());
        self.run_import_dry_from_source(&source).await
    }

    pub async fn run_import_dry_from_source(
        &self,
        source: &ImportSource,
    ) -> Result<ImportStats, crate::Error> {
        if !source.is_available() {
            return Err(crate::Error::SourceNotAvailable(source.name.clone()));
        }

        crate::sources::import_stats(source).await
    }
}

pub trait ImporterPluginExt<R: tauri::Runtime> {
    fn importer(&self) -> Importer<R>;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> ImporterPluginExt<R> for T {
    fn importer(&self) -> Importer<R> {
        Importer {
            _runtime: std::marker::PhantomData,
        }
    }
}
