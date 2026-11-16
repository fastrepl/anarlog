/// The platform's primary shortcut modifier. Static key bindings take explicit modifiers.
macro_rules! secondary {
    ($key:literal) => {
        if cfg!(target_os = "macos") {
            concat!("cmd-", $key)
        } else {
            concat!("ctrl-", $key)
        }
    };
}

mod library;
mod text_input;
mod workspace;

use std::path::PathBuf;

use clap::Parser;
use gpui::{
    App, Bounds, KeyBinding, Menu, MenuItem, TitlebarOptions, WindowBounds, WindowOptions,
    prelude::*, px, size,
};
use workspace::{
    Dismiss, FocusFilter, Next, NextPage, OpenLibrary, Previous, PreviousPage, Quit, Reload,
    Workspace,
};

fn key_bindings() -> Vec<KeyBinding> {
    let mut bindings = vec![
        KeyBinding::new(secondary!("q"), Quit, None),
        KeyBinding::new(secondary!("o"), OpenLibrary, Some("Library")),
        KeyBinding::new(secondary!("r"), Reload, Some("Library")),
        KeyBinding::new(secondary!("f"), FocusFilter, Some("Library")),
        KeyBinding::new("escape", Dismiss, Some("Library")),
        KeyBinding::new("down", Next, Some("Library")),
        KeyBinding::new("up", Previous, Some("Library")),
        KeyBinding::new("alt-right", NextPage, Some("NoteList")),
        KeyBinding::new("alt-left", PreviousPage, Some("NoteList")),
    ];
    bindings.extend(text_input::key_bindings());
    bindings
}

#[derive(Parser)]
#[command(about = "Anarlog's native desktop preview (read-only)")]
struct Args {
    /// Existing app.db to browse. No default profile is opened or migrated.
    #[arg(long)]
    database: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let handle = runtime.handle().clone();
    gpui_platform::application().run(move |cx: &mut App| {
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys(key_bindings());
        cx.set_menus(vec![Menu {
            name: "Anarlog".into(),
            items: vec![
                MenuItem::action("Open Library…", OpenLibrary),
                MenuItem::action("Reload Library", Reload),
                MenuItem::action("Filter Notes", FocusFilter),
                MenuItem::action("Previous Page", PreviousPage),
                MenuItem::action("Next Page", NextPage),
                MenuItem::separator(),
                MenuItem::action("Quit", Quit),
            ],
            disabled: false,
        }]);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(1100.), px(760.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(700.), px(450.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Anarlog — Native Preview".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| Workspace::new(args.database, handle, window, cx)),
        )
        .expect("Could not create the native desktop window");
        cx.activate(true);
    });
    runtime.shutdown_timeout(std::time::Duration::from_secs(2));
    Ok(())
}
