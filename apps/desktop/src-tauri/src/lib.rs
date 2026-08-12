mod agents;
mod appearance;
mod commands;
mod db;
mod embedded_cli;
mod ext;
mod search_index;
mod store;
mod supervisor;

use db::{cloudsync_runtime_config_from_env, open_desktop_db};
use ext::*;
use store::*;

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use tauri::{Emitter, Manager};
use tauri_plugin_permissions::{Permission, PermissionsPluginExt};
use tauri_plugin_windows::AppWindow;

#[cfg(any(feature = "dev", feature = "devtools"))]
const STAGING_BUNDLE_ID: &str = "com.hyprnote.staging";

const APP_EXIT_REQUESTED_EVENT: &str = "app-exit-requested";
static EXIT_FLUSH_COMPLETE: AtomicBool = AtomicBool::new(false);
static EXIT_FLUSH_REQUESTED: AtomicBool = AtomicBool::new(false);
static CRASH_REPORTING_ENABLED: AtomicBool = AtomicBool::new(true);
const EXIT_FLUSH_FALLBACK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const EXIT_HARD_FALLBACK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(12);

pub(crate) struct CrashReportingState {
    client: Option<sentry::Client>,
    minidump: Mutex<Option<tauri_plugin_sentry::minidump::Handle>>,
}

impl CrashReportingState {
    fn new(client: Option<sentry::Client>, enabled: bool) -> Self {
        CRASH_REPORTING_ENABLED.store(enabled, Ordering::SeqCst);
        let minidump = enabled
            .then(|| client.as_ref().and_then(start_minidump_reporting))
            .flatten();
        Self {
            client,
            minidump: Mutex::new(minidump),
        }
    }

    fn set_enabled(&self, enabled: bool) {
        CRASH_REPORTING_ENABLED.store(enabled, Ordering::SeqCst);
        let mut minidump = self.minidump.lock().unwrap();
        if enabled {
            if minidump.is_none() {
                *minidump = self.client.as_ref().and_then(start_minidump_reporting);
            }
        } else {
            minidump.take();
        }
    }
}

fn start_minidump_reporting(
    client: &sentry::Client,
) -> Option<tauri_plugin_sentry::minidump::Handle> {
    match tauri_plugin_sentry::minidump::init(client) {
        Ok(handle) => Some(handle),
        Err(error) => {
            tracing::warn!(%error, "failed to initialize Sentry minidump reporting");
            None
        }
    }
}

async fn load_crash_reporting_consent(db: &anlg_db_core::Db) -> bool {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT id, value_json FROM app_settings \
         WHERE id IN ('crash_reporting_consent', 'telemetry_consent')",
    )
    .fetch_all(db.pool())
    .await
    .unwrap_or_default();

    crash_reporting_consent_from_rows(&rows)
}

fn crash_reporting_consent_from_rows(rows: &[(String, String)]) -> bool {
    let read = |id: &str| {
        rows.iter()
            .find(|(key, _)| key == id)
            .and_then(|(_, value)| serde_json::from_str::<bool>(value).ok())
    };

    read("crash_reporting_consent")
        .or_else(|| read("telemetry_consent"))
        .unwrap_or(true)
}

fn mark_exit_flush_complete() {
    EXIT_FLUSH_COMPLETE.store(true, Ordering::SeqCst);
}

fn start_exit_hard_fallback() {
    std::thread::spawn(|| {
        std::thread::sleep(EXIT_HARD_FALLBACK_TIMEOUT);
        std::process::exit(0);
    });
}

fn should_force_quit() -> bool {
    #[cfg(target_os = "macos")]
    {
        return anlg_intercept::should_force_quit();
    }

    #[cfg(not(target_os = "macos"))]
    false
}

fn versions_indicate_update(previous: Option<&str>, current: Option<&str>) -> bool {
    matches!(
        (previous, current),
        (Some(previous), Some(current)) if !previous.is_empty() && previous != current
    )
}

fn should_recenter_after_update(app: &tauri::AppHandle<tauri::Wry>) -> bool {
    use tauri_plugin_updater2::Updater2PluginExt;

    let previous = match app.updater2().get_last_seen_version() {
        Ok(previous) => previous,
        Err(error) => {
            tracing::warn!(%error, "failed to read the previous app version during startup");
            return false;
        }
    };

    versions_indicate_update(previous.as_deref(), app.config().version.as_deref())
}

fn create_audio_provider(_bundle_id: &str) -> std::sync::Arc<dyn anlg_audio_actual::AudioProvider> {
    #[cfg(any(feature = "dev", feature = "devtools"))]
    {
        let bundle_id = _bundle_id;
        let selection: u32 = std::env::var("MOCK_AUDIO")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let mock_audio_allowed = cfg!(feature = "dev") || bundle_id == STAGING_BUNDLE_ID;

        if mock_audio_allowed && selection > 0 {
            return std::sync::Arc::new(anlg_audio_mock::MockAudio::new(selection));
        }
    }
    std::sync::Arc::new(anlg_audio_actual::ActualAudio)
}

#[tokio::main]
pub async fn main() {
    tauri::async_runtime::set(tokio::runtime::Handle::current());
    let context = tauri::generate_context!();

    let (root_supervisor_ctx, root_supervisor_handle) =
        match supervisor::spawn_root_supervisor().await {
            Some((ctx, handle)) => (Some(ctx), Some(handle)),
            None => (None, None),
        };

    let db = match open_desktop_db(&context.config().identifier).await {
        Ok(db) => db,
        Err(error) => exit_after_startup_failure(&error),
    };
    let crash_reporting_enabled = load_crash_reporting_consent(&db).await;

    let sentry_client = {
        let dsn = if std::env::var_os("ANARLOG_DISABLE_SENTRY").is_some() {
            None
        } else {
            option_env!("SENTRY_DSN")
        };

        if let Some(dsn) = dsn {
            let release =
                option_env!("APP_VERSION").map(|v| format!("anarlog-desktop@{}", v).into());

            let client = sentry::init((
                dsn,
                sentry::ClientOptions {
                    release,
                    traces_sample_rate: 1.0,
                    auto_session_tracking: false,
                    before_send: Some(Arc::new(|event| {
                        CRASH_REPORTING_ENABLED
                            .load(Ordering::SeqCst)
                            .then(|| tauri_plugin_tracing::redaction::sanitize_sentry_event(event))
                            .flatten()
                    })),
                    before_breadcrumb: Some(Arc::new(|breadcrumb| {
                        CRASH_REPORTING_ENABLED
                            .load(Ordering::SeqCst)
                            .then_some(breadcrumb)
                    })),
                    ..Default::default()
                },
            ));

            sentry::configure_scope(|scope| {
                scope.set_tag("service.namespace", "anarlog");
                scope.set_tag("service.name", "desktop");
                scope.set_tag("enduser.pseudo.id", anlg_host::fingerprint());
                scope.set_user(Some(sentry::User {
                    id: Some(anlg_host::fingerprint()),
                    ..Default::default()
                }));
            });

            Some(client)
        } else {
            None
        }
    };
    let crash_reporting_state =
        CrashReportingState::new(sentry_client.as_deref().cloned(), crash_reporting_enabled);

    let audio: std::sync::Arc<dyn anlg_audio_actual::AudioProvider> =
        create_audio_provider(&context.config().identifier);
    let cloudsync_config = match cloudsync_runtime_config_from_env() {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "invalid CloudSync environment configuration; CloudSync disabled");
            None
        }
    };

    let mut builder = tauri_plugin_windows::extend_builder(tauri::Builder::default())
        .manage(audio)
        .manage(db.clone())
        .manage(crash_reporting_state);

    // https://docs.crabnebula.dev/plugins/tauri-e2e-tests/#macos-support
    #[cfg(all(target_os = "macos", feature = "automation"))]
    {
        builder = builder.plugin(tauri_plugin_automation::init());
    }

    // https://v2.tauri.app/plugin/deep-linking/#desktop
    // should always be the first plugin
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));
    }

    builder = builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_opener2::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_tracing::init())
        .plugin(tauri_plugin_analytics::init())
        .plugin(tauri_plugin_attachment_sync::init())
        .plugin(tauri_plugin_agent::init())
        .plugin(tauri_plugin_db::init_with_cloudsync(
            db.clone(),
            cloudsync_config,
        ))
        .plugin(tauri_plugin_bedrock::init());

    builder = builder
        .plugin(tauri_plugin_importer::init())
        .plugin(tauri_plugin_calendar::init())
        .plugin(tauri_plugin_todo::init())
        .plugin(tauri_plugin_auth::init())
        .plugin(tauri_plugin_hooks::init())
        .plugin(tauri_plugin_icon::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_sidecar2::init())
        .plugin(tauri_plugin_permissions::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_deeplink2::init())
        .plugin(tauri_plugin_fs_sync::init())
        .plugin(tauri_plugin_fs2::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_path2::init())
        .plugin(tauri_plugin_export::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_local_api::init())
        .plugin(tauri_plugin_mcp::init())
        .plugin(tauri_plugin_messenger::init())
        .plugin(tauri_plugin_misc::init())
        .plugin(tauri_plugin_template::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_detect::init())
        .plugin(tauri_plugin_dock::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_notify::init())
        .plugin(tauri_plugin_overlay::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_store2::init())
        .plugin(tauri_plugin_updater2::init())
        .plugin(tauri_plugin_tray::init())
        .plugin(tauri_plugin_settings::init())
        .plugin(tauri_plugin_sfx::init())
        .plugin(tauri_plugin_shortcut::init())
        .plugin(tauri_plugin_dictation::init())
        .plugin(tauri_plugin_windows::init())
        .plugin(tauri_plugin_js::init())
        .plugin(tauri_plugin_flag::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .skip_initial_state("main")
                .build(),
        )
        .plugin(tauri_plugin_transcription::init())
        .plugin(tauri_plugin_tantivy::init())
        .plugin(tauri_plugin_audio_priority::init())
        .plugin(tauri_plugin_local_llm::init())
        .plugin(tauri_plugin_local_stt::init(
            tauri_plugin_local_stt::InitOptions {
                parent_supervisor: root_supervisor_ctx
                    .as_ref()
                    .map(|ctx| ctx.supervisor.get_cell()),
            },
        ))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ));

    if let Some(client) = sentry_client.as_ref() {
        builder = builder.plugin(tauri_plugin_sentry::init_with_no_injection(client));
    }

    #[cfg(any(debug_assertions, feature = "devtools"))]
    {
        builder = builder.plugin(tauri_plugin_relay::init());
    }

    #[cfg(all(not(debug_assertions), not(feature = "devtools")))]
    {
        let plugin = tauri_plugin_prevent_default::init();
        builder = builder.plugin(plugin);
    }

    #[cfg(target_os = "macos")]
    {
        builder = builder.menu(tauri_plugin_tray::build_app_menu);
    }

    let specta_builder = make_specta_builder::<tauri::Wry>();

    let root_supervisor_ctx_for_run = root_supervisor_ctx.clone();

    let app_result = builder
        .invoke_handler(specta_builder.invoke_handler())
        .on_window_event(tauri_plugin_windows::on_window_event)
        .setup(move |app| {
            let app_handle = app.handle().clone();

            specta_builder.mount_events(&app_handle);

            #[cfg(any(windows, target_os = "linux"))]
            {
                // https://v2.tauri.app/ko/plugin/deep-linking/#desktop-1
                // Registration shells out to update-desktop-database/xdg-mime on Linux,
                // which are missing on NixOS; failing setup here panics the app.
                use tauri_plugin_deep_link::DeepLinkExt;
                if let Err(error) = app.deep_link().register_all() {
                    tracing::warn!(%error, "failed to register deep link handlers");
                }
            }

            {
                use tauri_plugin_tray::TrayPluginExt;
                use tauri_plugin_windows::WindowsPluginExt;

                let appearance_settings =
                    appearance::load_app_appearance_settings::<tauri::Wry, _>(&app_handle);

                if let Err(error) = app_handle
                    .windows()
                    .set_show_app_in_dock(appearance_settings.show_app_in_dock)
                {
                    tracing::warn!(%error, "failed to apply dock visibility during startup");
                }

                if appearance_settings.show_tray_icon {
                    if let Err(error) = app_handle.tray().create_tray_menu() {
                        tracing::warn!(%error, "failed to create tray menu during startup");
                    }
                }
            }

            {
                use tauri_plugin_tray::AnlgMenuItem;
                app_handle.on_menu_event(|app, event| {
                    if let Ok(item) = AnlgMenuItem::try_from(event.id().clone()) {
                        item.handle(app);
                    } else {
                        tauri_plugin_tray::handle_agenda_menu_event(app, event.id());
                    }
                });
            }

            {
                use tauri_plugin_settings::SettingsPluginExt;
                if let Ok(base) = app_handle.settings().vault_base()
                    && let Err(e) = agents::write_agents_file(base.as_std_path())
                {
                    tracing::error!("failed to write AGENTS.md: {}", e);
                }
            }

            if let (Some(ctx), Some(handle)) = (&root_supervisor_ctx, root_supervisor_handle) {
                supervisor::monitor_supervisor(handle, ctx.is_exiting.clone(), app_handle.clone());
            }

            {
                use tauri_plugin_local_llm::LocalLlmPluginExt;
                if false {
                    app_handle.local_llm().start_server();
                }
            }

            search_index::spawn(app_handle, db.clone());

            Ok(())
        })
        .build(context);

    let app = match app_result {
        Ok(app) => app,
        Err(error) => exit_after_startup_failure(&error),
    };

    match get_onboarding_flag() {
        None => {}
        Some(false) => {
            if let Err(error) = app.set_onboarding_needed(false) {
                tracing::warn!(%error, "failed to persist onboarding state during startup");
            }
        }
        Some(true) => {
            use tauri_plugin_auth::AuthPluginExt;
            use tauri_plugin_settings::SettingsPluginExt;
            use tauri_plugin_store2::Store2PluginExt;

            let _ = app.clear_auth();
            let _ = app.settings().reset();
            let _ = app.store2().reset();
            let _ = app.set_onboarding_needed(true);

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let permissions = app_handle.permissions();
                let _ = permissions.reset(Permission::Microphone).await;
                let _ = permissions.reset(Permission::SystemAudio).await;
                let _ = permissions.reset(Permission::ScreenRecording).await;
                let _ = permissions.reset(Permission::Accessibility).await;
                let _ = permissions.reset(Permission::Calendar).await;
                let _ = permissions.reset(Permission::Reminders).await;
            });
        }
    }

    {
        let app_handle = app.handle().clone();
        let recenter_after_update = should_recenter_after_update(&app_handle);
        match AppWindow::Main.show(&app_handle) {
            Ok(window) if recenter_after_update => {
                if let Err(error) = AppWindow::Main.center_on_primary(&app_handle, &window) {
                    tracing::warn!(%error, "failed to recenter the main window after an update");
                }
            }
            Ok(_) => {}
            Err(error) => exit_after_startup_failure(&error),
        }
    }

    #[cfg(target_os = "macos")]
    anlg_intercept::setup_force_quit_handler();

    #[allow(unused_variables)]
    app.run(move |app, event| match event {
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { .. } => {
            if let Err(error) = AppWindow::Main.show(app) {
                tracing::error!(%error, "failed to reopen main window");
            }
        }
        tauri::RunEvent::ExitRequested { api, .. } => {
            if let Some(ref ctx) = root_supervisor_ctx_for_run {
                ctx.mark_exiting();
            }

            if EXIT_FLUSH_COMPLETE.load(Ordering::SeqCst) || should_force_quit() {
                return;
            }

            api.prevent_exit();
            let first_request = !EXIT_FLUSH_REQUESTED.swap(true, Ordering::SeqCst);
            if first_request {
                start_exit_hard_fallback();
            }
            if app.emit_to("main", APP_EXIT_REQUESTED_EVENT, ()).is_err() {
                mark_exit_flush_complete();
                app.exit(0);
            } else if first_request {
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(EXIT_FLUSH_FALLBACK_TIMEOUT).await;
                    if !EXIT_FLUSH_COMPLETE.swap(true, Ordering::SeqCst) {
                        tracing::warn!(
                            "forcing app exit after frontend flush acknowledgement timed out"
                        );
                        app_handle.exit(0);
                    }
                });
            }
        }
        tauri::RunEvent::Exit => {
            if let Some(ref ctx) = root_supervisor_ctx_for_run {
                ctx.mark_exiting();
                ctx.stop();
            }

            anlg_host::kill_processes_by_matcher(anlg_host::ProcessMatcher::Sidecar);
        }
        _ => {}
    });
}

fn startup_failure_message(error: &impl std::fmt::Display) -> String {
    format!("Anarlog failed to start: {error}")
}

fn exit_after_startup_failure(error: &impl std::fmt::Display) -> ! {
    let message = startup_failure_message(error);
    eprintln!("{message}");
    tracing::error!(error = %error, "desktop startup failed");
    sentry::capture_message(&message, sentry::Level::Error);

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("/usr/bin/osascript")
            .args([
                "-e",
                "display alert \"Anarlog could not start\" message \"Your existing data was left unchanged. Please restart the app. If the problem continues, contact support.\" as critical buttons {\"OK\"} default button \"OK\"",
            ])
            .spawn();
    }

    std::process::exit(1);
}

fn get_onboarding_flag() -> Option<bool> {
    let parse_value = |v: &str| -> Option<bool> {
        match v {
            "1" | "true" => Some(true),
            "0" | "false" => Some(false),
            _ => {
                if let Ok(timestamp) = v.parse::<u64>() {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .ok()?
                        .as_millis() as u64;
                    let elapsed = now.saturating_sub(timestamp * 1000);
                    if elapsed < 2500 { Some(true) } else { None }
                } else {
                    None
                }
            }
        }
    };

    pico_args::Arguments::from_env()
        .opt_value_from_str::<_, String>("--onboarding")
        .ok()
        .flatten()
        .and_then(|v| parse_value(&v))
        .or_else(|| {
            std::env::var("ONBOARDING")
                .ok()
                .and_then(|v| parse_value(&v))
        })
}

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .commands(tauri_specta::collect_commands![
            commands::get_onboarding_needed::<tauri::Wry>,
            commands::set_onboarding_needed::<tauri::Wry>,
            commands::get_dismissed_toasts::<tauri::Wry>,
            commands::set_dismissed_toasts::<tauri::Wry>,
            commands::get_env::<tauri::Wry>,
            commands::show_devtool::<tauri::Wry>,
            commands::complete_app_exit::<tauri::Wry>,
            commands::get_tinybase_values::<tauri::Wry>,
            commands::get_pinned_tabs::<tauri::Wry>,
            commands::set_pinned_tabs::<tauri::Wry>,
            commands::get_recently_opened_sessions::<tauri::Wry>,
            commands::set_recently_opened_sessions::<tauri::Wry>,
            commands::is_crash_reporting_enabled,
            commands::set_crash_reporting_enabled,
            commands::check_embedded_cli::<tauri::Wry>,
            commands::install_embedded_cli::<tauri::Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn startup_failure_message_includes_the_original_error() {
        let message = startup_failure_message(&"legacy import did not pass parity verification");

        assert_eq!(
            message,
            "Anarlog failed to start: legacy import did not pass parity verification"
        );
    }

    #[test]
    fn recenters_after_the_app_version_changes() {
        assert!(versions_indicate_update(Some("1.4.7"), Some("1.4.8")));
        assert!(!versions_indicate_update(Some("1.4.8"), Some("1.4.8")));
        assert!(!versions_indicate_update(None, Some("1.4.8")));
        assert!(!versions_indicate_update(Some(""), Some("1.4.8")));
        assert!(!versions_indicate_update(Some("1.4.7"), None));
    }

    #[test]
    fn crash_reporting_uses_new_consent_before_legacy_telemetry_consent() {
        let rows = vec![
            ("telemetry_consent".to_string(), "false".to_string()),
            ("crash_reporting_consent".to_string(), "true".to_string()),
        ];

        assert!(crash_reporting_consent_from_rows(&rows));
        assert!(!crash_reporting_consent_from_rows(&rows[..1]));
        assert!(crash_reporting_consent_from_rows(&[]));
    }

    #[test]
    fn main_capability_allows_cloudsync_lifecycle_commands() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        let permissions = capability["permissions"].as_array().unwrap();

        for expected in [
            "db:allow-begin-cloudsync-activity",
            "db:allow-end-cloudsync-activity",
            "db:allow-sync-cloudsync-now",
        ] {
            assert!(
                permissions.iter().any(|permission| permission == expected),
                "missing permission: {expected}"
            );
        }
    }

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "../src/types/tauri.gen.ts";

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
