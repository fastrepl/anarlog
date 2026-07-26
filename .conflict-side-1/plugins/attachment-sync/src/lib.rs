mod commands;
mod control;
mod error;
mod models;
mod runtime;

use tauri::Manager;

pub use error::{Error, Result};
pub use models::*;

const PLUGIN_NAME: &str = "attachment-sync";

pub(crate) fn configured_supabase_url() -> Option<&'static str> {
    #[cfg(not(debug_assertions))]
    {
        option_env!("VITE_SUPABASE_URL").filter(|value| !value.trim().is_empty())
    }

    #[cfg(debug_assertions)]
    {
        Some(
            option_env!("VITE_SUPABASE_URL")
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("http://127.0.0.1:54321"),
        )
    }
}

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::describe_upload,
            commands::prepare_upload::<tauri::Wry>,
            commands::read_upload_range::<tauri::Wry>,
            commands::begin_shared_upload_operation,
            commands::cancel_shared_upload_operation,
            commands::prepare_shared_upload::<tauri::Wry>,
            commands::read_shared_upload_range::<tauri::Wry>,
            commands::validate_shared_upload::<tauri::Wry>,
            commands::cleanup_shared_upload::<tauri::Wry>,
            commands::prepare_delete_guard::<tauri::Wry>,
            commands::commit_delete_guard::<tauri::Wry>,
            commands::reconcile_delete_guards::<tauri::Wry>,
            commands::begin_attachment_download,
            commands::cancel_attachment_download,
            commands::download_and_restore::<tauri::Wry>,
            commands::cleanup_transfer_cache::<tauri::Wry>,
            commands::download_shared_attachment::<tauri::Wry>,
            commands::shared_attachment_path::<tauri::Wry>,
            commands::remove_shared_attachment::<tauri::Wry>,
            commands::clear_shared_attachment_scope::<tauri::Wry>,
            commands::clear_shared_attachment_preview_scopes::<tauri::Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();
    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(|app, _api| {
            app.manage(control::DownloadControl::default());
            runtime::clear_private_attachment_cache_root(app)?;
            runtime::clear_shared_upload_cache_root(app)?;
            runtime::clear_shared_attachment_cache_root(app)?;
            runtime::clear_shared_attachment_preview_cache_root(app)?;
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn configured_origin_uses_only_compile_time_configuration() {
        let configured = option_env!("VITE_SUPABASE_URL").filter(|value| !value.trim().is_empty());

        #[cfg(not(debug_assertions))]
        assert_eq!(configured_supabase_url(), configured);

        #[cfg(debug_assertions)]
        assert_eq!(
            configured_supabase_url(),
            Some(configured.unwrap_or("http://127.0.0.1:54321"))
        );
    }

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
        let content = content.trim_start_matches("// @ts-nocheck\n");
        std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
    }
}
