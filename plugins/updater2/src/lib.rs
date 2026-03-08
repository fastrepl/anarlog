mod commands;
mod error;
mod events;
mod ext;
mod store;

pub use error::{Error, Result};
pub use events::*;
pub use ext::*;
pub(crate) use store::*;

const PLUGIN_NAME: &str = "updater2";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::check::<tauri::Wry>,
            commands::download::<tauri::Wry>,
            commands::install::<tauri::Wry>,
            commands::maybe_emit_updated::<tauri::Wry>,
        ])
        .events(tauri_specta::collect_events![
            events::UpdateDownloadingEvent,
            events::UpdateDownloadFailedEvent,
            events::UpdateReadyEvent,
            events::UpdatedEvent,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);

            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    check_and_download(&handle).await;
                    tokio::time::sleep(std::time::Duration::from_secs(30 * 60)).await;
                }
            });

            Ok(())
        })
        .build()
}

async fn check_and_download<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if cfg!(debug_assertions) {
        return;
    }

    let updater2 = app.updater2();

    let version = match updater2.check().await {
        Ok(Some(v)) => v,
        Ok(None) => return,
        Err(e) => {
            tracing::error!("update_check_failed: {}", e);
            return;
        }
    };

    if let Err(e) = updater2.download(&version).await {
        tracing::error!("update_download_failed: {}", e);
    }
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
