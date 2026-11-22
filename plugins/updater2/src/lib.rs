mod commands;
#[cfg(target_os = "linux")]
mod deb;
mod error;
mod events;
mod ext;
#[cfg(target_os = "macos")]
mod startup_migration;
mod store;

pub use error::{Error, Result};
pub use events::*;
pub use ext::*;
pub(crate) use store::*;

use tauri::Manager;

const PLUGIN_NAME: &str = "updater2";
const WEBVIEW_STORAGE_MIRROR_WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::check::<tauri::Wry>,
            commands::download::<tauri::Wry>,
            commands::install_and_relaunch::<tauri::Wry>,
            commands::is_downloaded::<tauri::Wry>,
            commands::set_automatic_updates_enabled::<tauri::Wry>,
            commands::set_meeting_active::<tauri::Wry>,
            commands::maybe_emit_updated::<tauri::Wry>,
        ])
        .events(tauri_specta::collect_events![
            events::UpdateAvailableEvent,
            events::UpdateDownloadingEvent,
            events::UpdateDownloadProgressEvent,
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
            app.manage(UpdaterState::new());

            #[cfg(target_os = "macos")]
            match startup_migration::maybe_schedule_legacy_bundle_rename_on_launch(app) {
                Ok(true) => std::process::exit(0),
                Ok(false) => {}
                Err(err) => tracing::error!("failed to schedule legacy bundle rename: {}", err),
            }

            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                let mut install_at_open = true;
                loop {
                    install_at_open = check_and_download(&handle, install_at_open).await;
                    tokio::time::sleep(std::time::Duration::from_secs(30 * 60)).await;
                }
            });

            Ok(())
        })
        .build()
}

// Takes the install-at-open intent and returns it for the next tick. A
// completed pass installs (which restarts the app) and consumes the intent;
// a meeting deferral or a transient check/download/install failure (e.g.
// network not up yet at login) preserves it so a later tick can still install.
async fn check_and_download<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    install_at_open: bool,
) -> bool {
    if cfg!(debug_assertions) {
        return false;
    }

    let updater2 = app.updater2();

    match updater2.automatic_updates_enabled() {
        Ok(true) => {}
        Ok(false) => return false,
        Err(e) => {
            tracing::error!("automatic_update_policy_read_failed: {}", e);
            return false;
        }
    }

    if let Some(unfinished) = updater2.unfinished_vault_move() {
        tracing::warn!(
            from = %unfinished.from.display(),
            to = %unfinished.to.display(),
            "automatic_update_withheld_unfinished_vault_move"
        );
        return false;
    }

    // Never install (which restarts the app) or download while a meeting is
    // being recorded; the next 30-minute tick retries after the meeting.
    if updater2.meeting_active() {
        tracing::info!("automatic_update_deferred_meeting_active");
        return install_at_open;
    }

    let version = match updater2.check().await {
        Ok(Some(v)) => v,
        Ok(None) => return false,
        Err(e) => {
            tracing::error!("update_check_failed: {}", e);
            return install_at_open;
        }
    };

    // A meeting may have started while the check was in flight.
    if updater2.meeting_active() {
        tracing::info!("automatic_update_deferred_meeting_active");
        return install_at_open;
    }

    // install_and_relaunch re-checks the release feed before applying the bin,
    // so it can fail transiently just like check/download; keep the intent so
    // a later tick retries instead of disabling auto-install for the session.
    if install_at_open && updater2.has_cached_update(&version) {
        if !wait_ready_to_auto_install(&updater2).await {
            return true;
        }
        if let Err(e) = updater2.install_and_relaunch(&version).await {
            tracing::error!("cached_update_install_failed: {}", e);
            return true;
        }
        return false;
    }

    if let Err(e) = updater2.download(&version).await {
        tracing::error!("update_download_failed: {}", e);
        return install_at_open;
    }

    // With frequent releases the cached version is rarely still the latest by
    // the next open, so deferring the install to the next session would
    // re-download forever and never install anything. Install as soon as the
    // at-open download completes instead.
    if install_at_open {
        if !wait_ready_to_auto_install(&updater2).await {
            return true;
        }
        if let Err(e) = updater2.install_and_relaunch(&version).await {
            tracing::error!("downloaded_update_install_failed: {}", e);
            return true;
        }
    }

    false
}

async fn wait_ready_to_auto_install<R: tauri::Runtime, M: tauri::Manager<R>>(
    updater2: &Updater2<'_, R, M>,
) -> bool {
    wait_ready_to_auto_install_within(updater2, WEBVIEW_STORAGE_MIRROR_WAIT_TIMEOUT).await
}

async fn wait_ready_to_auto_install_within<R: tauri::Runtime, M: tauri::Manager<R>>(
    updater2: &Updater2<'_, R, M>,
    mirror_wait_timeout: std::time::Duration,
) -> bool {
    if updater2.meeting_active() {
        tracing::info!("automatic_update_deferred_meeting_active");
        return false;
    }

    if !updater2
        .wait_for_webview_storage_mirror(mirror_wait_timeout)
        .await
    {
        tracing::info!("automatic_update_deferred_webview_storage_not_mirrored");
        return false;
    }

    // A meeting may have started while waiting for the WebView to mirror localStorage.
    if updater2.meeting_active() {
        tracing::info!("automatic_update_deferred_meeting_active");
        return false;
    }

    true
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

    #[tokio::test]
    async fn auto_install_waits_for_webview_storage_mirror_and_defers_on_timeout() {
        let app = tauri::test::mock_app();
        app.manage(UpdaterState::new());
        let updater2 = app.updater2();

        assert!(
            !wait_ready_to_auto_install_within(&updater2, std::time::Duration::from_millis(25))
                .await
        );

        let handle = app.handle().clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(15)).await;
            handle.updater2().mark_webview_storage_mirrored();
        });

        assert!(
            wait_ready_to_auto_install_within(&updater2, std::time::Duration::from_secs(2)).await
        );
    }
}
