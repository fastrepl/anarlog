use std::path::PathBuf;

use desktop_gpui::{application::ApplicationView, workspace::assets::Assets};
use desktop_gpui::{
    native_events::{MenuState, NativeEvents},
    platform::windows::WindowIdentity,
    ui::theme::UiFonts,
};
use desktop_runtime::Profile;
use gpui::AppContext;

#[cfg(target_os = "windows")]
mod link_instance;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();
    let mut args = std::env::args_os().skip(1).peekable();
    if args.peek().is_some_and(|argument| argument == "--help") {
        let url_usage = if cfg!(target_os = "windows") {
            " [URL]"
        } else {
            ""
        };
        println!(
            "Anarlog native preview\nUsage: desktop-gpui [--profile DIRECTORY]{url_usage}\nDefault: $HOME/.anarlog-gpui-sandbox/library.sqlite\nUse only an isolated profile or a backup copy, never the shipping app's live profile."
        );
        return Ok(());
    }
    let explicit_profile = args.next_if(|argument| argument == "--profile").is_some();
    let profile = if explicit_profile {
        Some(PathBuf::from(args.next().ok_or_else(|| {
            anyhow::anyhow!("--profile requires a directory")
        })?))
    } else {
        None
    };
    #[cfg(target_os = "windows")]
    let url = args.next().map(link_instance::parse_argument).transpose()?;
    #[cfg(not(target_os = "windows"))]
    if !explicit_profile && args.peek().is_some() {
        anyhow::bail!("Unknown option. Use --help.");
    }
    let profile = match profile {
        Some(path) => path,
        None => PathBuf::from(
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .ok_or_else(|| anyhow::anyhow!("No home directory; pass --profile DIRECTORY"))?,
        )
        .join(".anarlog-gpui-sandbox"),
    };
    anyhow::ensure!(args.next().is_none(), "Unexpected extra arguments");
    let pointer = profile.with_extension("storage.json");
    let profile = if pointer.exists() {
        let path: PathBuf = serde_json::from_slice(&std::fs::read(&pointer)?)?;
        anyhow::ensure!(
            path.is_absolute() && path.is_dir(),
            "Stored native profile location is not available"
        );
        path
    } else {
        profile
    };
    let events = NativeEvents::default();
    #[cfg(target_os = "windows")]
    let link_instance = match link_instance::start(&profile, url.as_deref(), events.clone())? {
        Some(instance) => instance,
        None => return Ok(()),
    };
    #[cfg(target_os = "windows")]
    if let Some(url) = url {
        events.receive_deep_link(url)?;
    }
    let (runtime, ready) = desktop_gpui::runtime_bridge::start(Profile {
        database: profile.join("library.sqlite"),
    })?;
    let zoom = desktop_gpui::zoom::Zoom::load(profile.join("zoom.json"));
    let application = zoom.application().with_assets(Assets);
    events.install(&application);
    application.run(move |cx| {
        cx.set_global(zoom);
        UiFonts::init(cx);
        use desktop_gpui::platform::system_settings::{Installed, NativeSettings};
        match NativeSettings::current() {
            Ok(native) => cx.set_global(Installed(std::sync::Arc::new(native))),
            Err(error) => tracing::warn!(%error, "native setting side effects are unavailable"),
        }
        desktop_gpui::meeting::automation::NativePlatform::install(cx);
        cx.set_global(desktop_gpui::platform::device_auth::Installed(
            std::sync::Arc::new(desktop_gpui::platform::device_auth::NativeDeviceAuth),
        ));
        match desktop_gpui::meeting::schedule::apps::DetectMeetingApps::spawn() {
            Ok(apps) => cx.set_global(desktop_gpui::meeting::schedule::apps::InstalledMeetingApps(
                std::sync::Arc::new(apps),
            )),
            Err(error) => tracing::warn!(%error, "meeting app automation is unavailable"),
        }
        let (mut handles, errors) = events.attach(cx);
        for error in errors {
            tracing::warn!("{error}");
        }
        let result = cx.open_window(
            WindowIdentity::Main
                .options(None, cx)
                .expect("valid default window"),
            move |window, cx| {
                let root = cx.new(|cx| {
                    ApplicationView::new(runtime.clone(), ready, profile, pointer, window, cx)
                });
                let root_weak = root.downgrade();
                // Like shipping, closing the main window hides it; the app keeps running.
                window.on_window_should_close(cx, move |window, cx| {
                    let _ = root_weak.update(cx, |root, cx| root.close_main(window, cx));
                    false
                });
                root
            },
        );
        match result {
            Ok(main) => {
                cx.spawn(async move |cx| {
                    loop {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(100))
                            .await;
                        let done = cx.update(|cx| {
                            let Ok((tray, update)) = main.update(cx, |view, _, cx| {
                                view.native_tick(cx);
                                (view.tray_model(), view.update_menu())
                            }) else {
                                return true;
                            };
                            let fullscreen = cx
                                .active_window()
                                .and_then(|window| {
                                    window
                                        .update(cx, |_, window, _| window.is_fullscreen())
                                        .ok()
                                })
                                .unwrap_or(false);
                            events.sync_menus(
                                &mut handles,
                                MenuState {
                                    update: (update.label(), update.enabled()),
                                    fullscreen,
                                },
                                cx,
                            );
                            events.poll(&mut handles, &tray, main, cx);
                            false
                        });
                        if done {
                            break;
                        }
                    }
                })
                .detach();
            }
            Err(error) => {
                tracing::error!(%error, "native window could not open");
                cx.quit();
            }
        }
        cx.activate(true);
    });
    #[cfg(target_os = "windows")]
    drop(link_instance);
    if desktop_gpui::platform::windows::restart_requested() {
        std::process::Command::new(std::env::current_exe()?)
            .args(
                std::env::args_os()
                    .skip(1)
                    .take(if explicit_profile { 2 } else { 0 }),
            )
            .spawn()?;
    }
    Ok(())
}
