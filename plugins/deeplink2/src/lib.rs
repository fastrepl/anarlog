mod commands;
mod pending_deep_link;
mod pending_share_open;
pub mod server;
mod types;

#[cfg(test)]
mod docs;

pub use anlg_deeplink_core::{Error, Result};
pub use types::{
    AuthCallbackSearch, BillingRefreshSearch, DeepLink, DeepLinkEvent, IntegrationCallbackSearch,
    OnboardingDemoCompleteSearch, ShareOpenPendingEvent, ShareOpenRequest,
};

use std::str::FromStr;

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_specta::Event;

const PLUGIN_NAME: &str = "deeplink2";

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

#[derive(Debug)]
pub(crate) enum Classified {
    DeepLink(DeepLink),
    ShareOpen(ShareOpenRequest),
    Invalid(anlg_deeplink_core::Error),
}

pub(crate) fn classify(url: &str) -> Classified {
    match anlg_deeplink_core::IncomingDeepLink::from_str(url) {
        Ok(anlg_deeplink_core::IncomingDeepLink::Existing(deep_link)) => {
            Classified::DeepLink(deep_link)
        }
        Ok(anlg_deeplink_core::IncomingDeepLink::ShareOpen(request)) => {
            Classified::ShareOpen(request)
        }
        Err(error) => Classified::Invalid(error),
    }
}

fn process_url<R: Runtime>(app_handle: &AppHandle<R>, url: &url::Url, delivery: Delivery) {
    let url_str = url.as_str();
    let redacted = anlg_deeplink_core::redact_url(url_str);
    tracing::info!(url = %redacted, "deeplink_received");

    match classify(url_str) {
        Classified::DeepLink(deep_link) => {
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
        Classified::ShareOpen(request) => {
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
        Classified::Invalid(error) => {
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

#[cfg(test)]
mod contract_tests {
    use super::{Classified, classify};
    use anlg_deeplink_core::contract::deeplink_cases;

    #[test]
    fn adapter_matches_deeplink_contract() {
        for case in deeplink_cases() {
            match (classify(&case.url), case.expect.kind.as_str()) {
                (Classified::DeepLink(deep_link), "deep_link") => {
                    assert_eq!(
                        Some(deep_link.path()),
                        case.expect.path.as_deref(),
                        "{}",
                        case.name
                    );
                }
                (Classified::ShareOpen(_), "share_open") => {
                    assert_eq!(
                        case.expect.path.as_deref(),
                        Some("/share/open"),
                        "{}",
                        case.name
                    );
                }
                (Classified::Invalid(_), "invalid") => {}
                (classified, expected) => {
                    panic!("{}: expected {expected}, got {classified:?}", case.name)
                }
            }
        }
    }
}
