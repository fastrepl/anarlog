use std::path::PathBuf;

use desktop_gpui::{application::ApplicationView, workspace::assets::Assets};
use desktop_gpui::{
    native_events::{MenuState, NativeEvents},
    platform::windows::WindowIdentity,
    ui::theme::UiFonts,
};
use desktop_runtime::Profile;
use gpui::AppContext;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();
    let mut args = std::env::args_os().skip(1);
    let first = args.next();
    if first.as_deref() == Some(std::ffi::OsStr::new("--help")) {
        #[cfg(target_os = "linux")]
        let url = " [URL]";
        #[cfg(not(target_os = "linux"))]
        let url = "";
        println!(
            "Anarlog native preview\nUsage: desktop-gpui [--profile DIRECTORY]{url}\nDefault: $HOME/.anarlog-gpui-sandbox/library.sqlite\nUse only an isolated profile or a backup copy, never the shipping app's live profile."
        );
        return Ok(());
    }
    let (profile, next) = if first.as_deref() == Some(std::ffi::OsStr::new("--profile")) {
        let profile = args
            .next()
            .ok_or_else(|| anyhow::anyhow!("--profile requires a directory"))?;
        (PathBuf::from(profile), args.next())
    } else {
        #[cfg(not(target_os = "linux"))]
        anyhow::ensure!(first.is_none(), "Unknown option. Use --help.");
        let profile = PathBuf::from(
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .ok_or_else(|| anyhow::anyhow!("No home directory; pass --profile DIRECTORY"))?,
        )
        .join(".anarlog-gpui-sandbox");
        (profile, first)
    };
    #[cfg(target_os = "linux")]
    let launch_url = next
        .map(|argument| {
            argument
                .into_string()
                .ok()
                .filter(|url| desktop_gpui::platform::deeplinks::has_registered_scheme(url))
                .ok_or_else(|| anyhow::anyhow!("Unknown option. Use --help."))
        })
        .transpose()?;
    #[cfg(not(target_os = "linux"))]
    anyhow::ensure!(next.is_none(), "Unexpected extra arguments");
    anyhow::ensure!(args.next().is_none(), "Unexpected extra arguments");
    #[cfg(target_os = "linux")]
    let restart_profile = profile.clone();
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
    #[cfg(target_os = "linux")]
    let Some(launch_links) =
        desktop_gpui::platform::deeplinks::instance::LaunchLinks::bind_or_forward(
            &profile, launch_url,
        )?
    else {
        return Ok(());
    };
    #[cfg(target_os = "linux")]
    let launch_links = std::rc::Rc::new(launch_links);
    #[cfg(target_os = "linux")]
    let launch_poller = std::rc::Rc::downgrade(&launch_links);
    let (runtime, ready) = desktop_gpui::runtime_bridge::start(Profile {
        database: profile.join("library.sqlite"),
    })?;
    let zoom = desktop_gpui::zoom::Zoom::load(profile.join("zoom.json"));
    let application = zoom.application().with_assets(Assets);
    let events = NativeEvents::install(&application);
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
                            let Ok((tray, update)) = main.update(cx, |view, _window, cx| {
                                view.native_tick(cx);
                                #[cfg(target_os = "linux")]
                                if view.accepts_launch_links()
                                    && let Some(links) = launch_poller.upgrade()
                                    && let Some(request) = links.next()
                                {
                                    request.deliver(|url| {
                                        events.dispatch_launch(url, view, _window, cx)
                                    });
                                }
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
    // Release the listening socket before a replacement process tries to bind it.
    #[cfg(target_os = "linux")]
    drop(launch_links);
    if desktop_gpui::platform::windows::restart_requested() {
        let mut command = std::process::Command::new(std::env::current_exe()?);
        #[cfg(target_os = "linux")]
        command.arg("--profile").arg(restart_profile);
        #[cfg(not(target_os = "linux"))]
        command.args(std::env::args_os().skip(1));
        command.spawn()?;
    }
    Ok(())
}
