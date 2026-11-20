//! The accessibility tree the native app hands to the OS screen-reader adapter, asserted
//! node by node: roles, names and states, never pixels. The app runs against
//! [`platform::A11yPlatform`], which builds the AccessKit tree as if VoiceOver were on but
//! opens no window and reaches nothing outside the process.

#![cfg(not(target_os = "windows"))]

mod platform;
mod scenarios;
mod tree;

use std::{rc::Rc, sync::Arc, time::Duration};

use desktop_gpui::{application::ApplicationView, workspace::navigation::Route};
use desktop_runtime::{Profile, RuntimeHandle, ServiceError};
use gpui::{
    AnyWindowHandle, AppContext as _, Application, ApplicationHandle, Bounds, WindowBounds,
    WindowHandle, WindowOptions, accesskit, point, px, size,
};
use platform::A11yPlatform;
use tree::Tree;

pub struct Harness {
    pub platform: Rc<A11yPlatform>,
    pub app: ApplicationHandle,
    pub runtime: RuntimeHandle,
    pub window: WindowHandle<ApplicationView>,
    _profile: tempfile::TempDir,
}

impl Harness {
    /// The main window over the screens harness's fictional library.
    pub fn start() -> Self {
        desktop_gpui::platform::os::go_headless();
        let profile = tempfile::tempdir().unwrap();
        let (runtime, ready) = desktop_gpui::runtime_bridge::start(Profile {
            database: profile.path().join("library.sqlite"),
        })
        .unwrap();
        prepare(&runtime);
        let platform = A11yPlatform::new();
        platform.dispatcher.allow_parking();
        let app = Application::with_platform(platform.clone())
            .with_assets(desktop_gpui::workspace::assets::Assets)
            .run_embedded(|_| {});
        app.update(|cx| {
            use desktop_gpui::{
                meeting::upload::FilesGlobal, product, ui::theme::UiFonts, workspace::task,
            };
            UiFonts::init(cx);
            // Imports, Reminders, GitHub and file pickers answer from here, never this machine.
            cx.set_global(product::local::imports::InstalledAppsOverride(Arc::new(
                NoApps,
            )));
            cx.set_global(product::ticket::RemindersOverride(Arc::new(Reminders)));
            cx.set_global(task::GithubOverride(Arc::new(NoGithub)));
            cx.set_global(FilesGlobal(Rc::new(NoFiles)));
        });
        let window = app.update(|cx| {
            let runtime = runtime.clone();
            let profile = profile.path().to_path_buf();
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(910.), px(600.)),
                    ))),
                    focus: false,
                    show: false,
                    ..Default::default()
                },
                move |window, cx| {
                    cx.new(|cx| {
                        ApplicationView::new(
                            runtime,
                            ready,
                            desktop_gpui::launch::Storage {
                                channel: None,
                                root: profile.clone(),
                                pointer: Some(profile.with_extension("storage.json")),
                            },
                            window,
                            cx,
                        )
                    })
                },
            )
            .unwrap()
        });
        let harness = Self {
            platform,
            app,
            runtime,
            window,
            _profile: profile,
        };
        harness.settle();
        harness
    }

    /// Lets runtime replies and timers land, then draws so the tree is current.
    pub fn settle(&self) {
        for _ in 0..20 {
            self.platform
                .dispatcher
                .advance_clock(Duration::from_millis(50));
            self.platform.dispatcher.run_until_parked();
            std::thread::sleep(Duration::from_millis(10));
        }
        self.draw();
    }

    fn draw(&self) {
        self.draw_window(self.window.into());
    }

    fn draw_window(&self, window: AnyWindowHandle) {
        self.app.update(|cx| {
            cx.update_window(window, |_, window, cx| window.draw(cx).clear(cx))
                .unwrap()
        });
        self.platform.dispatcher.run_until_parked();
    }

    pub fn tree(&self) -> Tree {
        self.window_tree(self.window.into())
    }

    /// The tree of another window the app opened, such as a standalone note.
    pub fn window_tree(&self, window: AnyWindowHandle) -> Tree {
        // The first frame after activation only turns the builder on.
        self.draw_window(window);
        self.draw_window(window);
        let state = self.platform.window(window);
        let state = state.borrow();
        Tree::new(state.tree.as_ref().expect("an accessibility tree update"))
    }

    pub fn open(&self, route: Route) {
        self.app.update(|cx| {
            self.window
                .update(cx, |root, _, cx| {
                    root.workspace()
                        .clone()
                        .update(cx, |workspace, cx| workspace.open_route(route, false, cx))
                })
                .unwrap()
        });
        self.settle();
    }

    pub fn chat(
        &self,
        action: impl FnOnce(
            &mut desktop_gpui::chat::ChatView,
            &mut gpui::Window,
            &mut gpui::Context<desktop_gpui::chat::ChatView>,
        ),
    ) {
        self.app.update(|cx| {
            self.window
                .update(cx, |root, window, cx| {
                    let chat = root.workspace().read(cx).chat().clone();
                    chat.update(cx, |chat, cx| action(chat, window, cx))
                })
                .unwrap()
        });
        self.settle();
    }

    /// What VoiceOver does on VO-Space: the node's own Click action, not a pointer event.
    pub fn press(&self, node: accesskit::NodeId) {
        let state = self.platform.window(self.window.into());
        let state = state.borrow();
        let callbacks = state.callbacks.as_ref().unwrap();
        (callbacks.action)(accesskit::ActionRequest {
            action: accesskit::Action::Click,
            target_tree: accesskit::TreeId::ROOT,
            target_node: node,
            data: None,
        });
        drop(state);
        self.settle();
    }

    pub fn keys(&self, keys: &[&'static str]) {
        let window: AnyWindowHandle = self.window.into();
        for key in keys {
            self.app.update(|cx| {
                cx.update_window(window, |_, window, cx| {
                    window.draw(cx).clear(cx);
                    window.dispatch_keystroke(gpui::Keystroke::parse(key).unwrap(), cx);
                })
                .unwrap()
            });
            self.platform.dispatcher.run_until_parked();
        }
        self.settle();
    }
}

/// Onboarding done and the screens library seeded, before any window exists.
fn prepare(runtime: &RuntimeHandle) {
    let mut state = runtime.state();
    while !matches!(
        *state.borrow_and_update(),
        desktop_runtime::RuntimeState::Ready
    ) {
        futures::executor::block_on(state.changed()).unwrap();
    }
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
    let reply = runtime
        .submit(move |services| async move {
            let failed = |error: sqlx::Error| ServiceError::Failed(error.to_string().into());
            sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, 'false')")
                .bind(desktop_gpui::product::onboarding::NEEDED_KEY)
                .execute(services.db.pool())
                .await
                .map_err(failed)?;
            for script in scripts {
                sqlx::raw_sql(sqlx::AssertSqlSafe(script))
                    .execute(services.db.pool())
                    .await
                    .map_err(failed)?;
            }
            Ok(())
        })
        .unwrap();
    futures::executor::block_on(reply.receive()).unwrap();
}

struct NoFiles;

impl desktop_gpui::meeting::upload::Files for NoFiles {
    fn pick(
        &self,
        _: desktop_gpui::platform::dialogs::Picker,
        _: &mut gpui::App,
    ) -> futures::future::LocalBoxFuture<'static, desktop_runtime::Result<Option<std::path::PathBuf>>>
    {
        Box::pin(async { Ok(None) })
    }

    fn open_folder(&self, _: &std::path::Path, _: &mut gpui::App) {}
}

struct NoApps;

impl desktop_gpui::product::local::imports::InstalledApps for NoApps {
    fn list(
        &self,
    ) -> desktop_runtime::Result<Vec<desktop_gpui::product::local::imports::InstalledApp>> {
        Ok(Vec::new())
    }
}

struct Reminders;

impl desktop_gpui::product::ticket::Reminders for Reminders {
    fn status(&self) -> desktop_gpui::product::ticket::Access {
        desktop_gpui::product::ticket::Access::NotDetermined
    }

    fn request(&self) -> desktop_gpui::product::ticket::Access {
        self.status()
    }
}

struct NoGithub;

impl desktop_gpui::workspace::task::Github for NoGithub {
    fn get(
        &self,
        _: String,
    ) -> futures::future::BoxFuture<'static, desktop_runtime::Result<serde_json::Value>> {
        Box::pin(async { Err(ServiceError::Unsupported("no network in tests".into())) })
    }
}
