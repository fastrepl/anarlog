use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use desktop_runtime::{CancellationToken, Reply, Result, RuntimeHandle};
use futures::{
    FutureExt,
    future::{Either, select},
};
use gpui::{
    Animation, AnimationExt, App, Context, Entity, EventEmitter, FontWeight, Hsla, MouseButton,
    MouseDownEvent, Pixels, Point, Render, SharedString, Subscription, Transformation, Window,
    anchored, deferred, div, img, percentage, prelude::*, px, radians, rgb, svg,
};
use serde_json::{Value, json};

use super::{
    calendar::parse_color,
    calendar_accounts::{
        Accounts, AppleAccess, CalendarAccounts, CalendarSync, Connection, Flow, SyncStatus,
    },
    mutations::{statement, transaction},
    resource_list::{dialog_button, glass_dialog, modal},
};
use crate::ui::theme::{Theme, theme};

pub enum CalendarSidebarEvent {
    Back,
    /// Account services are unavailable here; the product Calendar screen explains why.
    Connect,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Provider {
    Apple,
    Google,
    Outlook,
}

impl Provider {
    fn id(self) -> &'static str {
        match self {
            Self::Apple => "apple",
            Self::Google => "google",
            Self::Outlook => "outlook",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Apple => "Apple Calendar",
            Self::Google => "Google",
            Self::Outlook => "Outlook",
        }
    }

    /// Apple Calendar is read through EventKit; the others are cloud account integrations.
    fn cloud(self) -> bool {
        self != Self::Apple
    }

    fn visible() -> &'static [Self] {
        if cfg!(target_os = "macos") {
            &[Self::Apple, Self::Google, Self::Outlook]
        } else {
            &[Self::Google, Self::Outlook]
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CalendarRow {
    pub id: Arc<str>,
    pub name: Arc<str>,
    pub enabled: bool,
    pub provider: Arc<str>,
    pub source: Arc<str>,
    pub color: Arc<str>,
    pub connection: Arc<str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CalendarGroup {
    pub source: Arc<str>,
    /// The account the group's calendars came from, when known.
    pub connection: Arc<str>,
    pub calendars: Vec<CalendarRow>,
}

const SUBSCRIBED: &str = "Subscribed Calendars";

/// Groups a provider's calendars by account/source like the shipping sidebar.
pub fn groups(provider: Provider, rows: &[CalendarRow]) -> Vec<CalendarGroup> {
    let rows = rows
        .iter()
        .filter(|row| &*row.provider == provider.id())
        .collect::<Vec<_>>();
    let sources = rows
        .iter()
        .filter(|row| !row.source.is_empty() && !row.connection.is_empty())
        .map(|row| (row.connection.clone(), row.source.clone()))
        .collect::<HashMap<_, _>>();
    let named = rows
        .iter()
        .filter_map(|row| {
            (!row.source.is_empty())
                .then(|| row.source.clone())
                .or_else(|| sources.get(&row.connection).cloned())
        })
        .collect::<HashSet<_>>();
    let single = (named.len() == 1)
        .then(|| named.iter().next().cloned())
        .flatten();
    let mut groups = Vec::<CalendarGroup>::new();
    for row in rows {
        let source = if !row.source.is_empty() {
            row.source.clone()
        } else if !provider.cloud() {
            "Apple Calendar".into()
        } else {
            sources
                .get(&row.connection)
                .cloned()
                .or_else(|| single.clone())
                .unwrap_or_else(|| provider.name().into())
        };
        let index = match groups.iter().position(|group| group.source == source) {
            Some(index) => index,
            None => {
                groups.push(CalendarGroup {
                    source,
                    connection: "".into(),
                    calendars: vec![],
                });
                groups.len() - 1
            }
        };
        if groups[index].connection.is_empty() {
            groups[index].connection = row.connection.clone();
        }
        groups[index].calendars.push(row.clone());
    }
    if !provider.cloud() {
        groups.sort_by_key(|group| &*group.source == SUBSCRIBED);
    }
    groups
}

const CALENDAR_QUERY: &str = "SELECT id, name, enabled, provider, source, color, connection_id
    FROM calendars WHERE deleted_at IS NULL ORDER BY name, id LIMIT 500";

fn field(row: &Value, key: &str) -> Arc<str> {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .into()
}

fn rows(values: &[Value]) -> Vec<CalendarRow> {
    values
        .iter()
        .map(|row| CalendarRow {
            id: field(row, "id"),
            name: field(row, "name"),
            enabled: row["enabled"] == 1 || row["enabled"] == true,
            provider: field(row, "provider"),
            source: field(row, "source"),
            color: field(row, "color"),
            connection: field(row, "connection_id"),
        })
        .collect()
}

/// Mirrors the shipping toggle: disabling a calendar also tombstones its stored events.
pub fn set_enabled(runtime: &RuntimeHandle, id: Arc<str>, enabled: bool) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        transaction(
            &services.executor,
            vec![
                statement(
                    "UPDATE calendars SET enabled = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL"
                        .into(),
                    vec![json!(enabled), json!(now), json!(id)],
                    Some(1),
                ),
                statement(
                    "UPDATE events SET deleted_at = ?, updated_at = ? WHERE calendar_id = ? AND deleted_at IS NULL AND ? = 0"
                        .into(),
                    vec![json!(now), json!(now), json!(id), json!(enabled)],
                    None,
                ),
            ],
        )
        .await
    })
}

/// One row of an in-app context menu.
#[derive(Clone)]
pub struct MenuItem {
    label: SharedString,
    disabled: bool,
    separator: bool,
}

impl MenuItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            disabled: false,
            separator: false,
        }
    }

    pub fn after_separator(mut self) -> Self {
        self.separator = true;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Stands in for shipping's native context menus, styled like the timeline's note menu.
pub fn context_menu(
    id: &'static str,
    position: Point<Pixels>,
    items: impl IntoIterator<Item = MenuItem>,
    colors: Theme,
    on_select: impl Fn(&usize, &mut Window, &mut App) + 'static,
    on_dismiss: impl Fn(&(), &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let on_select = Rc::new(on_select);
    deferred(
        anchored().position(position).snap_to_window().child(
            div()
                .id(id)
                .w(px(224.))
                .p_1()
                .bg(colors.card)
                .border_1()
                .border_color(colors.border)
                .shadow_lg()
                .text_color(colors.popover_foreground)
                .occlude()
                .on_mouse_down_out(move |_, window, cx| on_dismiss(&(), window, cx))
                .children(items.into_iter().enumerate().map(|(index, item)| {
                    let on_select = on_select.clone();
                    div()
                        .when(item.separator, |row| {
                            row.child(div().my_1().h(px(1.)).bg(colors.border))
                        })
                        .child(
                            div()
                                .id(index)
                                .h(px(30.))
                                .px_3()
                                .flex()
                                .items_center()
                                .text_sm()
                                .map(|row| {
                                    if item.disabled {
                                        row.opacity(0.5)
                                    } else {
                                        row.cursor_pointer()
                                            .hover(|style| style.bg(colors.accent))
                                            .on_click(move |_, window, cx| {
                                                cx.stop_propagation();
                                                on_select(&index, window, cx)
                                            })
                                    }
                                })
                                .child(item.label),
                        )
                })),
        ),
    )
    .with_priority(1)
}

#[derive(Clone, Debug, PartialEq)]
enum Menu {
    Provider(Provider),
    /// A cloud account's Reconnect / Disconnect.
    Account(Provider, Arc<str>),
}

pub struct CalendarSidebar {
    runtime: RuntimeHandle,
    calendars: Arc<[CalendarRow]>,
    open: HashSet<Provider>,
    /// Which accounts were connected when `open` was last reset; like the shipping
    /// accordion, expansion returns to its defaults when that changes.
    open_key: Option<Vec<bool>>,
    pending: HashMap<Arc<str>, bool>,
    apple_access: AppleAccess,
    /// The Calendar permission request is in flight (shipping `calendar.isPending`).
    apple_pending: bool,
    /// The browser flow being opened (shipping `openingAction`).
    opening: Option<(Provider, Flow)>,
    menu: Option<(Point<Pixels>, Menu)>,
    permission_dialog: bool,
    signed_in: bool,
    sync: Entity<CalendarSync>,
    watch_cancel: CancellationToken,
    message: String,
    _sync: Subscription,
}

impl EventEmitter<CalendarSidebarEvent> for CalendarSidebar {}

impl CalendarSidebar {
    pub fn new(runtime: RuntimeHandle, sync: Entity<CalendarSync>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&sync, |this, _, cx| {
            this.reset_open(cx);
            cx.notify();
        });
        Self {
            runtime,
            calendars: Arc::from([]),
            open: HashSet::new(),
            open_key: None,
            pending: HashMap::new(),
            apple_access: AppleAccess::NotDetermined,
            apple_pending: false,
            opening: None,
            menu: None,
            permission_dialog: false,
            signed_in: false,
            sync,
            watch_cancel: CancellationToken::new(),
            message: String::new(),
            _sync: subscription,
        }
    }

    pub fn set_signed_in(&mut self, signed_in: bool, cx: &mut Context<Self>) {
        if self.signed_in != signed_in {
            self.signed_in = signed_in;
            self.refresh_accounts(cx);
            self.reset_open(cx);
            cx.notify();
        }
    }

    pub fn suspend(&mut self) {
        self.watch_cancel.cancel();
        self.menu = None;
    }

    pub fn activate(&mut self, cx: &mut Context<Self>) {
        self.refresh_apple(cx);
        self.refresh_accounts(cx);
        self.watch_cancel.cancel();
        self.watch_cancel = CancellationToken::new();
        let cancel = self.watch_cancel.clone();
        let reply = self.runtime.watch_query(CALENDAR_QUERY.into(), vec![]);
        cx.spawn(async move |this, cx| {
            let mut watch = match async { reply?.receive().await }.await {
                Ok(watch) => watch,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.message = format!("Could not load calendars: {error}");
                        cx.notify();
                    });
                    return;
                }
            };
            loop {
                if cancel.is_cancelled() {
                    break;
                }
                let snapshot = watch.snapshots.borrow_and_update().rows.clone();
                let error = watch
                    .terminal_error()
                    .or_else(|| watch.errors.try_recv().ok());
                if this
                    .update(cx, |this, cx| {
                        match error {
                            Some(error) => {
                                this.message = format!("Could not load calendars: {error}")
                            }
                            None => this.replace(rows(&snapshot), cx),
                        }
                        cx.notify();
                    })
                    .is_err()
                    || watch.terminal_error().is_some()
                {
                    break;
                }
                match select(
                    watch.snapshots.changed().boxed(),
                    cancel.cancelled().boxed(),
                )
                .await
                {
                    Either::Left((Ok(()), _)) => {}
                    _ => break,
                }
            }
            let _ = watch.unsubscribe().await;
        })
        .detach();
    }

    /// Like shipping's `useConnections(isPaid)`, only paid accounts list their connections.
    fn refresh_accounts(&mut self, cx: &mut Context<Self>) {
        if self.signed_in && crate::product::account::is_paid(cx) {
            self.sync.update(cx, |sync, cx| sync.refresh_accounts(cx));
        }
    }

    fn services(&self, cx: &App) -> Option<Arc<dyn CalendarAccounts>> {
        self.sync.read(cx).services()
    }

    fn refresh_apple(&mut self, cx: &App) {
        self.apple_access = match self.services(cx) {
            Some(services) => services.apple_access(),
            None if apple_authorized() => AppleAccess::Authorized,
            None => AppleAccess::NotDetermined,
        };
    }

    fn replace(&mut self, calendars: Vec<CalendarRow>, cx: &App) {
        self.message.clear();
        self.pending.retain(|id, enabled| {
            calendars
                .iter()
                .any(|row| &row.id == id && row.enabled != *enabled)
        });
        self.calendars = calendars.into();
        self.reset_open(cx);
    }

    /// The provider's accounts, once the account list has loaded.
    fn connections(&self, provider: Provider, cx: &App) -> Option<Vec<Connection>> {
        match &self.sync.read(cx).accounts {
            Accounts::Ready(connections) => Some(
                connections
                    .iter()
                    .filter(|connection| connection.provider == provider)
                    .cloned()
                    .collect(),
            ),
            _ => None,
        }
    }

    fn connected(&self, provider: Provider, cx: &App) -> bool {
        self.signed_in
            && self
                .connections(provider, cx)
                .is_some_and(|connections| !connections.is_empty())
    }

    fn reset_open(&mut self, cx: &App) {
        let key = Provider::visible()
            .iter()
            .map(|provider| self.connected(*provider, cx))
            .collect::<Vec<_>>();
        if self.open_key.as_ref() != Some(&key) {
            // Local providers and connected accounts start expanded.
            self.open = Provider::visible()
                .iter()
                .copied()
                .filter(|provider| !provider.cloud() || self.connected(*provider, cx))
                .collect();
            self.open_key = Some(key);
        }
    }

    /// Shipping's `canAddAccount`: a paid, signed-in account whose list has loaded.
    fn can_add_account(&self, provider: Provider, cx: &App) -> bool {
        provider.cloud()
            && self.signed_in
            && crate::product::account::is_paid(cx)
            && self.connections(provider, cx).is_some()
    }

    fn toggle_calendar(&mut self, row: &CalendarRow, cx: &mut Context<Self>) {
        let enabled = !self.pending.get(&row.id).copied().unwrap_or(row.enabled);
        self.pending.insert(row.id.clone(), enabled);
        cx.notify();
        let reply = set_enabled(&self.runtime, row.id.clone(), enabled);
        let id = row.id.clone();
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    // Like shipping, events follow five seconds after the last toggle.
                    Ok(()) => this.sync.update(cx, |sync, cx| sync.schedule_debounced(cx)),
                    Err(error) => {
                        if this.pending.get(&id) == Some(&enabled) {
                            this.pending.remove(&id);
                        }
                        this.message = format!("Could not update the calendar: {error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn refresh_calendars(&mut self, cx: &mut Context<Self>) {
        self.refresh_accounts(cx);
        self.sync.update(cx, |sync, cx| {
            sync.cancel_debounced(cx);
            sync.schedule(cx);
        });
    }

    /// Opens shipping's browser flow for a cloud account; connecting and reconnecting then
    /// watch for the account to appear.
    fn start_flow(
        &mut self,
        provider: Provider,
        flow: Flow,
        connection: Option<Arc<str>>,
        cx: &mut Context<Self>,
    ) {
        let Some(services) = self.services(cx) else {
            cx.emit(CalendarSidebarEvent::Connect);
            return;
        };
        if self.opening.is_some() {
            return;
        }
        self.opening = Some((provider, flow));
        if flow != Flow::Disconnect {
            self.sync
                .update(cx, |sync, cx| sync.watch_for_new_connection(cx));
        }
        cx.notify();
        let job = services.open(provider, flow, connection);
        cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.opening = None;
                if result.is_err() {
                    this.message = "Could not start the integration setup. Try again.".into();
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Shipping's `handleAppleConnect`: denied access explains how to turn it on, otherwise
    /// the system asks.
    fn connect_apple(&mut self, cx: &mut Context<Self>) {
        if self.apple_pending {
            return;
        }
        let Some(services) = self.services(cx) else {
            cx.emit(CalendarSidebarEvent::Connect);
            return;
        };
        self.refresh_apple(cx);
        if self.apple_access == AppleAccess::Denied {
            self.permission_dialog = true;
            cx.notify();
            return;
        }
        self.apple_pending = true;
        cx.notify();
        let job = services.request_apple();
        cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.apple_pending = false;
                this.refresh_apple(cx);
                if result.is_ok() {
                    this.sync.update(cx, |sync, cx| sync.schedule(cx));
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn disconnect_apple(&mut self, cx: &mut Context<Self>) {
        let Some(services) = self.services(cx) else {
            return;
        };
        let job = services.disconnect_apple();
        self.refresh_apple(cx);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                if let Err(error) = result {
                    this.message = format!("Could not disconnect Apple Calendar: {error}");
                }
                this.sync.update(cx, |sync, cx| sync.schedule(cx));
                cx.notify();
            });
        })
        .detach();
    }

    fn provider_menu(&self, provider: Provider, cx: &App) -> Vec<MenuItem> {
        if self.can_add_account(provider, cx) {
            vec![MenuItem::new(format!("Add {} account", provider.name()))]
        } else if provider == Provider::Apple && self.apple_access == AppleAccess::Authorized {
            vec![
                MenuItem::new("Reconnect").disabled(self.apple_pending),
                MenuItem::new("Disconnect").disabled(self.apple_pending),
            ]
        } else {
            vec![]
        }
    }

    fn choose(&mut self, menu: Menu, index: usize, cx: &mut Context<Self>) {
        self.menu = None;
        match menu {
            Menu::Provider(provider) if provider.cloud() => {
                self.start_flow(provider, Flow::Connect, None, cx)
            }
            Menu::Provider(_) if index == 0 => self.connect_apple(cx),
            Menu::Provider(_) => self.disconnect_apple(cx),
            Menu::Account(provider, connection) => self.start_flow(
                provider,
                if index == 0 {
                    Flow::Reconnect
                } else {
                    Flow::Disconnect
                },
                Some(connection),
                cx,
            ),
        }
        cx.notify();
    }

    fn click_provider(&mut self, provider: Provider, cx: &mut Context<Self>) {
        if provider == Provider::Apple && self.apple_access != AppleAccess::Authorized {
            self.connect_apple(cx);
            return;
        }
        if self.can_add_account(provider, cx)
            && self
                .connections(provider, cx)
                .is_some_and(|connections| connections.is_empty())
        {
            self.start_flow(provider, Flow::Connect, None, cx);
            return;
        }
        if !self.open.remove(&provider) {
            self.open.insert(provider);
        }
        cx.notify();
    }

    fn provider_icon(provider: Provider) -> gpui::Div {
        let image = match provider {
            Provider::Apple => img("workspace/AppleCalendar.png").size(px(20.)),
            Provider::Google => img("workspace/GoogleCalendar.svg").w(px(15.)).h(px(16.)),
            Provider::Outlook => img("workspace/OutlookIcon.svg").w(px(16.)).h(px(15.)),
        };
        div()
            .size(px(20.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .child(image)
    }

    fn round_button(id: SharedString, colors: Theme) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .p_1()
            .flex_shrink_0()
            .cursor_pointer()
            .hover(move |style| style.bg(colors.accent))
    }

    fn open_menu(&mut self, position: Point<Pixels>, menu: Menu, cx: &mut Context<Self>) {
        self.menu = Some((position, menu));
        cx.stop_propagation();
        cx.notify();
    }

    fn provider(&self, provider: Provider, colors: Theme, cx: &mut Context<Self>) -> gpui::Div {
        let needs_permission =
            provider == Provider::Apple && self.apple_access != AppleAccess::Authorized;
        let open = !needs_permission && self.open.contains(&provider);
        let can_add = self.can_add_account(provider, cx);
        let add_button = can_add && crate::product::account::is_pro(cx);
        let menu_button = provider == Provider::Apple && !needs_permission;
        let has_menu = !self.provider_menu(provider, cx).is_empty();
        let group = SharedString::from(format!("calendar-provider-row:{}", provider.id()));
        let plus = |busy: bool| {
            if busy {
                spinner(16., colors.muted_foreground).into_any_element()
            } else {
                svg()
                    .path("workspace/PlusIcon.svg")
                    .size(px(16.))
                    .text_color(colors.muted_foreground)
                    .into_any_element()
            }
        };
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .id(SharedString::from(format!(
                        "calendar-provider:{}",
                        provider.id()
                    )))
                    .group(group.clone())
                    .mx(px(-8.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .cursor_pointer()
                    .hover(move |style| style.bg(colors.accent))
                    .on_click(cx.listener(move |this, _, _, cx| this.click_provider(provider, cx)))
                    .when(has_menu, |row| {
                        row.on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                this.open_menu(event.position, Menu::Provider(provider), cx)
                            }),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .py_3()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(Self::provider_icon(provider))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .line_height(px(20.))
                                    .font_weight(crate::ui::theme::MEDIUM)
                                    .text_color(colors.foreground)
                                    .child(provider.name()),
                            ),
                    )
                    .map(|row| {
                        if needs_permission {
                            row.child(
                                Self::round_button(
                                    SharedString::from(format!(
                                        "calendar-connect:{}",
                                        provider.id()
                                    )),
                                    colors,
                                )
                                .when(self.apple_pending, |button| button.opacity(0.5))
                                .child(plus(self.apple_pending))
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.connect_apple(cx);
                                    },
                                )),
                            )
                        } else if add_button {
                            let busy = self.opening == Some((provider, Flow::Connect));
                            row.child(
                                Self::round_button(
                                    SharedString::from(format!(
                                        "calendar-connect:{}",
                                        provider.id()
                                    )),
                                    colors,
                                )
                                .when(self.opening.is_some(), |button| button.opacity(0.5))
                                .child(plus(busy))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.start_flow(provider, Flow::Connect, None, cx);
                                    },
                                )),
                            )
                        } else if menu_button {
                            row.child(
                                Self::round_button(
                                    SharedString::from(format!("calendar-more:{}", provider.id())),
                                    colors,
                                )
                                .invisible()
                                .group_hover(group.clone(), |style| style.visible())
                                .child(
                                    svg()
                                        .path("workspace/MoreHorizontalIcon.svg")
                                        .size(px(16.))
                                        .text_color(colors.muted_foreground),
                                )
                                .on_click(cx.listener(
                                    move |this, event: &gpui::ClickEvent, _, cx| {
                                        this.open_menu(
                                            event.position(),
                                            Menu::Provider(provider),
                                            cx,
                                        )
                                    },
                                )),
                            )
                        } else {
                            row
                        }
                    })
                    .when(!needs_permission, |row| {
                        row.child(
                            svg()
                                .path("workspace/ArrowRight01Icon.svg")
                                .size(px(16.))
                                .flex_shrink_0()
                                .text_color(colors.muted_foreground)
                                .when(open, |caret| {
                                    caret.with_transformation(Transformation::rotate(radians(
                                        std::f32::consts::FRAC_PI_2,
                                    )))
                                }),
                        )
                    }),
            )
            .when(open, |view| {
                view.child(self.provider_content(provider, colors, cx))
            })
    }

    /// Shipping's underlined `text-xs` connect links.
    fn link(
        id: SharedString,
        label: impl Into<SharedString>,
        color: Hsla,
        hover: Hsla,
        busy: bool,
        disabled: bool,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .flex()
            .items_center()
            .gap_1()
            .text_xs()
            // The link sits in the accordion's text-sm line box.
            .line_height(px(20.))
            .underline()
            .text_color(color)
            .cursor_pointer()
            .hover(move |style| style.text_color(hover))
            .when(disabled, |link| link.opacity(0.5))
            .when(busy, |link| link.child(spinner(12., color)))
            .child(label.into())
    }

    fn provider_content(
        &self,
        provider: Provider,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let content = div().pb_3().text_sm();
        if !provider.cloud() {
            let groups = groups(provider, &self.calendars);
            return content.child(self.selection(provider, groups, &[], colors, cx));
        }
        let label = format!("Connect {} Calendar", provider.name());
        if !self.signed_in {
            return content.child(
                div().pt_1().pb_2().flex().child(
                    div()
                        .text_xs()
                        .line_height(px(20.))
                        .text_color(colors.muted_foreground)
                        .opacity(0.5)
                        .child(label),
                ),
            );
        }
        // Shipping gates connecting a cloud calendar on Pro (`PlanGate`).
        if !crate::product::account::is_pro(cx) {
            return content.child(
                div().pt_1().pb_2().flex().child(
                    div()
                        .relative()
                        .opacity(0.6)
                        .text_xs()
                        .line_height(px(20.))
                        .underline()
                        .text_color(colors.muted_foreground)
                        .child(label)
                        .child(div().absolute().inset_0().occlude()),
                ),
            );
        }
        let connections = self.connections(provider, cx);
        if let Some(connections) = connections.clone().filter(|list| !list.is_empty()) {
            let groups = groups(provider, &self.calendars);
            return content.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .pb_2()
                    .children(
                        connections
                            .iter()
                            .filter(|connection| connection.reconnect_required)
                            .map(|connection| self.reconnect_required(connection, colors, cx)),
                    )
                    .child(self.selection(provider, groups, &connections, colors, cx)),
            );
        }
        if self.sync.read(cx).accounts == Accounts::Failed {
            return content.child(
                div()
                    .pt_1()
                    .pb_2()
                    .text_xs()
                    .line_height(px(20.))
                    .text_color(rgb(RED_600))
                    .child("Failed to load integration status"),
            );
        }
        let busy = self.opening == Some((provider, Flow::Connect));
        content.child(
            div().pt_1().pb_2().flex().child(
                Self::link(
                    SharedString::from(format!("calendar-connect-link:{}", provider.id())),
                    label,
                    colors.muted_foreground,
                    colors.foreground,
                    busy,
                    self.opening.is_some(),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.start_flow(provider, Flow::Connect, None, cx)
                })),
            ),
        )
    }

    fn reconnect_required(
        &self,
        connection: &Connection,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let provider = connection.provider;
        let (reconnect, disconnect) = (connection.id.clone(), connection.id.clone());
        let opening = self.opening.is_some();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .pb_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .line_height(px(16.))
                    .text_color(rgb(AMBER_700))
                    .child(div().size(px(10.)).bg(rgb(AMBER_500)))
                    .child(format!(
                        "Reconnect required for {} Calendar",
                        provider.name()
                    )),
            )
            .when_some(connection.error.clone(), |view, error| {
                view.child(
                    div()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(colors.muted_foreground)
                        .child(SharedString::from(error)),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Self::link(
                            SharedString::from(format!("calendar-reconnect:{}", connection.id)),
                            "Reconnect",
                            colors.muted_foreground,
                            colors.foreground,
                            self.opening == Some((provider, Flow::Reconnect)),
                            opening,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.start_flow(provider, Flow::Reconnect, Some(reconnect.clone()), cx)
                        })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(16.))
                            .text_color(colors.muted_foreground)
                            .child("or"),
                    )
                    .child(
                        Self::link(
                            SharedString::from(format!("calendar-disconnect:{}", connection.id)),
                            "Disconnect",
                            rgb(RED_500).into(),
                            rgb(RED_700).into(),
                            self.opening == Some((provider, Flow::Disconnect)),
                            opening,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.start_flow(
                                provider,
                                Flow::Disconnect,
                                Some(disconnect.clone()),
                                cx,
                            )
                        })),
                    ),
            )
    }

    /// Shipping's `CalendarSelection`: calendars grouped by account, each account header with
    /// its hover "…" menu, or the empty state with a refresh button.
    fn selection(
        &self,
        provider: Provider,
        groups: Vec<CalendarGroup>,
        connections: &[Connection],
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        if groups.is_empty() {
            // Only cloud accounts show loading, as in shipping.
            let loading = provider.cloud() && self.sync.read(cx).status() == SyncStatus::Syncing;
            return div()
                .px_4()
                .py_6()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(if loading {
                    div()
                        .mb_2()
                        .child(spinner(24., colors.muted_foreground.opacity(0.7)))
                        .into_any_element()
                } else {
                    svg()
                        .path("workspace/CalendarOffIcon.svg")
                        .size(px(24.))
                        .mb_2()
                        .text_color(colors.muted_foreground.opacity(0.7))
                        .into_any_element()
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(colors.muted_foreground)
                        .child(if loading {
                            "Loading calendars..."
                        } else {
                            "No calendars found"
                        })
                        .when(!loading, |row| {
                            row.child(
                                div()
                                    .id(SharedString::from(format!(
                                        "calendar-refresh:{}",
                                        provider.id()
                                    )))
                                    .p_1()
                                    .cursor_pointer()
                                    .hover(move |style| style.bg(colors.accent))
                                    .child(
                                        svg()
                                            .path("workspace/RefreshIcon.svg")
                                            .size(px(12.))
                                            .text_color(colors.muted_foreground),
                                    )
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.refresh_calendars(cx)),
                                    ),
                            )
                        }),
                );
        }
        // The account behind each group, matched like shipping's `ConnectedContent`.
        let sources = self
            .calendars
            .iter()
            .filter(|row| !row.source.is_empty() && !row.connection.is_empty())
            .map(|row| (row.connection.clone(), row.source.clone()))
            .collect::<HashMap<_, _>>();
        let multiple = groups.len() > 1;
        div()
            .flex()
            .flex_col()
            .gap_3()
            .children(groups.into_iter().enumerate().map(|(index, group)| {
                let account = connections
                    .iter()
                    .find(|connection| {
                        connection.id == group.connection
                            || sources.get(&connection.id) == Some(&group.source)
                    })
                    .map(|connection| connection.id.clone());
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .when(multiple || account.is_some(), |view| {
                        view.child(self.group_header(provider, index, &group, account, colors, cx))
                    })
                    .children(
                        group
                            .calendars
                            .iter()
                            .map(|row| self.calendar_row(row, colors, cx)),
                    )
            }))
    }

    fn group_header(
        &self,
        provider: Provider,
        index: usize,
        group: &CalendarGroup,
        account: Option<Arc<str>>,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let name = SharedString::from(format!("calendar-group:{}:{index}", provider.id()));
        div()
            .id(name.clone())
            .group(name.clone())
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .py_1()
            .child(
                div()
                    .truncate()
                    .text_xs()
                    .line_height(px(16.))
                    .font_weight(crate::ui::theme::MEDIUM)
                    .text_color(colors.muted_foreground)
                    .child(SharedString::from(group.source.clone())),
            )
            .when_some(account, |header, account| {
                let right_click = account.clone();
                header
                    .mx(px(-8.))
                    .px_2()
                    .hover(move |style| style.bg(colors.accent))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.open_menu(
                                event.position,
                                Menu::Account(provider, right_click.clone()),
                                cx,
                            )
                        }),
                    )
                    .child(
                        Self::round_button(name.clone(), colors)
                            .invisible()
                            .group_hover(name, |style| style.visible())
                            .child(
                                svg()
                                    .path("workspace/MoreHorizontalIcon.svg")
                                    .size(px(16.))
                                    .text_color(colors.muted_foreground),
                            )
                            .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                                this.open_menu(
                                    event.position(),
                                    Menu::Account(provider, account.clone()),
                                    cx,
                                )
                            })),
                    )
            })
    }

    fn calendar_row(
        &self,
        row: &CalendarRow,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let enabled = self.pending.get(&row.id).copied().unwrap_or(row.enabled);
        let color = parse_color(&row.color).unwrap_or_else(|| rgb(0x888888).into());
        let target = row.clone();
        div()
            .id(SharedString::from(format!("calendar-toggle:{}", row.id)))
            .w_full()
            .py_1()
            .pr_2()
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_calendar(&target, cx)))
            .child(
                div()
                    .size(px(16.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_1()
                    .border_color(color)
                    .when(enabled, |check| {
                        check.bg(color).child(
                            svg()
                                .path("workspace/CheckIcon.svg")
                                .size(px(12.))
                                .text_color(colors.primary_foreground),
                        )
                    }),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .line_height(px(20.))
                    .text_color(colors.foreground)
                    .child(if row.name.is_empty() {
                        SharedString::from("Untitled")
                    } else {
                        SharedString::from(row.name.clone())
                    }),
            )
    }

    /// Shipping's `AppleCalendarPermissionDialog`.
    fn permission_dialog(&self, window: &Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let colors = theme(window);
        let text = |weight: FontWeight, content: &'static str| {
            div()
                .w_full()
                .text_center()
                .text_size(px(13.))
                .font_weight(weight)
                .text_color(colors.foreground)
                .child(content)
        };
        modal(
            window,
            glass_dialog(colors)
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.permission_dialog = false;
                    cx.notify();
                }))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_2()
                        .child(text(FontWeight::SEMIBOLD, "Apple Calendar access is off").line_height(px(20.)))
                        .child(
                            text(
                                FontWeight::NORMAL,
                                "Turn on Anarlog in System Settings → Privacy & Security → Calendars, then return here.",
                            )
                            .line_height(px(13. * 1.36)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            dialog_button("dialog-cancel", "Cancel", false, colors)
                                .flex_1()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.permission_dialog = false;
                                    cx.notify();
                                })),
                        )
                        .child(
                            dialog_button("dialog-open-settings", "Open Settings", false, colors)
                                .flex_1()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.permission_dialog = false;
                                    if let Some(services) = this.services(cx)
                                        && let Err(error) = services.open_apple_settings()
                                    {
                                        this.message = error.to_string();
                                    }
                                    cx.notify();
                                })),
                        ),
                )
                .into_any_element(),
        )
    }
}

fn apple_authorized() -> bool {
    #[cfg(target_os = "macos")]
    {
        anlg_apple_calendar::Handle::authorization_status()
            == anlg_apple_calendar::CalendarAuthStatus::Authorized
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Shipping's `CircleNotch` spinner.
fn spinner(size: f32, color: Hsla) -> impl IntoElement {
    svg()
        .path("chat/LoaderCircleIcon.svg")
        .size(px(size))
        .flex_shrink_0()
        .text_color(color)
        .with_animation(
            "calendar-sidebar-spinner",
            Animation::new(Duration::from_secs(1)).repeat(),
            |icon, delta| icon.with_transformation(Transformation::rotate(percentage(delta))),
        )
}

const RED_500: u32 = 0xfb2c36;
const RED_600: u32 = 0xe7000b;
const RED_700: u32 = 0xc10007;
const AMBER_500: u32 = 0xfe9a00;
const AMBER_700: u32 = 0xbb4d00;

impl Drop for CalendarSidebar {
    fn drop(&mut self) {
        self.watch_cancel.cancel();
    }
}

impl Render for CalendarSidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let menu = self.menu.clone().map(|(position, menu)| {
            let items = match &menu {
                Menu::Provider(provider) => self.provider_menu(*provider, cx),
                Menu::Account(..) => vec![MenuItem::new("Reconnect"), MenuItem::new("Disconnect")],
            };
            context_menu(
                "calendar-sidebar-menu",
                position,
                items,
                colors,
                cx.listener(move |this, index: &usize, _, cx| {
                    this.choose(menu.clone(), *index, cx)
                }),
                cx.listener(|this, _: &(), _, cx| {
                    this.menu = None;
                    cx.notify();
                }),
            )
        });
        div()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .pb_2()
            .child(
                div()
                    .h(px(48.))
                    .flex_shrink_0()
                    .flex()
                    .items_start()
                    .pr_1()
                    .pl(crate::zoom::window_controls_gutter(cx))
                    .pt(crate::zoom::window_controls_top(cx))
                    .child(
                        div()
                            .id("calendar-back")
                            .group("calendar-back")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover(move |style| style.bg(colors.accent))
                            .on_click(
                                cx.listener(|_, _, _, cx| cx.emit(CalendarSidebarEvent::Back)),
                            )
                            .child(
                                svg()
                                    .path("workspace/ArrowLeft02Icon.svg")
                                    .size(px(16.))
                                    .text_color(colors.muted_foreground)
                                    .group_hover("calendar-back", move |style| {
                                        style.text_color(colors.foreground)
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .id("calendar-providers")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_3()
                    .children(
                        Provider::visible()
                            .iter()
                            .map(|provider| self.provider(*provider, colors, cx)),
                    )
                    .when(!self.message.is_empty(), |view| {
                        view.child(
                            div()
                                .pt_2()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child(self.message.clone()),
                        )
                    }),
            )
            .children(menu)
            .when(self.permission_dialog, |view| {
                view.child(self.permission_dialog(window, cx))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, provider: &str, source: &str, connection: &str) -> CalendarRow {
        CalendarRow {
            id: id.into(),
            name: id.into(),
            enabled: true,
            provider: provider.into(),
            source: source.into(),
            color: "#888".into(),
            connection: connection.into(),
        }
    }

    #[test]
    fn apple_groups_by_source_with_subscriptions_last() {
        let rows = [
            row("holidays", "apple", SUBSCRIBED, ""),
            row("work", "apple", "iCloud", ""),
            row("local", "apple", "", ""),
            row("other", "google", "me@example.com", "c1"),
        ];
        let sources = groups(Provider::Apple, &rows)
            .into_iter()
            .map(|group| group.source)
            .collect::<Vec<_>>();
        assert_eq!(
            sources.iter().map(|source| &**source).collect::<Vec<_>>(),
            ["iCloud", "Apple Calendar", SUBSCRIBED]
        );
    }

    #[test]
    fn cloud_calendars_without_a_source_join_their_account() {
        let rows = [
            row("primary", "google", "me@example.com", "c1"),
            row("team", "google", "", "c1"),
            row("orphan", "google", "", ""),
        ];
        let groups = groups(Provider::Google, &rows);
        assert_eq!(groups.len(), 1);
        assert_eq!(&*groups[0].source, "me@example.com");
        assert_eq!(groups[0].calendars.len(), 3);
        assert_eq!(&*groups[0].connection, "c1");
    }
}
