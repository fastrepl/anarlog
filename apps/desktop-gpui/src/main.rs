mod actions;
mod ai_health;
mod ai_models;
mod ai_providers;
mod ai_verify;
mod assets;
mod audio;
mod audio_player;
mod audio_retention;
mod automations;
mod automations_engine;
mod badges;
mod batch;
mod capture_marker;
mod chat;
mod chat_panel_layout;
mod chat_tools;
mod contact_summary;
mod contacts;
mod cuelume;
mod db;
mod deeplink;
mod developers;
mod dialogs;
mod dictation;
mod document;
mod edit_menu;
mod editor;
mod emoji;
mod enhancer;
mod event_contacts;
mod folders;
mod gtk_loop;
mod keywords;
mod live_transcript;
mod llm_stream;
mod mention;
mod note_search;
mod notification_apps;
mod notifications;
mod opener;
mod pre_meeting;
mod prose_text;
mod recording;
mod scheduled_auto_start;
mod search;
mod secrets;
mod session_correction;
mod sfx;
mod shell;
mod sidebar_layout;
mod speaker_assignment;
mod squircle;
mod startup;
mod stats;
mod storage;
mod store_file;
mod stt_capabilities;
mod stt_models;
mod system_theme;
mod templates;
mod text_area;
mod text_input;
mod theme;
mod timeline;
mod transcript;
mod tray;
mod ui;
mod unified_diff;
mod voiceprint;
mod webkit_local_storage;
mod window_state;
mod workspace;
#[cfg(target_os = "linux")]
mod x11;

/// The version the app reports (`app.package_info().version` in Tauri): the
/// release lane stamps `APP_VERSION` into `tauri.conf.json` and exports it to
/// the sidecar build, so both shells show the same number; a plain build
/// falls back to the crate's.
pub const APP_VERSION: &str = match option_env!("APP_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context as _;
use gpui::{
    App, AppContext as _, Application, Bounds, TitlebarOptions, WindowBounds, WindowDecorations,
    WindowHandle, WindowOptions, point, px, size,
};

use crate::db::Store;
use crate::workspace::Workspace;

/// The main window the tray menu acts on.
struct MainWindow {
    handle: Option<WindowHandle<Workspace>>,
}

impl gpui::Global for MainWindow {}

/// The loopback callback server the onboarding demo reports back to.
struct DeepLinks {
    server: deeplink::CallbackServer,
}

impl gpui::Global for DeepLinks {}

/// `useDeeplinkHandler` + the single-instance callback: bring the main
/// window back (reopening it when it was closed) and route the link.
fn handle_deep_link_url(url: &str, store: &Arc<Store>, cx: &mut App) {
    let incoming = deeplink::classify(url);
    let handle = match cx.global::<MainWindow>().handle {
        Some(handle) if cx.windows().contains(&handle.into()) => handle,
        _ => match open_main_window(store.clone(), cx) {
            Ok(handle) => handle,
            Err(error) => {
                tracing::error!(%error, "failed to reopen main window for deep link");
                return;
            }
        },
    };
    cx.activate(true);
    handle
        .update(cx, |workspace, window, cx| {
            workspace::show_window(window);
            match incoming {
                deeplink::Incoming::DeepLink(link) => workspace.handle_deep_link(link, cx),
                deeplink::Incoming::ShareOpen(request) => {
                    tracing::warn!(
                        ?request,
                        "shared-note opens need the signed-in flows, which the native shell does not ship yet"
                    );
                }
                deeplink::Incoming::Focus => {}
            }
        })
        .ok();
}

/// The main window's frame as `tauri-plugin-window-state` saved it (and
/// `true`), when there is one; otherwise `MAIN_WINDOW_WIDTH` ×
/// `MAIN_WINDOW_HEIGHT` centred.
fn main_window_bounds(identifier: &str, cx: &App) -> (WindowBounds, bool) {
    let saved = window_state::path(identifier)
        .and_then(|path| window_state::load(&path, window_state::MAIN_LABEL));
    match saved {
        Some(frame) => {
            let bounds = Bounds::new(
                point(px(frame.x as f32), px(frame.y as f32)),
                size(px(frame.width as f32), px(frame.height as f32)),
            );
            let bounds = if frame.maximized {
                WindowBounds::Maximized(bounds)
            } else {
                WindowBounds::Windowed(bounds)
            };
            (bounds, true)
        }
        None => (
            WindowBounds::Windowed(Bounds::centered(None, size(px(910.0), px(600.0)), cx)),
            false,
        ),
    }
}

fn open_main_window(store: Arc<Store>, cx: &mut App) -> anyhow::Result<WindowHandle<Workspace>> {
    let identifier = store.identifier().to_string();
    let (bounds, restored) = main_window_bounds(&identifier, cx);
    // Tauri ships `decorations: false` with its own title bar on Windows
    // and Linux, and a transparent title bar with inset traffic lights on
    // macOS (`tauri.macos.conf.json`).
    let window = cx.open_window(
        WindowOptions {
            window_bounds: Some(bounds),
            titlebar: Some(TitlebarOptions {
                // `productName` per channel, the title every Tauri window carries.
                title: Some(tray::app_name(&identifier).into()),
                appears_transparent: cfg!(target_os = "macos"),
                traffic_light_position: cfg!(target_os = "macos")
                    .then(|| point(px(12.0), px(12.0))),
            }),
            window_decorations: Some(if cfg!(any(target_os = "windows", target_os = "linux")) {
                WindowDecorations::Client
            } else {
                WindowDecorations::Server
            }),
            app_id: Some(APP_ID.to_string()),
            // `min_inner_size(500.0, 500.0)` on the Tauri main window.
            window_min_size: Some(size(px(500.0), px(500.0))),
            ..Default::default()
        },
        |window, cx| {
            let workspace = cx.new(|cx| Workspace::new(store, window, cx));
            // Key bindings dispatch through the focused element.
            workspace.read(cx).focus_handle().focus(window);
            workspace
        },
    )?;
    #[cfg(target_os = "linux")]
    if let WindowBounds::Windowed(rect) | WindowBounds::Maximized(rect) = bounds
        && restored
    {
        x11::move_window_when_mapped(
            f32::from(rect.size.width) as u32,
            f32::from(rect.size.height) as u32,
            f32::from(rect.origin.x) as i32,
            f32::from(rect.origin.y) as i32,
        );
    }
    // The window manager's close (`CloseRequested` on `AppWindow::Main`):
    // the frame is saved and the window hides behind the tray instead of
    // closing, like `Workspace::close_window`.
    let close_identifier = identifier.clone();
    window
        .update(cx, |_, window, cx| {
            window.on_window_should_close(cx, move |window, _| {
                window_state::save_main(&close_identifier, window.window_bounds());
                if window.is_fullscreen() {
                    window.toggle_fullscreen();
                }
                workspace::hide_window(window);
                false
            });
        })
        .ok();
    cx.global_mut::<MainWindow>().handle = Some(window);
    Ok(window)
}

/// A tray menu click: bring the main window back (opening it again when it
/// was closed) and run the item, like `TrayOpen` / `TrayStart` /
/// `TraySettings` / `handle_agenda_menu_event`.
fn handle_tray_action(action: tray::TrayAction, store: &Arc<Store>, cx: &mut App) {
    use tray::TrayAction;
    match action {
        // `TrayHide`: `window.hide()`; the window stays open (gpui 0.2.2's
        // X11 client stops the run loop once the last window closes).
        TrayAction::Hide => {
            if let Some(handle) = cx.global::<MainWindow>().handle {
                handle
                    .update(cx, |_, window, _| workspace::hide_window(window))
                    .ok();
            }
        }
        TrayAction::QuitCompletely => {
            let handle = cx.global::<MainWindow>().handle;
            match handle {
                Some(handle) => {
                    handle
                        .update(cx, |workspace, window, cx| {
                            workspace::show_window(window);
                            workspace.confirm_quit_completely(window, cx);
                        })
                        .ok();
                }
                None => cx.quit(),
            }
        }
        TrayAction::ToggleShowEvents => {
            let store_file = store_file::StoreFile::in_vault(store.vault_base());
            let show = !store_file
                .scoped_bool(tray::SCOPE, tray::SHOW_EVENTS_KEY)
                .unwrap_or(true);
            if let Err(error) = store_file.set_scoped(tray::SCOPE, tray::SHOW_EVENTS_KEY, show) {
                tracing::warn!(%error, "failed to persist tray event visibility");
            }
            cx.global::<tray::Tray>()
                .send(tray::TrayCommand::ShowEvents(show));
        }
        TrayAction::Open | TrayAction::Start | TrayAction::Settings | TrayAction::Agenda(_) => {
            let handle = match cx.global::<MainWindow>().handle {
                Some(handle) if cx.windows().contains(&handle.into()) => handle,
                _ => match open_main_window(store.clone(), cx) {
                    Ok(handle) => handle,
                    Err(error) => {
                        tracing::error!(%error, "failed to reopen main window from tray");
                        return;
                    }
                },
            };
            cx.activate(true);
            handle
                .update(cx, |workspace, window, cx| {
                    workspace::show_window(window);
                    match action {
                        TrayAction::Start => workspace.new_note_and_listen(cx),
                        TrayAction::Settings => {
                            workspace.open_settings(workspace::SettingsTab::App, window, cx)
                        }
                        TrayAction::Agenda(event_id) => {
                            workspace.open_event_and_record(event_id, cx)
                        }
                        _ => {}
                    }
                })
                .ok();
        }
    }
}

/// A notification's `Open Anarlog`: bring the window back and open the
/// session it names (`openNew({ type: "sessions", id })`).
fn handle_notification_open(opened: notifications::Opened, store: &Arc<Store>, cx: &mut App) {
    let handle = match cx.global::<MainWindow>().handle {
        Some(handle) if cx.windows().contains(&handle.into()) => handle,
        _ => match open_main_window(store.clone(), cx) {
            Ok(handle) => handle,
            Err(error) => {
                tracing::error!(%error, "failed to reopen main window from notification");
                return;
            }
        },
    };
    cx.activate(true);
    handle
        .update(cx, |workspace, window, cx| {
            workspace::show_window(window);
            workspace.open_session_from_notification(opened.session_id, cx);
        })
        .ok();
}

#[cfg(debug_assertions)]
const DEFAULT_IDENTIFIER: &str = "com.hyprnote.dev";
#[cfg(not(debug_assertions))]
const DEFAULT_IDENTIFIER: &str = "com.hyprnote.stable";

const APP_ID: &str = "so.anarlog.Anarlog";

struct Args {
    db_path: Option<PathBuf>,
    identifier: String,
    /// `anarlog://…` URLs the OS (or the Tauri launcher) passed along.
    urls: Vec<String>,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut args = pico_args::Arguments::from_env();
    if args.contains(["-h", "--help"]) {
        println!(
            "anarlog-gpui\n\n\
             Options:\n  \
             --db-path <PATH>       Open a specific app.db instead of the desktop app's database\n  \
             --identifier <ID>      Bundle identifier whose database to open (default: {DEFAULT_IDENTIFIER})\n\n\
             Positional arguments are deep-link URLs (e.g. anarlog://focus)."
        );
        std::process::exit(0);
    }
    let db_path = args.opt_value_from_str("--db-path")?;
    let identifier = args
        .opt_value_from_str("--identifier")?
        .unwrap_or_else(|| DEFAULT_IDENTIFIER.to_string());
    let rest = args.finish();
    let urls = deeplink::urls_from_args(&rest);
    if urls.len() != rest.len() {
        anyhow::bail!("unexpected arguments: {rest:?}");
    }
    Ok(Args {
        db_path,
        identifier,
        urls,
    })
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = parse_args()?;
    let db_path = match args.db_path {
        Some(path) => path,
        None => db::default_db_path(&args.identifier)?,
    };
    // One window per database: a second launch hands its URLs to the
    // running instance (`tauri-plugin-single-instance`).
    let forwarded = match deeplink::claim(&deeplink::socket_path(&db_path), &args.urls) {
        deeplink::Claim::Forwarded => return Ok(()),
        deeplink::Claim::Primary(receiver) => receiver,
    };
    // Held until the app exits so Nightly and stable never share the open
    // database at the same time.
    let db_dir = db_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    let _channel_lock = match startup::acquire_channel_lock(&args.identifier, &db_dir) {
        startup::ChannelLockState::Acquired(lock) => lock,
        startup::ChannelLockState::PeerRunning { peer } => {
            startup::exit_for_running_peer_channel(&args.identifier, peer)
        }
        startup::ChannelLockState::Unavailable(reason) => {
            eprintln!("starting without the channel lock: {reason}");
            None
        }
    };
    let (deeplink_sender, deeplink_receiver) = std::sync::mpsc::channel::<String>();
    let startup_urls = args.urls.clone();

    // sqlx runs on tokio; GPUI drives its own executor on the main thread. The
    // runtime lives for the whole process and the Store bridges the two.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to start tokio runtime")?;
    let store = runtime.block_on(Store::open(
        runtime.handle().clone(),
        db_path,
        args.identifier.clone(),
    ))?;
    let audio = audio::provider(&args.identifier);
    let store = Arc::new(store);
    let search = search::SearchIndex::start(&store);
    tracing::info!(path = %store.path().display(), "opened application database");
    // The direct-distribution Tauri build writes the vault's `AGENTS.md` on
    // every start (`agents::write_agents_file`).
    if let Err(error) = storage::write_agents_file(store.vault_base()) {
        tracing::error!(%error, "failed to write AGENTS.md");
    }

    let identifier = args.identifier.clone();
    let callback_server = deeplink::CallbackServer::new(
        runtime.handle().clone(),
        deeplink::scheme(&identifier),
        deeplink_sender.clone(),
    );
    let reopen_sender = deeplink_sender.clone();
    let app = Application::new().with_assets(assets::Assets);
    // macOS delivers scheme URLs to the running process (`on_open_url`)
    // instead of a second launch; a Dock click on a running app is a bare
    // reopen, which brings the window back like a focus link.
    app.on_open_urls(move |urls| {
        for url in urls {
            let _ = deeplink_sender.send(url);
        }
    })
    .on_reopen(move |_| {
        let _ = reopen_sender.send(String::new());
    });
    app.run(move |cx: &mut App| {
        cx.set_global(audio::Audio(audio));
        cx.set_global(search::Search(search));
        cx.set_global(MainWindow { handle: None });
        cx.set_global(system_theme::SystemTheme::start(store.runtime()));
        cx.set_global(notifications::Notifications::install());
        cx.set_global(DeepLinks {
            server: callback_server,
        });
        let store_file = store_file::StoreFile::in_vault(store.vault_base());
        cx.set_global(tray::Tray::start(tray::TrayState {
            app_name: tray::app_name(&identifier).to_string(),
            version_label: anlg_tray_core::labels::version(APP_VERSION, tray::channel(&identifier)),
            schedule: Vec::new(),
            show_events: store_file
                .scoped_bool(tray::SCOPE, tray::SHOW_EVENTS_KEY)
                .unwrap_or(true),
            start_disabled: false,
            recording: false,
            degraded: false,
        }));
        actions::bind_keys(cx);
        text_input::bind_keys(cx);
        text_area::bind_keys(cx);
        editor::bind_keys(cx);
        workspace::menu::bind_keys(cx);
        // The plugin saves on `ExitRequested`: the open main window's frame
        // is written when the app quits (the tray's Quit, the shell switch).
        let quit_identifier = identifier.clone();
        cx.on_app_quit(move |cx| {
            if let Some(handle) = cx.global::<MainWindow>().handle
                && let Ok(bounds) = handle.update(cx, |_, window, _| window.window_bounds())
            {
                window_state::save_main(&quit_identifier, bounds);
            }
            async {}
        })
        .detach();
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                // gpui 0.2.2's X11 client still holds its state borrow while
                // firing this callback; quitting synchronously re-borrows it
                // and panics, so hop to the next executor tick first.
                cx.spawn(async move |cx| cx.update(|cx| cx.quit()).ok())
                    .detach();
            }
        })
        .detach();

        if let Err(error) = open_main_window(store.clone(), cx) {
            tracing::error!(%error, "failed to open main window");
            cx.quit();
            return;
        }
        cx.activate(true);

        // URLs the launcher was started with are queued until the window
        // is up (`take_pending_deep_links`).
        for url in &startup_urls {
            handle_deep_link_url(url, &store, cx);
        }

        // Tray menu clicks, forwarded launches, and loopback callbacks
        // arrive on their threads' channels.
        let tray_store = store.clone();
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                let stop = cx
                    .update(|cx| {
                        for action in cx.global::<tray::Tray>().take_actions() {
                            handle_tray_action(action, &tray_store, cx);
                        }
                        for opened in cx.global::<notifications::Notifications>().take_opened() {
                            handle_notification_open(opened, &tray_store, cx);
                        }
                        for url in forwarded.try_iter().chain(deeplink_receiver.try_iter()) {
                            handle_deep_link_url(&url, &tray_store, cx);
                        }
                        // `onThemeChanged`: windows on the `system` theme re-resolve.
                        if cx.global::<system_theme::SystemTheme>().take_changed() {
                            cx.refresh_windows();
                        }
                    })
                    .is_err();
                if stop {
                    break;
                }
            }
        })
        .detach();
    });

    drop(runtime);
    Ok(())
}
