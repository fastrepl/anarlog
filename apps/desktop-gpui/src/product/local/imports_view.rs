//! `settings/imports/index.tsx` and `MeetingImportScreen` (`imports/screen.tsx`).

use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use desktop_runtime::{CancellationToken, Result, RuntimeHandle, ServiceError};
use gpui::{
    Animation, AnimationExt, AnyElement, App, ClickEvent, Context, Div, ElementId, EventEmitter,
    Hsla, Render, SharedString, Stateful, Subscription, Task, Transformation, Window, div, img,
    percentage, prelude::*, px, relative,
};
use serde_json::{Value, json};

use super::imports::{self, Direct, ImportResult, ImportRun, Mode, Provider, TextFile};
use crate::{
    application::toasts,
    product::{
        account,
        controls::{self, dropdown, icon, menu_panel},
        integrations::{self, Connection, Integrations},
    },
    ui::theme::{MEDIUM, Theme, theme},
};

const SYNC_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Asks the settings pane to start sign-in.
pub struct SignInRequested;

enum Detection {
    Loading,
    Ready(Vec<&'static Provider>),
    Failed(String),
}

#[derive(Default)]
struct Row {
    open: bool,
    importing: bool,
    connecting: Option<CancellationToken>,
    syncing: bool,
    disconnecting: bool,
    result: Option<ImportResult>,
    warnings: Vec<String>,
    error: Option<String>,
}

pub struct ImportsView {
    runtime: RuntimeHandle,
    detection: Detection,
    history: Vec<ImportRun>,
    /// The account's integrations; `None` while they load.
    connections: Option<std::result::Result<Vec<Connection>, String>>,
    signed_in: bool,
    rows: HashMap<&'static str, Row>,
    menu: Option<&'static str>,
    _sync: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<SignInRequested> for ImportsView {}

impl ImportsView {
    pub fn new(runtime: RuntimeHandle, cx: &mut Context<Self>) -> Self {
        let sync = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(SYNC_INTERVAL).await;
                if this.update(cx, |this, cx| this.sync_connected(cx)).is_err() {
                    break;
                }
            }
        });
        let mut this = Self {
            runtime,
            detection: Detection::Loading,
            history: Vec::new(),
            connections: None,
            signed_in: account::current(cx).signed_in(),
            rows: HashMap::new(),
            menu: None,
            _sync: sync,
            _subscriptions: vec![cx.observe_global::<account::Account>(|this, cx| {
                let signed_in = account::current(cx).signed_in();
                if signed_in != this.signed_in {
                    this.signed_in = signed_in;
                    this.load_connections(cx);
                }
            })],
        };
        this.detect(cx);
        this.load_history(cx);
        this.load_connections(cx);
        this
    }

    fn service(&self, cx: &App) -> Option<Arc<dyn Integrations>> {
        integrations::installed(cx).filter(|_| self.signed_in)
    }

    fn detect(&mut self, cx: &mut Context<Self>) {
        let apps = imports::installed_apps(cx);
        let detection = cx.background_executor().spawn(async move { apps.list() });
        cx.spawn(async move |this, cx| {
            let detection = match detection.await {
                Ok(apps) => Detection::Ready(imports::detect(&apps)),
                Err(error) => Detection::Failed(error.to_string()),
            };
            let _ = this.update(cx, |this, cx| {
                this.detection = detection;
                cx.notify();
            });
        })
        .detach();
    }

    fn load_history(&mut self, cx: &mut Context<Self>) {
        let runtime = self.runtime.clone();
        cx.spawn(async move |this, cx| {
            if let Ok(history) = imports::history(&runtime).await {
                let _ = this.update(cx, |this, cx| {
                    this.history = history;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn load_connections(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.service(cx) else {
            self.connections = None;
            cx.notify();
            return;
        };
        self.connections = None;
        let request = service.connections();
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |this, cx| {
                this.connections = Some(result.map_err(|error| error.to_string()));
                this.sync_connected(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn row(&mut self, provider: &'static Provider) -> &mut Row {
        self.rows.entry(provider.id).or_default()
    }

    fn connection(&self, provider: &Provider) -> Option<&Connection> {
        let integration = provider.nango?;
        let connections = self.connections.as_ref()?.as_ref().ok()?;
        connections
            .iter()
            .find(|connection| connection.integration_id == integration)
    }

    fn connected(&self, provider: &Provider) -> bool {
        self.signed_in && self.connection(provider).is_some_and(Connection::ready)
    }

    fn checking(&self, provider: &Provider) -> bool {
        self.signed_in && provider.nango.is_some() && self.connections.is_none()
    }

    fn sync_connected(&mut self, cx: &mut Context<Self>) {
        let Detection::Ready(providers) = &self.detection else {
            return;
        };
        for provider in providers.clone() {
            if self.connected(provider) {
                self.sync(provider, cx);
            }
        }
    }

    /// `syncNangoMeetings`: the API returns meetings not yet imported, which are imported here.
    fn sync(&mut self, provider: &'static Provider, cx: &mut Context<Self>) {
        let (Some(service), Some(connection)) = (
            self.service(cx),
            self.connection(provider).map(|c| c.connection_id.clone()),
        ) else {
            return;
        };
        let row = self.row(provider);
        if row.syncing {
            return;
        }
        row.syncing = true;
        let runtime = self.runtime.clone();
        cx.spawn(async move |this, cx| {
            let result = sync(&runtime, service.as_ref(), provider, connection).await;
            let _ = this.update(cx, |this, cx| {
                let row = this.row(provider);
                row.syncing = false;
                match result {
                    Ok((result, warnings)) => {
                        row.result = Some(result);
                        row.warnings = warnings;
                        row.error = None;
                    }
                    Err(error) => row.error = Some(error.to_string()),
                }
                this.load_history(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn connect(&mut self, provider: &'static Provider, cx: &mut Context<Self>) {
        if !self.signed_in {
            cx.emit(SignInRequested);
            return;
        }
        if let Some(cancel) = self.row(provider).connecting.take() {
            cancel.cancel();
            cx.notify();
            return;
        }
        let (Some(service), Some(integration)) = (self.service(cx), provider.nango) else {
            let row = self.row(provider);
            row.open = true;
            row.error = Some(match provider.direct {
                Some(Direct::McpOauth | Direct::Cli) => format!(
                    "{} connections aren't available in this version of Anarlog yet. Use files instead.",
                    provider.name
                ),
                _ => integrations::SETUP_ERROR.into(),
            });
            cx.notify();
            return;
        };
        let cancel = CancellationToken::new();
        let row = self.row(provider);
        row.connecting = Some(cancel.clone());
        row.error = None;
        cx.spawn(async move |this, cx| {
            let result = match service.open(integration, "connect", None, "imports").await {
                Ok(()) => {
                    integrations::wait_for_connection(
                        service.clone(),
                        provider.name,
                        integration,
                        cancel.clone(),
                        cx,
                    )
                    .await
                }
                Err(_) => Err(integrations::failure(integrations::SETUP_ERROR)),
            };
            let _ = this.update(cx, |this, cx| {
                let row = this.row(provider);
                row.connecting = None;
                match result {
                    Ok(connection) => {
                        if let Some(Ok(connections)) = &mut this.connections {
                            connections.retain(|c| c.connection_id != connection.connection_id);
                            connections.insert(0, connection);
                        } else {
                            this.connections = Some(Ok(vec![connection]));
                        }
                        this.sync(provider, cx);
                    }
                    Err(ServiceError::Cancelled) => {}
                    Err(error) => row.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn disconnect(&mut self, provider: &'static Provider, cx: &mut Context<Self>) {
        self.menu = None;
        let (Some(service), Some(integration), Some(connection)) = (
            self.service(cx),
            provider.nango,
            self.connection(provider).map(|c| c.connection_id.clone()),
        ) else {
            return;
        };
        self.row(provider).disconnecting = true;
        cx.spawn(async move |this, cx| {
            let result = service
                .open(integration, "disconnect", Some(connection), "imports")
                .await;
            let _ = this.update(cx, |this, cx| {
                let row = this.row(provider);
                row.disconnecting = false;
                if result.is_ok() {
                    row.result = None;
                    row.warnings.clear();
                    this.load_connections(cx);
                } else {
                    toasts::error(integrations::SETUP_ERROR, cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn choose_files(&mut self, provider: &'static Provider, cx: &mut Context<Self>) {
        self.menu = None;
        if self.rows.values().any(|row| row.importing) {
            return;
        }
        let picker = crate::platform::os::dialog(|| {
            rfd::AsyncFileDialog::new()
                .set_title(format!("Choose {} export files", provider.name))
                .add_filter("Meeting exports", imports::EXTENSIONS)
                .pick_files()
        });
        cx.spawn(async move |this, cx| {
            let Some(files) = picker.await else {
                return;
            };
            let paths = files.iter().map(|file| file.path().to_owned()).collect();
            let _ = this.update(cx, |this, cx| this.import_files(provider, paths, cx));
        })
        .detach();
        cx.notify();
    }

    pub fn import_files(
        &mut self,
        provider: &'static Provider,
        paths: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let row = self.row(provider);
        row.importing = true;
        row.error = None;
        let runtime = self.runtime.clone();
        let read = cx
            .background_executor()
            .spawn(async move { imports::read_files(paths) });
        cx.spawn(async move |this, cx| {
            let result = match read.await {
                Ok(files) => imports::run(&runtime, provider.id, files, Mode::Export).await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                let row = this.row(provider);
                row.importing = false;
                match result {
                    Ok(result) => {
                        row.result = Some(result);
                        row.warnings.clear();
                    }
                    Err(error) => row.error = Some(error.to_string()),
                }
                this.load_history(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn toggle(&mut self, provider: &'static Provider, cx: &mut Context<Self>) {
        let row = self.row(provider);
        row.open = !row.open;
        cx.notify();
    }

    fn toggle_menu(&mut self, provider: &'static Provider, cx: &mut Context<Self>) {
        self.menu = (self.menu != Some(provider.id)).then_some(provider.id);
        cx.notify();
    }
}

async fn sync(
    runtime: &RuntimeHandle,
    service: &dyn Integrations,
    provider: &'static Provider,
    connection: String,
) -> Result<(ImportResult, Vec<String>)> {
    let known = imports::known_ids(runtime, provider.id).await?;
    let response = service
        .post(
            format!("/{}/import-meetings", provider.id),
            json!({"connection_id": connection, "known_meeting_ids": known}),
        )
        .await
        .map_err(|_| {
            integrations::failure(format!("Reconnect {} to keep importing", provider.name))
        })?;
    let files = response["files"]
        .as_array()
        .map(|files| {
            files
                .iter()
                .filter_map(|file| {
                    Some(TextFile {
                        path: file["path"].as_str()?.to_owned(),
                        content: file["content"].as_str()?.to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let warnings = response["warnings"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let result = imports::run(runtime, provider.id, files, Mode::Connection).await?;
    Ok((result, warnings))
}

pub(crate) fn spinner(id: impl Into<ElementId>, size: f32, color: Hsla) -> AnyElement {
    icon("product/LoaderCircleIcon.svg", size, color)
        .with_animation(
            id,
            Animation::new(Duration::from_secs(1)).repeat(),
            |icon, delta| icon.with_transformation(Transformation::rotate(percentage(delta))),
        )
        .into_any_element()
}

/// The provider logo slot (`size-8`): a bundled logo or the name's initial on a tile.
pub(crate) fn provider_icon(name: &str, logo: Option<&'static str>, colors: &Theme) -> Div {
    div()
        .size(px(32.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .map(|slot| match logo {
            Some(path) if path.ends_with(".svg") => slot.child(
                gpui::svg()
                    .path(path)
                    .size(px(32.))
                    .text_color(colors.foreground),
            ),
            Some(path) => slot.child(img(path).size(px(32.))),
            None => slot.child(
                div()
                    .size(px(32.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(colors.muted)
                    .text_color(colors.muted_foreground)
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(name.chars().next().map(String::from).unwrap_or_default()),
            ),
        })
}

/// `ConnectButtonGroup`: a fixed `w-40` pair of small buttons, primary-filled or outlined,
/// with a sliding highlight while `busy`.
pub(crate) fn connect_group(
    id: impl Into<SharedString>,
    primary: bool,
    busy: bool,
    colors: &Theme,
) -> Div {
    let id = id.into();
    div()
        .relative()
        .w(px(160.))
        .h(px(28.))
        .flex_none()
        .flex()
        .overflow_hidden()
        .map(|group| {
            if primary {
                group.bg(colors.primary)
            } else {
                group.border_1().border_color(colors.input)
            }
        })
        .when(busy, |group| {
            group.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .w(relative(1. / 3.))
                    .bg(colors.primary_foreground.opacity(0.2))
                    .with_animation(
                        ElementId::Name(format!("{id}-busy").into()),
                        Animation::new(Duration::from_millis(1400)).repeat(),
                        |bar, delta| bar.left(relative(-1. / 3. + delta * 4. / 3.)),
                    ),
            )
        })
}

/// A `size="sm"` button inside a connect group.
pub(crate) fn group_button(
    id: impl Into<ElementId>,
    primary: bool,
    enabled: bool,
    colors: &Theme,
) -> Stateful<Div> {
    let hover = colors.primary_foreground.opacity(0.1);
    div()
        .id(id)
        .relative()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .gap_2()
        .text_xs()
        .font_weight(MEDIUM)
        .map(|button| {
            if primary {
                button.text_color(colors.primary_foreground)
            } else {
                button.bg(colors.muted).text_color(colors.foreground)
            }
        })
        .when(!enabled, |button| button.opacity(0.5))
        .when(enabled && primary, |button| {
            button.cursor_pointer().hover(move |style| style.bg(hover))
        })
}

/// The caret half of a connect group, with the divider shipping draws in `before:`.
pub(crate) fn group_caret(
    id: impl Into<ElementId>,
    primary: bool,
    enabled: bool,
    colors: &Theme,
) -> Stateful<Div> {
    let color = if primary {
        colors.primary_foreground
    } else {
        colors.foreground
    };
    group_button(id, primary, enabled, colors)
        .w(px(24.))
        .flex_none()
        .when(enabled, |button| button.cursor_pointer())
        .child(
            div()
                .absolute()
                .left_0()
                .top(px(6.))
                .bottom(px(6.))
                .w(px(1.))
                .bg(if primary {
                    colors.primary_foreground.opacity(0.2)
                } else {
                    colors.border
                }),
        )
        .child(icon("product/ArrowDown01Icon.svg", 14., color))
}

/// An app dropdown item with an optional leading icon.
pub(crate) fn menu_entry(
    id: impl Into<ElementId>,
    label: &'static str,
    leading: Option<&'static str>,
    enabled: bool,
    colors: &Theme,
) -> Stateful<Div> {
    let hover = colors.accent;
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .py(px(6.))
        .text_sm()
        .line_height(px(20.))
        .when(!enabled, |item| item.opacity(0.5))
        .when(enabled, |item| {
            item.cursor_pointer().hover(move |style| style.bg(hover))
        })
        .when_some(leading, |item, path| {
            item.child(icon(path, 16., colors.foreground))
        })
        .child(label)
}

/// A label that swaps for another while its group is hovered, keeping the wider width.
pub(crate) fn hover_label(group: SharedString, idle: &'static str, hovered: &'static str) -> Div {
    div()
        .relative()
        .flex()
        .flex_col()
        .items_center()
        .child(div().invisible().child(idle))
        .child(div().invisible().h_0().overflow_hidden().child(hovered))
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .group_hover(group.clone(), |style| style.invisible())
                .child(idle),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .invisible()
                .group_hover(group, |style| style.visible())
                .child(hovered),
        )
}

impl ImportsView {
    fn control(
        &self,
        provider: &'static Provider,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let row = self.rows.get(provider.id);
        let importing = row.is_some_and(|row| row.importing);
        let any_importing = self.rows.values().any(|row| row.importing);
        if !provider.connected() {
            let hover = colors.accent;
            return div()
                .id(SharedString::from(format!("import-files-{}", provider.id)))
                .w(px(160.))
                .h(px(28.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .gap_2()
                .border_1()
                .border_color(colors.input)
                .bg(colors.background)
                .shadow_xs()
                .text_xs()
                .font_weight(MEDIUM)
                .when(any_importing, |button| button.opacity(0.5))
                .when(!any_importing, |button| {
                    button
                        .cursor_pointer()
                        .hover(move |style| style.bg(hover))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.choose_files(provider, cx)
                        }))
                })
                .child(if importing {
                    spinner(
                        SharedString::from(format!("import-spinner-{}", provider.id)),
                        14.,
                        colors.foreground,
                    )
                } else {
                    icon("product/Download01Icon.svg", 14., colors.foreground).into_any_element()
                })
                .child("Choose files")
                .into_any_element();
        }
        let id = provider.id;
        let connected = self.connected(provider);
        let syncing = row.is_some_and(|row| row.syncing);
        let disconnecting = row.is_some_and(|row| row.disconnecting);
        let connecting = row.is_some_and(|row| row.connecting.is_some());
        let checking = self.checking(provider);
        let signed_in = self.signed_in;
        let menu_open = self.menu == Some(id);
        let fg = colors.primary_foreground;
        let (group, caret, items): (Div, Stateful<Div>, Vec<Stateful<Div>>) = if connected {
            let label_group = SharedString::from(format!("import-sync-{id}"));
            let enabled = !syncing && !disconnecting;
            let group = connect_group(format!("import-{id}"), true, syncing, colors).child(
                group_button(
                    SharedString::from(format!("import-sync-{id}")),
                    true,
                    enabled,
                    colors,
                )
                .group(label_group.clone())
                .flex_1()
                .min_w_0()
                .when(enabled, |button| {
                    button.on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| this.sync(provider, cx)),
                    )
                })
                .child(if syncing {
                    spinner(SharedString::from(format!("sync-spinner-{id}")), 14., fg)
                } else {
                    icon("product/RefreshIcon.svg", 14., fg).into_any_element()
                })
                .child(hover_label(label_group, "Connected", "Sync now")),
            );
            let caret = group_caret(
                SharedString::from(format!("import-more-{id}")),
                true,
                enabled,
                colors,
            );
            let items =
                vec![
                    menu_entry(
                        SharedString::from(format!("import-disconnect-{id}")),
                        "Disconnect",
                        None,
                        enabled,
                        colors,
                    )
                    .when(enabled, |item| {
                        item.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.disconnect(provider, cx)
                        }))
                    }),
                    menu_entry(
                        SharedString::from(format!("import-use-files-{id}")),
                        "Use files",
                        Some("product/Download01Icon.svg"),
                        !any_importing,
                        colors,
                    )
                    .when(!any_importing, |item| {
                        item.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.choose_files(provider, cx)
                        }))
                    }),
                ];
            (group, caret, items)
        } else {
            let enabled = !(signed_in && checking);
            let label_group = SharedString::from(format!("import-connect-{id}"));
            let primary = colors.primary;
            let text = if signed_in { fg } else { colors.foreground };
            let button = group_button(
                SharedString::from(format!("import-connect-{id}")),
                signed_in,
                enabled,
                colors,
            )
            .group(label_group.clone())
            .flex_1()
            .min_w_0()
            .when(enabled, |button| {
                button.on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| this.connect(provider, cx)),
                )
            })
            .when(!signed_in, |button| {
                button
                    .cursor_pointer()
                    .hover(move |style| style.bg(primary).text_color(fg))
            })
            .map(|button| {
                if !signed_in {
                    return button.child(
                        div()
                            .relative()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(div().invisible().child("Sign in to connect"))
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap_2()
                                    .group_hover(label_group.clone(), |style| style.invisible())
                                    .child(icon("product/ElectricPlugsIcon.svg", 14., text))
                                    .child("Connect"),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .invisible()
                                    .group_hover(label_group.clone(), |style| style.visible())
                                    .child("Sign in to connect"),
                            ),
                    );
                }
                button
                    .child(if checking || connecting {
                        spinner(
                            SharedString::from(format!("connect-spinner-{id}")),
                            14.,
                            text,
                        )
                    } else {
                        icon("product/ElectricPlugsIcon.svg", 14., text).into_any_element()
                    })
                    .child(if connecting {
                        "Cancel"
                    } else if checking {
                        "Checking connection"
                    } else {
                        "Connect"
                    })
            });
            let group =
                connect_group(format!("import-{id}"), signed_in, false, colors).child(button);
            let caret = group_caret(
                SharedString::from(format!("import-files-menu-{id}")),
                signed_in,
                !any_importing,
                colors,
            );
            let items = vec![
                menu_entry(
                    SharedString::from(format!("import-use-files-{id}")),
                    "Use files",
                    Some("product/Download01Icon.svg"),
                    true,
                    colors,
                )
                .on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| this.choose_files(provider, cx)),
                ),
            ];
            (group, caret, items)
        };
        let caret_enabled = if connected {
            !syncing && !disconnecting
        } else {
            !any_importing
        };
        let caret = caret.when(caret_enabled, |caret| {
            caret.on_click(
                cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_menu(provider, cx)),
            )
        });
        div()
            .relative()
            .flex_none()
            .flex()
            .items_center()
            .gap_1()
            .child(group.child(caret))
            .when(menu_open, |view| {
                view.child(dropdown(menu_panel(160., colors).children(items), true))
            })
            .into_any_element()
    }

    fn content(&self, provider: &Provider, colors: &Theme) -> Div {
        let row = self.rows.get(provider.id);
        let result = row.and_then(|row| row.result);
        let error = row.and_then(|row| row.error.clone());
        let last = self
            .history
            .iter()
            .find(|run| run.provider_id == provider.id);
        let line = |text: String| {
            div()
                .mt_1()
                .text_xs()
                .line_height(px(16.))
                .text_color(colors.muted_foreground)
                .child(text)
        };
        div()
            .px_4()
            .pb_4()
            .pl(px(60.))
            .when(self.connected(provider), |view| {
                view.child(
                    div()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(colors.muted_foreground)
                        .child("New meetings are imported automatically while Anarlog is running."),
                )
            })
            .when_some(last.filter(|_| result.is_none()), |view, run| {
                view.child(line(format!(
                    "Last import: {} added, {} unchanged",
                    run.result.imported, run.result.matched
                )))
            })
            .when(
                result.is_none() && last.is_none() && error.is_none(),
                |view| view.child(line("No imports yet.".into())),
            )
            .when_some(result, |view, result| {
                view.child(line(if result.errors > 0 || result.conflicts > 0 {
                    format!(
                        "Imported: {}. Unchanged: {}. Needs review: {}. Failed: {}.",
                        result.imported, result.matched, result.conflicts, result.errors
                    )
                } else {
                    format!(
                        "Last import: {} added, {} unchanged",
                        result.imported, result.matched
                    )
                }))
            })
            .children(
                row.map(|row| row.warnings.clone())
                    .unwrap_or_default()
                    .into_iter()
                    .map(line),
            )
            .when_some(error, |view, error| {
                view.child(
                    div()
                        .mt_1()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(colors.destructive)
                        .child(error),
                )
            })
    }

    fn provider_row(
        &self,
        provider: &'static Provider,
        first: bool,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let open = self.rows.get(provider.id).is_some_and(|row| row.open);
        div()
            .when(!first, |row| row.border_t_1().border_color(colors.border))
            .child(
                div()
                    .min_h(px(64.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .child(
                        div()
                            .id(SharedString::from(format!(
                                "import-provider-{}",
                                provider.id
                            )))
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap_3()
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.toggle(provider, cx)
                            }))
                            .child(provider_icon(provider.name, provider.icon(), colors))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .line_height(px(20.))
                                    .font_weight(MEDIUM)
                                    .child(provider.name),
                            )
                            .child(
                                icon("product/ArrowDown01Icon.svg", 14., colors.muted_foreground)
                                    .when(open, |caret| {
                                        caret.with_transformation(Transformation::rotate(
                                            percentage(0.5),
                                        ))
                                    }),
                            ),
                    )
                    .child(self.control(provider, colors, cx)),
            )
            .when(open, |row| row.child(self.content(provider, colors)))
    }
}

impl Render for ImportsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let ghost_hover = colors.accent;
        let title = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(controls::page_title("Imports"))
            .child(
                div()
                    .id("imports-documentation")
                    .h(px(28.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .font_weight(MEDIUM)
                    .cursor_pointer()
                    .hover(move |style| style.bg(ghost_hover))
                    .on_click(|_, _, cx| cx.open_url(imports::DOCUMENTATION_URL))
                    .child("Documentation")
                    .child(icon("product/ExternalLinkIcon.svg", 14., colors.foreground)),
            );
        let providers = match &self.detection {
            Detection::Ready(providers) => providers.clone(),
            _ => Vec::new(),
        };
        let settled = matches!(self.detection, Detection::Ready(_));
        let connections_error = self
            .connections
            .as_ref()
            .and_then(|connections| connections.as_ref().err().cloned())
            .filter(|_| self.signed_in);
        let screen = div()
            .flex()
            .flex_col()
            .gap_4()
            .when(matches!(self.detection, Detection::Loading), |view| {
                view.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .text_color(colors.muted_foreground)
                        .child(spinner("imports-detecting", 14., colors.muted_foreground))
                        .child("Checking installed meeting assistants…"),
                )
            })
            .when_some(
                match &self.detection {
                    Detection::Failed(error) => Some(error.clone()),
                    _ => None,
                },
                |view, error| {
                    view.child(div().text_xs().text_color(colors.destructive).child(error))
                },
            )
            .when_some(connections_error, |view, error| {
                view.child(div().text_sm().text_color(colors.destructive).child(error))
            })
            .when(settled, |view| {
                view.child(
                    div()
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.card)
                        .overflow_hidden()
                        .when(providers.is_empty(), |card| {
                            card.child(
                                div()
                                    .px_4()
                                    .py_6()
                                    .text_center()
                                    .text_sm()
                                    .text_color(colors.muted_foreground)
                                    .child("No apps found."),
                            )
                        })
                        .children(providers.iter().enumerate().map(|(index, provider)| {
                            self.provider_row(provider, index == 0, &colors, cx)
                        })),
                )
            });
        div()
            .flex()
            .flex_col()
            .gap_8()
            .child(title)
            .child(screen)
            .when(self.menu.is_some(), |view| {
                view.child(
                    gpui::deferred(
                        gpui::anchored()
                            .position(gpui::point(px(0.), px(0.)))
                            .child(
                                div()
                                    .id("imports-menu-backdrop")
                                    .w(window.viewport_size().width)
                                    .h(window.viewport_size().height)
                                    .occlude()
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.menu = None;
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    )
                    .with_priority(0),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use desktop_runtime::Profile;
    use gpui::{AppContext, TestAppContext};

    use super::*;
    use crate::product::{
        account::{Account, User},
        integrations::fake::FakeIntegrations,
    };

    struct Apps;

    impl imports::InstalledApps for Apps {
        fn list(&self) -> Result<Vec<imports::InstalledApp>> {
            Ok(Vec::new())
        }
    }

    fn settle(cx: &mut TestAppContext, done: impl Fn(&mut TestAppContext) -> bool) {
        for _ in 0..400 {
            cx.run_until_parked();
            if done(cx) {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("imports view did not settle");
    }

    #[gpui::test]
    fn connecting_google_meet_opens_the_integration_and_imports_new_meetings(
        cx: &mut TestAppContext,
    ) {
        cx.executor().allow_parking();
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        futures::executor::block_on(ready.receive()).unwrap();
        let fake = Arc::new(FakeIntegrations::default());
        *fake.on_connect.lock().unwrap() =
            vec![FakeIntegrations::connection("google-meet", "c1", None)];
        let meeting = json!({"id": "meet-1", "title": "Design review", "notes": "Ship it"});
        fake.responses.lock().unwrap().push((
            "/google-meet/import-meetings".into(),
            Ok(json!({"files": [{"path": "oauth://google-meet/meet-1.json", "name": "meet-1.json", "content": meeting.to_string()}], "warnings": []})),
        ));
        cx.update(|cx| {
            cx.set_global(integrations::Installed(fake.clone()));
            cx.set_global(imports::InstalledAppsOverride(Arc::new(Apps)));
            account::set(
                Account {
                    user: Some(User {
                        id: "user".into(),
                        email: None,
                    }),
                    ..Default::default()
                },
                cx,
            );
        });
        let view = cx.new(|cx| ImportsView::new(runtime.clone(), cx));
        settle(cx, |cx| {
            view.read_with(cx, |view, _| {
                matches!(view.detection, Detection::Ready(_)) && view.connections.is_some()
            })
        });
        let meet = &imports::PROVIDERS
            .iter()
            .find(|provider| provider.id == "google-meet")
            .unwrap();
        view.update(cx, |view, cx| view.connect(meet, cx));
        settle(cx, |cx| {
            view.read_with(cx, |view, _| {
                view.rows
                    .get("google-meet")
                    .is_some_and(|row| row.result.is_some())
            })
        });

        assert_eq!(
            fake.opened.lock().unwrap()[0],
            (
                "google-meet".into(),
                "connect".into(),
                None,
                "imports".into()
            )
        );
        assert_eq!(
            fake.posts.lock().unwrap()[0].1,
            json!({"connection_id": "c1", "known_meeting_ids": []})
        );
        view.read_with(cx, |view, _| {
            assert_eq!(view.rows["google-meet"].result.unwrap().imported, 1)
        });
        let known =
            futures::executor::block_on(imports::known_ids(&runtime, "google-meet")).unwrap();
        assert_eq!(known, vec!["meet-1".to_owned()]);
        drop(view);
        futures::executor::block_on(runtime.shutdown()).unwrap();
    }
}
