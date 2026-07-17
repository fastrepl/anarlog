mod commands;
mod error;
mod pending_deep_link;
mod pending_share_open;
pub mod server;
mod types;

#[cfg(test)]
mod docs;

pub use error::{Error, Result};
pub use types::{
    AuthCallbackSearch, BillingRefreshSearch, DeepLink, DeepLinkEvent, IntegrationCallbackSearch,
    ShareOpenPendingEvent, ShareOpenRequest,
};

use std::str::FromStr;

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_specta::Event;

const PLUGIN_NAME: &str = "deeplink2";

fn redact_url(url_str: &str) -> String {
    match url::Url::parse(url_str) {
        Ok(parsed) => {
            let scheme = parsed.scheme();
            let host = parsed.host_str().unwrap_or("");
            let path = parsed.path();
            format!("{}://{}{}", scheme, host, path)
        }
        Err(_) => "[invalid_url]".to_string(),
    }
}

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::start_callback_server::<tauri::Wry>,
            commands::stop_callback_server::<tauri::Wry>,
            commands::take_pending_deep_links,
            commands::list_pending_share_opens,
            commands::take_pending_share_open,
        ])
        .events(tauri_specta::collect_events![
            types::DeepLinkEvent,
            types::ShareOpenPendingEvent
        ])
        .typ::<types::DeepLink>()
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

#[derive(Clone, Copy)]
enum Delivery {
    Emit,
    Queue,
}

fn process_url<R: Runtime>(app_handle: &AppHandle<R>, url: &url::Url, delivery: Delivery) {
    let url_str = url.as_str();
    let redacted = redact_url(url_str);
    tracing::info!(url = %redacted, "deeplink_received");

    match types::IncomingDeepLink::from_str(url_str) {
        Ok(types::IncomingDeepLink::Existing(deep_link)) => {
            tracing::info!(path = deep_link.path(), "deeplink_parsed");
            match delivery {
                Delivery::Emit => {
                    if let Err(error) = DeepLinkEvent(deep_link).emit(app_handle) {
                        tracing::error!(?error, "deeplink_event_emit_failed");
                    }
                }
                Delivery::Queue => {
                    if app_handle
                        .state::<pending_deep_link::PendingDeepLinkState>()
                        .push(deep_link)
                        .is_err()
                    {
                        tracing::error!("pending_deep_link_queue_unavailable");
                    }
                }
            }
        }
        Ok(types::IncomingDeepLink::ShareOpen(request)) => {
            let state = app_handle.state::<pending_share_open::PendingShareOpenState>();
            match state.push(request) {
                Ok(pending_id) => {
                    tracing::info!(path = "/share/open", "deeplink_parsed");
                    if matches!(delivery, Delivery::Emit)
                        && let Err(error) =
                            (types::ShareOpenPendingEvent { pending_id }).emit(app_handle)
                    {
                        tracing::error!(?error, "deeplink_event_emit_failed");
                    }
                }
                Err(()) => {
                    tracing::error!("pending_share_open_queue_unavailable");
                }
            }
        }
        Err(error) => {
            tracing::debug!(?error, url = %redacted, "deeplink_parse_failed");
        }
    }
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);
            app.manage(server::CallbackServerState::new());
            app.manage(pending_deep_link::PendingDeepLinkState::default());
            app.manage(pending_share_open::PendingShareOpenState::default());

            let app_handle = app.clone();
            let startup_app_handle = app_handle.clone();

            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    process_url(&app_handle, &url, Delivery::Emit);
                }
            });

            match app.deep_link().get_current() {
                Ok(Some(urls)) => {
                    for url in urls {
                        process_url(&startup_app_handle, &url, Delivery::Queue);
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::error!(?error, "deeplink_current_read_failed");
                }
            }

            Ok(())
        })
        .build()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export() {
        export_types();
        export_docs();
    }

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

    #[test]
    fn redacts_query_and_fragment_from_logged_urls() {
        let value = redact_url(
            "hyprnote://share/open?mode=handoff&request_id=ba5ca57a-8f88-44e8-ab92-f9e10c89425c#secret",
        );
        assert_eq!(value, "hyprnote://share/open");
    }

    fn export_docs() {
        let source_code = std::fs::read_to_string("./js/bindings.gen.ts").unwrap();
        let deeplinks = docs::parse_deeplinks(&source_code).unwrap();
        assert!(!deeplinks.is_empty());

        let output_dir = std::path::Path::new("../../apps/web/content/deeplinks");
        std::fs::create_dir_all(output_dir).unwrap();

        for deeplink in &deeplinks {
            let filepath = output_dir.join(deeplink.doc_path());
            let content = deeplink.doc_render();
            std::fs::write(&filepath, content).unwrap();
        }
    }
}
