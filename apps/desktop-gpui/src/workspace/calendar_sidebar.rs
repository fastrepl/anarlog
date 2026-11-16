use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use desktop_runtime::{CancellationToken, Reply, Result, RuntimeHandle};
use futures::{
    FutureExt,
    future::{Either, select},
};
use gpui::{
    Context, EventEmitter, FontWeight, Render, SharedString, Transformation, Window, div, img,
    prelude::*, px, radians, rgb, svg,
};
use serde_json::{Value, json};

use super::{
    calendar::parse_color,
    mutations::{statement, transaction},
};
use crate::ui::theme::{Theme, theme};

pub enum CalendarSidebarEvent {
    Back,
    /// Connecting an account or granting calendar access happens in the product surface.
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
    pub connected: bool,
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
                    connected: false,
                    calendars: vec![],
                });
                groups.len() - 1
            }
        };
        groups[index].connected |= !row.connection.is_empty();
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

pub struct CalendarSidebar {
    runtime: RuntimeHandle,
    calendars: Arc<[CalendarRow]>,
    loaded: bool,
    open: HashSet<Provider>,
    /// Which accounts were connected when `open` was last reset; like the shipping
    /// accordion, expansion returns to its defaults when that changes.
    open_key: Option<Vec<bool>>,
    pending: HashMap<Arc<str>, bool>,
    apple_authorized: bool,
    signed_in: bool,
    watch_cancel: CancellationToken,
    message: String,
}

impl EventEmitter<CalendarSidebarEvent> for CalendarSidebar {}

impl CalendarSidebar {
    pub fn new(runtime: RuntimeHandle) -> Self {
        Self {
            runtime,
            calendars: Arc::from([]),
            loaded: false,
            open: HashSet::new(),
            open_key: None,
            pending: HashMap::new(),
            apple_authorized: false,
            signed_in: false,
            watch_cancel: CancellationToken::new(),
            message: String::new(),
        }
    }

    pub fn set_signed_in(&mut self, signed_in: bool, cx: &mut Context<Self>) {
        if self.signed_in != signed_in {
            self.signed_in = signed_in;
            self.reset_open();
            cx.notify();
        }
    }

    pub fn suspend(&mut self) {
        self.watch_cancel.cancel();
    }

    pub fn activate(&mut self, cx: &mut Context<Self>) {
        self.apple_authorized = apple_authorized();
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
                            None => this.replace(rows(&snapshot)),
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

    fn replace(&mut self, calendars: Vec<CalendarRow>) {
        self.message.clear();
        self.pending.retain(|id, enabled| {
            calendars
                .iter()
                .any(|row| &row.id == id && row.enabled != *enabled)
        });
        self.calendars = calendars.into();
        self.loaded = true;
        self.reset_open();
    }

    /// A cloud provider counts as connected once its account has stored calendars.
    fn connected(&self, provider: Provider) -> bool {
        self.signed_in
            && self
                .calendars
                .iter()
                .any(|row| &*row.provider == provider.id())
    }

    fn reset_open(&mut self) {
        let key = Provider::visible()
            .iter()
            .map(|provider| self.connected(*provider))
            .collect::<Vec<_>>();
        if self.open_key.as_ref() != Some(&key) {
            // Local providers and connected accounts start expanded.
            self.open = Provider::visible()
                .iter()
                .copied()
                .filter(|provider| !provider.cloud() || self.connected(*provider))
                .collect();
            self.open_key = Some(key);
        }
    }

    fn toggle_calendar(&mut self, row: &CalendarRow, cx: &mut Context<Self>) {
        let enabled = !self.pending.get(&row.id).copied().unwrap_or(row.enabled);
        self.pending.insert(row.id.clone(), enabled);
        cx.notify();
        let reply = set_enabled(&self.runtime, row.id.clone(), enabled);
        let id = row.id.clone();
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    if this.pending.get(&id) == Some(&enabled) {
                        this.pending.remove(&id);
                    }
                    this.message = format!("Could not update the calendar: {error}");
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn toggle_provider(&mut self, provider: Provider, cx: &mut Context<Self>) {
        if provider == Provider::Apple && !self.apple_authorized
            || provider.cloud() && self.signed_in && !self.connected(provider)
        {
            cx.emit(CalendarSidebarEvent::Connect);
            return;
        }
        if !self.open.remove(&provider) {
            self.open.insert(provider);
        }
        cx.notify();
    }

    fn provider_icon(provider: Provider) -> gpui::Div {
        let image = match provider {
            Provider::Apple => img("workspace/AppleCalendar.png")
                .size(px(20.))
                .rounded(px(4.)),
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
            .rounded(px(8.))
            .cursor_pointer()
            .hover(move |style| style.bg(colors.accent))
    }

    fn provider(&self, provider: Provider, colors: Theme, cx: &mut Context<Self>) -> gpui::Div {
        let needs_permission = provider == Provider::Apple && !self.apple_authorized;
        let open = !needs_permission && self.open.contains(&provider);
        let add = needs_permission || provider.cloud() && self.signed_in;
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .id(SharedString::from(format!(
                        "calendar-provider:{}",
                        provider.id()
                    )))
                    .mx(px(-8.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .hover(move |style| style.bg(colors.accent))
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_provider(provider, cx)))
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
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(colors.foreground)
                                    .child(provider.name()),
                            ),
                    )
                    .when(add, |row| {
                        row.child(
                            Self::round_button(
                                SharedString::from(format!("calendar-connect:{}", provider.id())),
                                colors,
                            )
                            .child(
                                svg()
                                    .path("workspace/PlusIcon.svg")
                                    .size(px(16.))
                                    .text_color(colors.muted_foreground),
                            )
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.stop_propagation();
                                cx.emit(CalendarSidebarEvent::Connect);
                            })),
                        )
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

    fn provider_content(
        &self,
        provider: Provider,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let groups = groups(provider, &self.calendars);
        let content = div().pb_3().text_sm();
        if provider.cloud() && !self.connected(provider) {
            let label = format!("Connect {} Calendar", provider.name());
            return content.child(div().pt_1().pb_2().flex().child(if self.signed_in {
                div()
                    .id(SharedString::from(format!(
                        "calendar-connect-link:{}",
                        provider.id()
                    )))
                    .text_xs()
                    // The link sits in the accordion's text-sm line box.
                    .line_height(px(20.))
                    .underline()
                    .text_color(colors.muted_foreground)
                    .cursor_pointer()
                    .hover(move |style| style.text_color(colors.foreground))
                    .child(label)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(CalendarSidebarEvent::Connect)))
                    .into_any_element()
            } else {
                div()
                    .text_xs()
                    .line_height(px(20.))
                    .text_color(colors.muted_foreground)
                    .opacity(0.5)
                    .child(label)
                    .into_any_element()
            }));
        }
        if groups.is_empty() {
            return content.child(
                div()
                    .px_4()
                    .py_6()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .path("workspace/CalendarOffIcon.svg")
                            .size(px(24.))
                            .mb_2()
                            .text_color(colors.muted_foreground.opacity(0.7)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(16.))
                            .text_color(colors.muted_foreground)
                            .child(if self.loaded {
                                "No calendars found"
                            } else {
                                "Loading calendars..."
                            }),
                    ),
            );
        }
        let headers = groups.len() > 1 || groups.iter().any(|group| group.connected);
        content.child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .children(groups.into_iter().map(|group| {
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .when(headers, |view| {
                            view.child(
                                div()
                                    .py_1()
                                    .truncate()
                                    .text_xs()
                                    .line_height(px(16.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(colors.muted_foreground)
                                    .child(SharedString::from(group.source.clone())),
                            )
                        })
                        .children(
                            group
                                .calendars
                                .iter()
                                .map(|row| self.calendar_row(row, colors, cx)),
                        )
                })),
        )
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
                    .rounded(px(4.))
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
}

impl Drop for CalendarSidebar {
    fn drop(&mut self) {
        self.watch_cancel.cancel();
    }
}

impl Render for CalendarSidebar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
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
                    // The macOS traffic lights overlay the sidebar; keep the shipping app's gutter.
                    .pl(px(if cfg!(target_os = "macos") { 76. } else { 8. }))
                    .pt(px(9.))
                    .child(
                        div()
                            .id("calendar-back")
                            .group("calendar-back")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(8.))
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
        assert!(groups[0].connected);
    }
}
