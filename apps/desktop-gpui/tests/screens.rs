//! Renders the app's main screens to PNGs, without opening a window, for side-by-side
//! comparison with the shipping desktop app. The seeded library is copied next to the
//! screenshots so the shipping app can render the same data.
//!
//! ANLG_SCREENS_DIR=/tmp/screens cargo test --test screens
//!
//! Seed data is every `tests/screens/*.sql`, applied in name order.
//!
//! Without ANLG_SCREENS_DIR it does nothing, so `cargo test` and CI skip it. It runs as a
//! plain `main` because the macOS platform must be created on the main thread.

use std::{path::PathBuf, sync::Arc, time::Duration};

use desktop_gpui::{
    application::ApplicationView,
    ui::theme::UiFonts,
    workspace::{assets::Assets, navigation::Route},
};
use desktop_runtime::{Profile, RuntimeHandle, ServiceError, SessionId};
use gpui::{
    AnyWindowHandle, AppContext, HeadlessAppContext, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PlatformInput, ScrollDelta, ScrollEvent, ScrollSource,
    TouchPhase, WindowHandle, point, px, size,
};

const WINDOW: (f32, f32) = (910., 600.);

fn main() {
    let Some(out) = std::env::var_os("ANLG_SCREENS_DIR").map(PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&out).unwrap();
    let profile = tempfile::tempdir().unwrap();
    let (runtime, ready) = desktop_gpui::runtime_bridge::start(Profile {
        database: profile.path().join("library.sqlite"),
    })
    .unwrap();

    let text_system = gpui_platform::current_platform(true).text_system();
    let mut cx = HeadlessAppContext::with_platform(
        text_system,
        Arc::new(Assets),
        gpui_platform::current_headless_renderer,
    );
    // The runtime answers from its own threads, so the dispatcher must wait for real I/O.
    cx.allow_parking();
    cx.update(UiFonts::init);
    let window = {
        let runtime = runtime.clone();
        let profile = profile.path().to_path_buf();
        cx.open_window(size(px(WINDOW.0), px(WINDOW.1)), move |window, cx| {
            cx.new(|cx| {
                ApplicationView::new(
                    runtime,
                    ready,
                    profile.clone(),
                    profile.with_extension("storage.json"),
                    window,
                    cx,
                )
            })
        })
        .unwrap()
    };
    settle(&mut cx);
    seed(&runtime, &out);
    settle(&mut cx);

    capture(&mut cx, window, &out, "main");
    // Sidebar chrome, by window position: sort menu, its Grouping submenu, then the toggle.
    click(&mut cx, window, 178., 23.);
    pointer(&mut cx, window, 260., 67.);
    capture(&mut cx, window, &out, "sort-menu");
    click(&mut cx, window, 450., 108.);
    capture(&mut cx, window, &out, "folder-grouping");
    click(&mut cx, window, 178., 23.);
    pointer(&mut cx, window, 260., 67.);
    click(&mut cx, window, 450., 76.);
    click(&mut cx, window, 94., 23.);
    capture(&mut cx, window, &out, "sidebar-collapsed");
    click(&mut cx, window, 94., 23.);
    pointer(&mut cx, window, 700., 590.);
    scroll(&mut cx, window, -260.);
    capture(&mut cx, window, &out, "timeline-scrolled");
    scroll(&mut cx, window, 1000.);
    for (name, route) in [
        ("note", Route::Session(SessionId("s-sync".into()))),
        (
            "note-markdown",
            Route::Session(SessionId("s-korean".into())),
        ),
        ("calendar", Route::Calendar),
        ("contacts", Route::Contacts),
        ("templates", Route::Templates),
        ("folders", Route::Folders),
        ("settings", Route::Settings("app".into())),
    ] {
        open(&mut cx, window, route);
        capture(&mut cx, window, &out, name);
    }
    for page in [
        "account",
        "billing",
        "referrals",
        "insights",
        "team",
        "sync",
        "appearance",
        "notifications",
        "transcription",
        "dictation",
        "intelligence",
        "dictionary",
        "meetings",
        "imports",
        "crm",
        "privacy",
        "permissions",
        "developers",
    ] {
        open(&mut cx, window, Route::Settings(page.into()));
        capture(&mut cx, window, &out, &format!("settings-{page}"));
    }
}

fn seed(runtime: &RuntimeHandle, out: &std::path::Path) {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/screens");
    let mut files = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "sql"))
        .collect::<Vec<_>>();
    files.sort();
    let scripts = files
        .iter()
        .map(|path| std::fs::read_to_string(path).unwrap())
        .collect::<Vec<_>>();
    let copy = out.join("library.sqlite");
    let _ = std::fs::remove_file(&copy);
    let copy = copy.to_string_lossy().into_owned();
    let reply = runtime
        .submit(move |services| async move {
            let failed = |error: sqlx::Error| ServiceError::Failed(error.to_string().into());
            for script in scripts {
                sqlx::raw_sql(sqlx::AssertSqlSafe(script))
                    .execute(services.db.pool())
                    .await
                    .map_err(failed)?;
            }
            sqlx::query("VACUUM INTO ?")
                .bind(copy)
                .execute(services.db.pool())
                .await
                .map_err(failed)?;
            Ok(())
        })
        .unwrap();
    futures::executor::block_on(reply.receive()).unwrap();
}

fn open(cx: &mut HeadlessAppContext, window: WindowHandle<ApplicationView>, route: Route) {
    cx.update(|cx| {
        window
            .update(cx, |root, _, cx| {
                root.workspace()
                    .clone()
                    .update(cx, |workspace, cx| workspace.open_route(route, false, cx))
            })
            .unwrap()
    });
    settle(cx);
}

fn capture(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    out: &std::path::Path,
    name: &str,
) {
    let window: AnyWindowHandle = window.into();
    // Captures read the last rendered scene, so draw the current state first.
    cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
    let image = cx.capture_screenshot(window).unwrap();
    let path = out.join(format!("{name}.png"));
    image.save(&path).unwrap();
    println!("{}", path.display());
}

fn pointer(cx: &mut HeadlessAppContext, window: WindowHandle<ApplicationView>, x: f32, y: f32) {
    input(
        cx,
        window,
        [PlatformInput::MouseMove(MouseMoveEvent {
            position: point(px(x), px(y)),
            pressed_button: None,
            modifiers: Modifiers::default(),
        })],
    );
}

/// Scrolls the sidebar timeline; negative deltas move the content up.
fn scroll(cx: &mut HeadlessAppContext, window: WindowHandle<ApplicationView>, y: f32) {
    input(
        cx,
        window,
        [PlatformInput::Scroll(ScrollEvent {
            position: point(px(100.), px(400.)),
            delta: ScrollDelta::Pixels(point(px(0.), px(y))),
            modifiers: Modifiers::default(),
            touch_phase: TouchPhase::Moved,
            source: ScrollSource::Pointer,
        })],
    );
}

fn click(cx: &mut HeadlessAppContext, window: WindowHandle<ApplicationView>, x: f32, y: f32) {
    let position = point(px(x), px(y));
    input(
        cx,
        window,
        [
            PlatformInput::MouseMove(MouseMoveEvent {
                position,
                pressed_button: None,
                modifiers: Modifiers::default(),
            }),
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            }),
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers: Modifiers::default(),
                click_count: 1,
            }),
        ],
    );
}

/// Dispatches pointer input against the last drawn frame's hitboxes.
fn input<const N: usize>(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ApplicationView>,
    events: [PlatformInput; N],
) {
    let window: AnyWindowHandle = window.into();
    for event in events {
        cx.update_window(window, |_, window, cx| {
            window.draw(cx).clear(cx);
            window.dispatch_event(event, cx);
        })
        .unwrap();
        cx.run_until_parked();
    }
    settle(cx);
}

/// Lets runtime replies and simulated timers land before a frame is captured.
fn settle(cx: &mut HeadlessAppContext) {
    for _ in 0..40 {
        cx.advance_clock(Duration::from_millis(50));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(25));
    }
}
