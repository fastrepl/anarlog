mod actions;
mod ai_providers;
mod assets;
mod audio;
mod contacts;
mod db;
mod developers;
mod document;
mod editor;
mod folders;
mod secrets;
mod squircle;
mod store_file;
mod templates;
mod text_area;
mod text_input;
mod theme;
mod timeline;
mod transcript;
mod ui;
mod workspace;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context as _;
use gpui::{
    App, AppContext as _, Application, Bounds, TitlebarOptions, WindowBounds, WindowDecorations,
    WindowOptions, point, px, size,
};

use crate::db::Store;
use crate::workspace::Workspace;

#[cfg(debug_assertions)]
const DEFAULT_IDENTIFIER: &str = "com.hyprnote.dev";
#[cfg(not(debug_assertions))]
const DEFAULT_IDENTIFIER: &str = "com.hyprnote.stable";

const APP_ID: &str = "so.anarlog.Anarlog";

struct Args {
    db_path: Option<PathBuf>,
    identifier: String,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut args = pico_args::Arguments::from_env();
    if args.contains(["-h", "--help"]) {
        println!(
            "anarlog-gpui\n\n\
             Options:\n  \
             --db-path <PATH>       Open a specific app.db instead of the desktop app's database\n  \
             --identifier <ID>      Bundle identifier whose database to open (default: {DEFAULT_IDENTIFIER})"
        );
        std::process::exit(0);
    }
    let db_path = args.opt_value_from_str("--db-path")?;
    let identifier = args
        .opt_value_from_str("--identifier")?
        .unwrap_or_else(|| DEFAULT_IDENTIFIER.to_string());
    let rest = args.finish();
    if !rest.is_empty() {
        anyhow::bail!("unexpected arguments: {rest:?}");
    }
    Ok(Args {
        db_path,
        identifier,
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
    tracing::info!(path = %store.path().display(), "opened application database");

    Application::new()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            cx.set_global(audio::Audio(audio));
            actions::bind_keys(cx);
            text_input::bind_keys(cx);
            text_area::bind_keys(cx);
            editor::bind_keys(cx);
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

            let bounds = Bounds::centered(None, size(px(1100.0), px(720.0)), cx);
            // Tauri ships `decorations: false` with its own title bar on Windows
            // and Linux, and a transparent title bar with inset traffic lights on
            // macOS (`tauri.macos.conf.json`).
            let window = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Anarlog".into()),
                        appears_transparent: cfg!(target_os = "macos"),
                        traffic_light_position: cfg!(target_os = "macos")
                            .then(|| point(px(12.0), px(12.0))),
                    }),
                    window_decorations: Some(
                        if cfg!(any(target_os = "windows", target_os = "linux")) {
                            WindowDecorations::Client
                        } else {
                            WindowDecorations::Server
                        },
                    ),
                    app_id: Some(APP_ID.to_string()),
                    window_min_size: Some(size(px(640.0), px(400.0))),
                    ..Default::default()
                },
                |window, cx| {
                    let workspace = cx.new(|cx| Workspace::new(store, window, cx));
                    // Key bindings dispatch through the focused element.
                    workspace.read(cx).focus_handle().focus(window);
                    workspace
                },
            );
            if let Err(error) = window {
                tracing::error!(%error, "failed to open main window");
                cx.quit();
                return;
            }
            cx.activate(true);
        });

    drop(runtime);
    Ok(())
}
