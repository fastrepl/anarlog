use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::Arc,
};

use desktop_runtime::{CancellationToken, RuntimeHandle, ServiceError};
use futures::future::{Either, select};
use gpui::{
    AnyElement, Context, Entity, EventEmitter, Focusable, KeyDownEvent, MouseButton, Render,
    SharedString, Subscription, Window, deferred, div, point, prelude::*, px,
};
use serde_json::Value;

use crate::ui::{
    input::{InputEvent, TextInput},
    theme::{Theme, theme},
};

use super::{
    controls,
    settings::{self, Snapshot},
};

type OnSelect = Rc<dyn Fn(&mut PreferencesView, String, &mut Context<PreferencesView>)>;

pub struct SelectOption {
    pub value: SharedString,
    pub label: SharedString,
    pub detail: Option<SharedString>,
}

impl SelectOption {
    pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            detail: None,
        }
    }
}

/// The curated settings pages backed by `app_settings`/`synced_preferences`.
/// Controls write immediately, like shipping's `setSettingValues`; writes to one key are
/// serialized so each compare-and-swap uses the revision the previous write produced.
pub struct PreferencesView {
    pub(super) runtime: RuntimeHandle,
    pub section: &'static str,
    snapshot: Option<Arc<Snapshot>>,
    desired: BTreeMap<String, Value>,
    saving: BTreeSet<String>,
    pub(super) menu: Option<&'static str>,
    pub(super) menu_search: Entity<TextInput>,
    pub(super) dictionary_input: Entity<TextInput>,
    cancellation: CancellationToken,
    status: Option<String>,
    active: bool,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ServiceError> for PreferencesView {}

impl PreferencesView {
    pub fn new(runtime: RuntimeHandle, cx: &mut Context<Self>) -> Self {
        let menu_search = cx.new(|cx| TextInput::new("Search...", cx).inline());
        let subscription = cx.subscribe(&menu_search, |_, _, _: &InputEvent, cx| cx.notify());
        let dictionary_input = cx
            .new(|cx| TextInput::new("Add names, jargon, or product terms to prefer", cx).inline());
        let dictionary_subscription =
            cx.subscribe(&dictionary_input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Submitted) && super::account::is_pro(cx) {
                    this.add_dictionary_terms(cx);
                }
            });
        let mut this = Self {
            runtime,
            section: "app",
            snapshot: None,
            desired: BTreeMap::new(),
            saving: BTreeSet::new(),
            menu: None,
            menu_search,
            dictionary_input,
            cancellation: CancellationToken::new(),
            status: None,
            active: true,
            _subscriptions: vec![subscription, dictionary_subscription],
        };
        this.watch(cx);
        this
    }

    fn watch(&mut self, cx: &mut Context<Self>) {
        self.cancellation.cancel();
        self.cancellation = CancellationToken::new();
        let cancel = self.cancellation.clone();
        let reply = settings::watch(&self.runtime);
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let mut watch = match result {
                Ok(watch) => watch,
                Err(error) => {
                    if !cancel.is_cancelled() {
                        let _ = this.update(cx, |this, cx| this.fail(error, cx));
                    }
                    return;
                }
            };
            loop {
                if cancel.is_cancelled() {
                    break;
                }
                let error = watch
                    .terminal_error()
                    .or_else(|| watch.errors.try_recv().ok());
                if let Some(error) = error {
                    let _ = this.update(cx, |this, cx| this.fail(error, cx));
                    break;
                }
                let rows = watch.snapshots.borrow_and_update().rows.clone();
                let decoded = cx
                    .background_executor()
                    .spawn(async move { settings::decode(&rows).map(Arc::new) })
                    .await;
                if cancel.is_cancelled() {
                    break;
                }
                if this
                    .update(cx, |this, cx| match decoded {
                        Ok(snapshot) => {
                            this.status = None;
                            this.snapshot = Some(snapshot);
                            cx.notify();
                        }
                        Err(error) => this.fail(error, cx),
                    })
                    .is_err()
                {
                    break;
                }
                match select(
                    Box::pin(cancel.cancelled()),
                    Box::pin(watch.snapshots.changed()),
                )
                .await
                {
                    Either::Left(_) | Either::Right((Err(_), _)) => break,
                    Either::Right((Ok(()), _)) => {}
                }
            }
            let _ = watch.unsubscribe().await;
        })
        .detach();
    }

    fn fail(&mut self, error: ServiceError, cx: &mut Context<Self>) {
        self.status = Some(error.to_string());
        cx.emit(error);
        cx.notify();
    }

    pub fn loaded(&self) -> bool {
        self.snapshot.is_some()
    }

    /// The value shown for `key`: an unconfirmed write, else the stored or default value.
    pub fn value(&self, key: &str) -> Value {
        self.desired
            .get(key)
            .cloned()
            .or_else(|| {
                self.snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.get(key))
                    .map(|setting| setting.value.clone())
            })
            .or_else(|| {
                settings::definition(key)
                    .ok()
                    .map(|def| def.default.clone())
            })
            .unwrap_or(Value::Null)
    }

    pub fn flag(&self, key: &str) -> bool {
        self.value(key) == Value::Bool(true)
    }

    pub fn text(&self, key: &str) -> String {
        self.value(key).as_str().unwrap_or_default().to_owned()
    }

    pub fn writable(&self, key: &str) -> bool {
        self.loaded() && settings::write_gate(key).is_none()
    }

    pub fn set(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        if let Some(reason) = settings::write_gate(key) {
            self.fail(ServiceError::Unsupported(reason.into()), cx);
            return;
        }
        self.desired.insert(key.to_owned(), value);
        self.flush(key.to_owned(), cx);
        cx.notify();
    }

    fn flush(&mut self, key: String, cx: &mut Context<Self>) {
        if self.saving.contains(&key) {
            return;
        }
        let (Some(value), Some(base)) = (
            self.desired.get(&key).cloned(),
            self.snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.get(&key))
                .cloned(),
        ) else {
            return;
        };
        if base.value == value {
            self.desired.remove(&key);
            return;
        }
        let reply = settings::save(&self.runtime, key.clone(), base, value.clone());
        self.saving.insert(key.clone());
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                this.saving.remove(&key);
                match result {
                    Ok(setting) => {
                        if let Some(snapshot) = &mut this.snapshot {
                            Arc::make_mut(snapshot).insert(key.clone(), setting);
                        }
                        if this.desired.get(&key) == Some(&value) {
                            this.desired.remove(&key);
                        } else {
                            this.flush(key, cx);
                        }
                    }
                    Err(error) => {
                        this.desired.remove(&key);
                        this.fail(error, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn select(&mut self, section: &'static str, cx: &mut Context<Self>) {
        if !self.active {
            self.active = true;
            self.watch(cx);
        }
        if self.section != section {
            self.menu = None;
        }
        self.section = section;
        cx.notify();
    }

    pub fn suspend(&mut self) {
        self.active = false;
        self.menu = None;
        self.cancellation.cancel();
    }

    pub fn has_unsaved(&self, _: &gpui::App) -> bool {
        !self.desired.is_empty()
    }

    pub(super) fn switch_row(
        &self,
        key: &'static str,
        title: &'static str,
        detail: Option<&'static str>,
        enabled: bool,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.gated_switch_row(key, title, detail, true, enabled, colors, cx)
    }

    /// A switch row whose value only shows as on when `allowed` (shipping renders Pro-only
    /// switches as off for other plans).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn gated_switch_row(
        &self,
        key: &'static str,
        title: &'static str,
        detail: Option<&'static str>,
        allowed: bool,
        enabled: bool,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let checked = self.flag(key);
        controls::setting_row(
            title,
            detail.map(Into::into),
            false,
            controls::switch(
                SharedString::from(format!("switch-{key}")),
                allowed && checked,
                allowed && enabled && self.writable(key),
                colors,
                cx.listener(move |this, _, _, cx| this.set(key, Value::Bool(!checked), cx)),
            ),
            colors,
        )
        .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn dropdown_select(
        &self,
        id: &'static str,
        options: Vec<SelectOption>,
        selected: &str,
        placeholder: &'static str,
        enabled: bool,
        searchable: bool,
        width: f32,
        on_select: impl Fn(&mut Self, String, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = theme(window);
        let label = options
            .iter()
            .find(|option| option.value.as_ref() == selected)
            .map(|option| match &option.detail {
                Some(detail) if searchable => format!("{} ({detail})", option.label).into(),
                _ => option.label.clone(),
            });
        let open = self.menu == Some(id);
        let trigger = controls::select_trigger(id, label, placeholder, enabled, &colors).when(
            enabled,
            |view| {
                view.on_click(cx.listener(move |this, _, window, cx| {
                    this.menu = Some(id);
                    if searchable {
                        this.menu_search.update(cx, |search, cx| {
                            search.set_text(String::new(), cx);
                            search.focus_handle(cx).focus(window, cx);
                        });
                    }
                    cx.notify();
                }))
            },
        );
        let on_select: OnSelect = Rc::new(on_select);
        let query = self.menu_search.read(cx).buffer.text.to_lowercase();
        let menu = open.then(|| {
            let visible = options
                .into_iter()
                .filter(|option| {
                    !searchable
                        || query.trim().is_empty()
                        || format!(
                            "{} {}",
                            option.label,
                            option.detail.as_deref().unwrap_or_default()
                        )
                        .to_lowercase()
                        .contains(query.trim())
                })
                .collect::<Vec<_>>();
            let empty = visible.is_empty();
            controls::menu_panel(width.max(controls::CONTROL_WIDTH), &colors)
                .when(searchable, |view| {
                    view.child(
                        div()
                            .h(px(36.))
                            .px_2()
                            .mb_1()
                            .flex()
                            .items_center()
                            .gap_2()
                            .border_b_1()
                            .border_color(colors.border)
                            .text_sm()
                            .child(controls::icon(
                                "Search01Icon.svg",
                                16.,
                                colors.muted_foreground,
                            ))
                            .child(div().flex_1().h(px(20.)).child(self.menu_search.clone())),
                    )
                })
                .child(
                    div()
                        .id(SharedString::from(format!("{id}-options")))
                        .max_h(px(250.))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .when(empty, |view| {
                            view.child(
                                div()
                                    .px_2()
                                    .py(px(6.))
                                    .text_sm()
                                    .text_color(colors.muted_foreground)
                                    .child("No results found."),
                            )
                        })
                        .children(visible.into_iter().map(|option| {
                            let value = option.value.to_string();
                            let on_select = on_select.clone();
                            controls::menu_item(
                                SharedString::from(format!("{id}-{value}")),
                                option.label,
                                option.detail,
                                value == selected,
                                &colors,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.menu = None;
                                    on_select(this, value.clone(), cx);
                                    cx.notify();
                                },
                            ))
                        })),
                )
        });
        div()
            .relative()
            .w_full()
            .child(trigger)
            .when_some(menu, |view, menu| {
                view.child(controls::dropdown(menu, width > controls::CONTROL_WIDTH))
            })
            .into_any_element()
    }

    /// Closes an open menu on any click outside it, without the click reaching the page.
    fn backdrop(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let size = window.viewport_size();
        deferred(
            gpui::anchored().position(point(px(0.), px(0.))).child(
                div()
                    .id("settings-menu-backdrop")
                    .w(size.width)
                    .h(size.height)
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.menu = None;
                            cx.notify();
                        }),
                    ),
            ),
        )
        .with_priority(0)
        .into_any_element()
    }
}

impl Drop for PreferencesView {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

impl Render for PreferencesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let page = match self.section {
            "app" => self.general(window, cx),
            "appearance" => self.appearance(window, cx),
            "notifications" => self.notifications(window, cx),
            "meetings" => self.meetings(window, cx),
            "privacy" => self.privacy(window, cx),
            "dictation" => self.dictation(window, cx),
            "dictionary" => self.dictionary(window, cx),
            _ => div().into_any_element(),
        };
        div()
            .flex()
            .flex_col()
            .gap_4()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" && this.menu.take().is_some() {
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .when_some(self.status.clone(), |view, status| {
                view.child(div().text_sm().text_color(colors.destructive).child(status))
            })
            .child(page)
            .when(self.menu.is_some(), |view| {
                view.child(self.backdrop(window, cx))
            })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use desktop_runtime::Profile;
    use gpui::{AppContext, TestAppContext};

    use super::*;

    /// Runs foreground work until `done`, letting replies from the runtime's threads land.
    fn settle(cx: &mut TestAppContext, done: impl Fn(&mut TestAppContext) -> bool) {
        for _ in 0..400 {
            cx.run_until_parked();
            if done(cx) {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("settings view did not settle");
    }

    #[gpui::test]
    fn rapid_changes_to_one_setting_persist_the_last_value(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        futures::executor::block_on(ready.receive()).unwrap();
        let view = cx.new(|cx| PreferencesView::new(runtime.clone(), cx));
        settle(cx, |cx| view.read_with(cx, |view, _| view.loaded()));

        view.update(cx, |view, cx| {
            for value in [true, false, true] {
                view.set("use_24_hour_time", Value::Bool(value), cx);
            }
            assert!(view.flag("use_24_hour_time"));
        });
        settle(cx, |cx| {
            view.read_with(cx, |view, cx| !view.has_unsaved(cx))
        });

        view.read_with(cx, |view, _| assert_eq!(view.status, None));
        let stored = futures::executor::block_on(async {
            let watch = settings::watch(&runtime).unwrap().receive().await.unwrap();
            let snapshot = settings::decode(&watch.snapshots.borrow().rows).unwrap();
            watch.unsubscribe().await.unwrap();
            snapshot
        });
        assert_eq!(stored["use_24_hour_time"].value, Value::Bool(true));
        assert_eq!(stored["use_24_hour_time"].source, "Synced");
        drop(view);
        futures::executor::block_on(runtime.shutdown()).unwrap();
    }
}
