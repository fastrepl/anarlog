use std::path::PathBuf;

use anlg_desktop_identity::Locations;
use desktop_gpui::launch::{self, Launch, Mode, Storage};
use desktop_gpui::{application::ApplicationView, workspace::assets::Assets};
use desktop_gpui::{
    native_events::{MenuState, NativeEvents},
    platform::{
        package::{self, AppIds},
        windows::WindowIdentity,
    },
    ui::theme::UiFonts,
};
use desktop_runtime::{Profile, RuntimeHandle, Startup};
use gpui::AppContext;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();
    let (mode, urls) = match launch::parse(
        std::env::args_os().skip(1),
        launch::COMPILED_CHANNEL,
        cfg!(debug_assertions),
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from),
    )? {
        Launch::Help => {
            println!("{}", launch::HELP);
            return Ok(());
        }
        Launch::CrashReporter => return Ok(()),
        Launch::Package => {
            println!("{}", package::report());
            return Ok(());
        }
        Launch::Run { mode, urls } => (mode, urls),
    };
    let (storage, database) = match mode {
        Mode::Sandbox(profile) => {
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
            let database = profile.join("library.sqlite");
            let storage = Storage {
                channel: None,
                root: profile,
                pointer: Some(pointer),
            };
            (storage, database)
        }
        Mode::Channel(channel) => {
            let locations = Locations::resolve(channel.identifier())
                .ok_or_else(|| anyhow::anyhow!("The application data directory is unavailable"))?;
            (Storage::channel(channel, &locations), locations.database())
        }
    };
    let events = NativeEvents::default();
    // Before the runtime takes the profile, which the running instance holds.
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    let Some(instance) = desktop_gpui::platform::single_instance::claim(&storage.root, &urls, {
        let events = events.clone();
        move |urls| events.relaunched(urls)
    })?
    else {
        return Ok(());
    };
    events.open_urls(urls);
    let startup = match storage.channel {
        Some(_) => Startup::shipping(storage.vault()),
        None => Startup::default(),
    };
    let (runtime, ready) = RuntimeHandle::start_with(Profile { database }, startup)?;
    let zoom = desktop_gpui::zoom::Zoom::load(storage.root.join("zoom.json"));
    let application = zoom.application().with_assets(Assets);
    events.install(&application);
    if let Some(channel) = package::channel() {
        desktop_gpui::platform::deeplinks::register_handler(channel);
    }
    application.run(move |cx| {
        let ids = AppIds::current();
        cx.set_app_identity(ids.app_user_model_id, ids.name);
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
                let root =
                    cx.new(|cx| ApplicationView::new(runtime.clone(), ready, storage, window, cx));
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
    // The replacement serves this profile's launches next.
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    drop(instance);
    if desktop_gpui::platform::windows::restart_requested() {
        std::process::Command::new(std::env::current_exe()?)
            .args(launch::relaunch_args(std::env::args_os().skip(1)))
            .spawn()?;
    }
    Ok(())
}
