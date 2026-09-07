mod commands;
mod error;
mod ext;

use tauri::Manager;

pub use anlg_search_index::{
    CollectionConfig, CollectionIndex, Collections, CreatedAtFilter, HighlightRange,
    SCHEMA_VERSION, SearchDocument, SearchFilters, SearchHit, SearchOptions, SearchRequest,
    SearchResult, Snippet, build_schema, get_tokenizer_name_for_language,
};
pub use error::{Error, Result};
pub use ext::*;

const PLUGIN_NAME: &str = "tantivy";

/// The plugin's managed state: the shared collection registry.
#[derive(Default)]
pub struct IndexState {
    pub collections: Collections,
}

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::search::<tauri::Wry>,
            commands::reindex::<tauri::Wry>,
            commands::add_document::<tauri::Wry>,
            commands::update_document::<tauri::Wry>,
            commands::update_documents::<tauri::Wry>,
            commands::remove_document::<tauri::Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(|app, _api| {
            app.manage(IndexState::default());

            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                let config = CollectionConfig {
                    name: anlg_search_index::DEFAULT_COLLECTION.to_string(),
                    path: anlg_search_index::DEFAULT_INDEX_PATH.to_string(),
                    schema_builder: build_schema,
                    schema_version: SCHEMA_VERSION,
                };

                if let Err(e) = handle.tantivy().register_collection(config).await {
                    tracing::error!("Failed to register default collection: {}", e);
                }
            });

            Ok(())
        })
        .build()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder::<tauri::Wry>()
            .export(
                specta_typescript::Typescript::default()
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                OUTPUT_FILE,
            )
            .unwrap();

        let content = std::fs::read_to_string(OUTPUT_FILE).unwrap();
        std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
    }
}
