mod db;
mod theme;
mod timeline;
mod workspace;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context as _;
use gpui::{
    App, AppContext as _, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px,
    size,
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
    let store = runtime.block_on(Store::open(runtime.handle().clone(), db_path))?;
    let store = Arc::new(store);
    tracing::info!(path = %store.path().display(), "opened application database");

    Application::new().run(move |cx: &mut App| {
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
        let window = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Anarlog".into()),
                    ..Default::default()
                }),
                app_id: Some(APP_ID.to_string()),
                window_min_size: Some(size(px(640.0), px(400.0))),
                ..Default::default()
            },
            |_window, cx| cx.new(|cx| Workspace::new(store, cx)),
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
