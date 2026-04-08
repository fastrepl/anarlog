mod agents;
mod commands;
mod ext;
mod store;
mod supervisor;

use ext::*;
use store::*;

use tauri::Manager;
use tauri_plugin_permissions::{Permission, PermissionsPluginExt};
use tauri_plugin_windows::{AppWindow, WindowsPluginExt};

const STAGING_BUNDLE_ID: &str = "com.hyprnote.staging";

fn create_audio_provider(bundle_id: &str) -> std::sync::Arc<dyn hypr_audio_actual::AudioProvider> {
    #[cfg(any(feature = "dev", feature = "devtools"))]
    {
        let selection: u32 = std::env::var("MOCK_AUDIO")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let mock_audio_allowed = cfg!(feature = "dev") || bundle_id == STAGING_BUNDLE_ID;

        if mock_audio_allowed && selection > 0 {
            return std::sync::Arc::new(hypr_audio_mock::MockAudio::new(selection));
        }
    }
    std::sync::Arc::new(hypr_audio_actual::ActualAudio)
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

    let sentry_client = {
        let dsn = option_env!("SENTRY_DSN");

        if let Some(dsn) = dsn {
            let release =
                option_env!("APP_VERSION").map(|v| format!("hyprnote-desktop@{}", v).into());

            let client = sentry::init((
                dsn,
                sentry::ClientOptions {
                    release,
                    traces_sample_rate: 1.0,
                    auto_session_tracking: false,
                    ..Default::default()
                },
            ));

            sentry::configure_scope(|scope| {
                scope.set_tag("service.namespace", "hyprnote");
                scope.set_tag("service.name", "desktop");
                scope.set_tag("enduser.pseudo.id", hypr_host::fingerprint());
                scope.set_user(Some(sentry::User {
                    id: Some(hypr_host::fingerprint()),
                    ..Default::default()
                }));
            });

            Some(client)
        } else {
            None
        }
    };

    let _guard = sentry_client
        .as_ref()
        .map(|client| tauri_plugin_sentry::minidump::init(client));

    let audio: std::sync::Arc<dyn hypr_audio_actual::AudioProvider> =
        create_audio_provider(&context.config().identifier);

    let mut builder = tauri::Builder::default().manage(audio);

    // https://docs.crabnebula.dev/plugins/tauri-e2e-tests/#macos-support
    #[cfg(all(target_os = "macos", feature = "automation"))]
    {
        builder = builder.plugin(tauri_plugin_automation::init());
    }

    // https://v2.tauri.app/plugin/deep-linking/#desktop
    // should always be the first plugin
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            app.windows().show(AppWindow::Main).unwrap();
        }));
    }

    builder = builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_opener2::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_analytics::init())
        .plugin(tauri_plugin_agent::init())
        .plugin(tauri_plugin_activity_capture::init())
        .plugin(tauri_plugin_bedrock::init())
        .plugin(tauri_plugin_importer::init())
        .plugin(tauri_plugin_calendar::init())
        .plugin(tauri_plugin_todo::init())
        .plugin(tauri_plugin_auth::init())
        .plugin(tauri_plugin_db2::init())
        .plugin(tauri_plugin_tracing::init())
        .plugin(tauri_plugin_hooks::init())
        .plugin(tauri_plugin_icon::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_permissions::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_deeplink2::init())
        .plugin(tauri_plugin_fs_db::init())
        .plugin(tauri_plugin_fs_sync::init())
        .plugin(tauri_plugin_fs2::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_path2::init())
        .plugin(tauri_plugin_export::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_mcp::init())
        .plugin(tauri_plugin_misc::init())
        .plugin(tauri_plugin_template::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_detect::init())
        .plugin(tauri_plugin_dock::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_notify::init())
        .plugin(tauri_plugin_overlay::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_tray::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_store2::init())
        .plugin(tauri_plugin_settings::init())
        .plugin(tauri_plugin_sfx::init())
        .plugin(tauri_plugin_windows::init())
        .plugin(tauri_plugin_js::init())
        .plugin(tauri_plugin_flag::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
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
        .plugin(tauri_plugin_network::init(
            tauri_plugin_network::InitOptions {
                parent_supervisor: root_supervisor_ctx
                    .as_ref()
                    .map(|ctx| ctx.supervisor.get_cell()),
            },
        ))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .plugin(tauri_plugin_updater2::init());

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

    let specta_builder = make_specta_builder();

    let root_supervisor_ctx_for_run = root_supervisor_ctx.clone();

    let app = builder
        .invoke_handler(specta_builder.invoke_handler())
        .on_window_event(tauri_plugin_windows::on_window_event)
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let app_clone = app_handle.clone();

            specta_builder.mount_events(&app_handle);

            #[cfg(any(windows, target_os = "linux"))]
            {
                // https://v2.tauri.app/ko/plugin/deep-linking/#desktop-1
                use tauri_plugin_deep_link::DeepLinkExt;
                app.deep_link().register_all()?;
            }

            {
                use tauri_plugin_tray::TrayPluginExt;
                app_handle.tray().create_tray_menu().unwrap();
                app_handle.tray().create_app_menu().unwrap();
            }

            {
                use tauri_plugin_tray::HyprMenuItem;
                app_handle.on_menu_event(|app, event| {
                    if let Ok(item) = HyprMenuItem::try_from(event.id().clone()) {
                        item.handle(app);
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

            tokio::spawn(async move {
                use tauri_plugin_db2::Database2PluginExt;

                if let Err(e) = app_clone.db2().init_local().await {
                    tracing::error!("failed_to_init_local: {}", e);
                }
            });

            if let (Some(ctx), Some(handle)) = (&root_supervisor_ctx, root_supervisor_handle) {
                supervisor::monitor_supervisor(handle, ctx.is_exiting.clone(), app_handle.clone());
            }

            // control::setup(&app_handle);

            Ok(())
        })
        .build(context)
        .unwrap();

    match get_onboarding_flag() {
        None => {}
        Some(false) => app.set_onboarding_needed(false).unwrap(),
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
                let _ = permissions.reset(Permission::Accessibility).await;
                let _ = permissions.reset(Permission::Calendar).await;
            });
        }
    }

    {
        let app_handle = app.handle().clone();
        AppWindow::Main.show(&app_handle).unwrap();
    }

    #[cfg(target_os = "macos")]
    hypr_intercept::setup_force_quit_handler();

    #[allow(unused_variables)]
    app.run(move |app, event| match event {
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { .. } => {
            AppWindow::Main.show(app).unwrap();
        }
        #[cfg(target_os = "macos")]
        tauri::RunEvent::ExitRequested { api, .. } => {
            if hypr_intercept::should_force_quit() {
                return;
            }

            api.prevent_exit();

            for (_, window) in app.webview_windows() {
                let _ = window.close();
            }

            let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
        }
        tauri::RunEvent::Exit => {
            {
                use tauri_plugin_store2::Store2PluginExt;
                if let Ok(store) = app.store2().store() {
                    let _ = store.save();
                }
            }

            if let Some(ref ctx) = root_supervisor_ctx_for_run {
                ctx.mark_exiting();
                ctx.stop();
            }
            hypr_host::kill_processes_by_matcher(hypr_host::ProcessMatcher::Sidecar);
        }
        _ => {}
    });
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
            commands::show_devtool,
            commands::resize_window_for_chat::<tauri::Wry>,
            commands::resize_window_for_sidebar::<tauri::Wry>,
            commands::get_tinybase_values::<tauri::Wry>,
            commands::set_tinybase_values::<tauri::Wry>,
            commands::get_pinned_tabs::<tauri::Wry>,
            commands::set_pinned_tabs::<tauri::Wry>,
            commands::get_recently_opened_sessions::<tauri::Wry>,
            commands::set_recently_opened_sessions::<tauri::Wry>,
            commands::get_char_v1p1_preview::<tauri::Wry>,
            commands::set_char_v1p1_preview::<tauri::Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

#[cfg(test)]
mod test {
    use super::*;

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
