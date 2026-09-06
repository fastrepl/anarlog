//! The Calendar tab: `calendar/components/{calendar-view,day-cell,event-chip,
//! session-chip,sidebar}.tsx` and `calendar/hooks.ts`' `useCalendarData`, in
//! the month view over the timeline's session and event rows.

use std::collections::{BTreeMap, HashSet};

use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use gpui::{
    AnyElement, ClickEvent, Context, Div, MouseButton, MouseDownEvent, SharedString, Window, div,
    prelude::*, px,
};

use super::Workspace;
use crate::theme::alpha;
use crate::timeline::{EventRow, SessionRow};
use crate::ui::{TailwindText as _, icon};

/// `text-xs leading-tight` chips: 15px rows with the `gap-0.5`.
const CHIP_HEIGHT: f32 = 15.0;
const CHIP_GAP: f32 = 2.0;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Popover {
    Event(String),
    Session(String),
    /// The `+N more` panel for a day.
    More(NaiveDate),
}

pub(crate) struct CalendarState {
    /// The first day of the month on screen (`currentMonth`).
    month: NaiveDate,
    /// `visibleStart`: the day the compact strip scrolls to when it (re)opens
    /// or the arrows move it; the strip spans 42 days either side of it.
    compact_start: NaiveDate,
    compact_scroll: gpui::ScrollHandle,
    /// Set by the arrows, Today, and a column-count change; the next compact
    /// render scrolls the strip to `compact_start` (the `scrollTo` effect).
    compact_scroll_reset: std::cell::Cell<bool>,
    /// The layout the last compact render used, for the arrows and the
    /// month-to-compact transition.
    compact_cols: std::cell::Cell<usize>,
    compact_day_width: std::cell::Cell<f32>,
    popover: Option<Popover>,
    /// `EventChip`'s `useNativeContextMenu`: the event id and the pointer.
    context_menu: Option<(String, gpui::Point<gpui::Pixels>)>,
    /// Expanded provider accordions (`google`, `outlook`).
    expanded: HashSet<&'static str>,
}

/// `useCalendarData`: ids per `yyyy-MM-dd` key in the app's sort order.
#[derive(Default)]
struct CalendarData {
    events_by_date: BTreeMap<NaiveDate, Vec<usize>>,
    sessions_by_date: BTreeMap<NaiveDate, Vec<usize>>,
}

/// `eventCalendarDay`: all-day events use the stored date, timed ones the
/// local day of the instant.
fn event_day(event: &EventRow) -> Option<NaiveDate> {
    if event.is_all_day != 0 {
        return NaiveDate::parse_from_str(event.started_at.trim().get(..10)?, "%Y-%m-%d").ok();
    }
    crate::timeline::parse_date(&event.started_at, &Local)
        .map(|instant| instant.with_timezone(&Local).date_naive())
}

pub(super) fn ignored_ids(
    settings: &crate::db::ProviderSettings,
    key: &str,
    field: &str,
) -> HashSet<String> {
    settings
        .value(key, &[key])
        .and_then(|value| match value {
            serde_json::Value::String(text) => serde_json::from_str(&text).ok(),
            other => Some(other),
        })
        .and_then(|value: serde_json::Value| value.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|entry| {
            entry
                .get(field)
                .and_then(|id| id.as_str())
                .map(str::to_string)
        })
        .collect()
}

/// `mutateSettingList`'s stored shape: the raw JSON array of entries.
fn ignored_list(settings: &crate::db::ProviderSettings, key: &str) -> Vec<serde_json::Value> {
    settings
        .value(key, &[key])
        .and_then(|value| match value {
            serde_json::Value::String(text) => serde_json::from_str(&text).ok(),
            other => Some(other),
        })
        .and_then(|value: serde_json::Value| value.as_array().cloned())
        .unwrap_or_default()
}

/// `ignoreEvent` / `ignoreSeries`: drop any entry with the same id, then
/// append `{ <field>: id, last_seen: now }`.
pub(crate) fn ignore_entry(
    list: Vec<serde_json::Value>,
    field: &str,
    id: &str,
    now: &str,
) -> Vec<serde_json::Value> {
    let mut next: Vec<serde_json::Value> = list
        .into_iter()
        .filter(|entry| entry.get(field).and_then(|v| v.as_str()) != Some(id))
        .collect();
    let mut entry = serde_json::Map::new();
    entry.insert(field.to_string(), serde_json::Value::String(id.to_string()));
    entry.insert(
        "last_seen".to_string(),
        serde_json::Value::String(now.to_string()),
    );
    next.push(serde_json::Value::Object(entry));
    next
}

fn build_calendar_data(
    sessions: &[SessionRow],
    events: &[EventRow],
    settings: &crate::db::ProviderSettings,
) -> CalendarData {
    let ignored_events = ignored_ids(settings, "ignored_events", "tracking_id");
    let ignored_series = ignored_ids(settings, "ignored_recurring_series", "id");
    let mut data = CalendarData::default();
    for (index, event) in events.iter().enumerate() {
        if event.title.is_empty() {
            continue;
        }
        if ignored_events.contains(&event.tracking_id_event)
            || (!event.recurrence_series_id.is_empty()
                && ignored_series.contains(&event.recurrence_series_id))
        {
            continue;
        }
        let Some(day) = event_day(event) else {
            continue;
        };
        data.events_by_date.entry(day).or_default().push(index);
    }
    for ids in data.events_by_date.values_mut() {
        ids.sort_by(|a, b| {
            let (a, b) = (&events[*a], &events[*b]);
            (a.is_all_day == 0)
                .cmp(&(b.is_all_day == 0))
                .then_with(|| {
                    crate::timeline::parse_date(&a.started_at, &Local)
                        .cmp(&crate::timeline::parse_date(&b.started_at, &Local))
                })
                .then_with(|| a.title.cmp(&b.title))
                .then_with(|| a.id.cmp(&b.id))
        });
    }
    for (index, session) in sessions.iter().enumerate() {
        if !session.event_json.is_empty() || session.title.is_empty() {
            continue;
        }
        let Some(created) = crate::timeline::parse_date(&session.created_at, &Local) else {
            continue;
        };
        data.sessions_by_date
            .entry(created.with_timezone(&Local).date_naive())
            .or_default()
            .push(index);
    }
    for ids in data.sessions_by_date.values_mut() {
        ids.sort_by(|a, b| {
            let (a, b) = (&sessions[*a], &sessions[*b]);
            crate::timeline::parse_date(&a.created_at, &Local)
                .cmp(&crate::timeline::parse_date(&b.created_at, &Local))
                .then_with(|| a.title.cmp(&b.title))
                .then_with(|| a.id.cmp(&b.id))
        });
    }
    data
}

/// `useVisibleItemCount`: how many chips fit, keeping room for the overflow line.
fn visible_item_count(available: f32, total: usize) -> usize {
    if total == 0 {
        return 0;
    }
    let all = total as f32 * CHIP_HEIGHT + (total.saturating_sub(1)) as f32 * CHIP_GAP;
    if all <= available {
        return total;
    }
    let mut count = 0;
    let mut used = 0.0;
    while count < total {
        let next = CHIP_HEIGHT + if count > 0 { CHIP_GAP } else { 0.0 };
        let remaining = total - count - 1;
        let more_space = if remaining > 0 {
            CHIP_HEIGHT + CHIP_GAP
        } else {
            0.0
        };
        if used + next + more_space > available {
            break;
        }
        used += next;
        count += 1;
    }
    count.max(1)
}

/// The width of a `font-mono text-xs` label, for sizing the truncating title.
/// `VIEW_BREAKPOINTS`: the month grid needs 700px; narrower surfaces show a
/// horizontally scrolling strip of 4, 2, or 1 day columns.
fn visible_cols(width: f32) -> usize {
    if width >= 700.0 {
        7
    } else if width >= 400.0 {
        4
    } else if width >= 200.0 {
        2
    } else {
        1
    }
}

const COMPACT_SCROLL_PAST_DAYS: i64 = 42;
const COMPACT_SCROLL_FUTURE_DAYS: i64 = 42;

/// `handleCompactScroll`: the day column at the strip's left edge.
fn compact_start_index(scroll_x: f32, day_width: f32, total_days: usize, cols: usize) -> usize {
    if day_width <= 0.0 {
        return 0;
    }
    let max_start = total_days.saturating_sub(cols);
    ((scroll_x / day_width).round().max(0.0) as usize).min(max_start)
}

/// `compactVisibleStart`: the day at the strip's left edge, from the scroll
/// offset of the last compact render (or `compact_start` before one).
fn compact_visible_start(state: &CalendarState) -> NaiveDate {
    let first = state.compact_start - Duration::days(COMPACT_SCROLL_PAST_DAYS);
    let total = (COMPACT_SCROLL_PAST_DAYS + COMPACT_SCROLL_FUTURE_DAYS) as usize;
    if state.compact_scroll_reset.get() || state.compact_day_width.get() <= 0.0 {
        return state.compact_start;
    }
    let index = compact_start_index(
        -f32::from(state.compact_scroll.offset().x),
        state.compact_day_width.get(),
        total,
        state.compact_cols.get(),
    );
    first + Duration::days(index as i64)
}

fn mono_text_width(text: &str, family: Option<&SharedString>, window: &Window) -> f32 {
    let mut style = window.text_style();
    if let Some(family) = family {
        style.font_family = family.clone();
    }
    let run = style.to_run(text.len());
    window
        .text_system()
        .shape_line(SharedString::from(text.to_string()), px(12.0), &[run], None)
        .width
        .into()
}

fn format_time(instant: &str) -> Option<String> {
    crate::timeline::parse_date(instant, &Local)
        .map(|utc| utc.with_timezone(&Local).format("%-I:%M %p").to_string())
}

impl Workspace {
    pub(crate) fn open_calendar(&mut self, cx: &mut Context<Self>) {
        self.close_settings(cx);
        self.close_folders(cx);
        self.close_templates(cx);
        self.close_contacts(cx);
        if self.calendar.is_none() {
            let today = Local::now().date_naive();
            self.calendar = Some(CalendarState {
                month: today.with_day(1).unwrap_or(today),
                compact_start: today,
                compact_scroll: gpui::ScrollHandle::new(),
                compact_scroll_reset: std::cell::Cell::new(true),
                compact_cols: std::cell::Cell::new(7),
                compact_day_width: std::cell::Cell::new(0.0),
                popover: None,
                context_menu: None,
                expanded: HashSet::new(),
            });
        }
        cx.notify();
    }

    pub(crate) fn close_calendar(&mut self, cx: &mut Context<Self>) {
        if self.calendar.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn calendar_open(&self) -> bool {
        self.calendar.is_some()
    }

    /// `useIgnoredEvents().ignoreEvent` / `ignoreSeries` over the
    /// `ignored_events` / `ignored_recurring_series` settings.
    pub(super) fn ignore_calendar_entry(
        &mut self,
        key: &'static str,
        field: &str,
        id: &str,
        cx: &mut Context<Self>,
    ) {
        let now = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        let next = ignore_entry(ignored_list(&self.provider_settings, key), field, id, &now);
        self.set_setting(key, serde_json::Value::Array(next), cx);
    }

    /// `unignoreEvent` / `unignoreSeries`: drop every entry with the id.
    pub(super) fn unignore_calendar_entry(
        &mut self,
        key: &'static str,
        field: &str,
        id: &str,
        cx: &mut Context<Self>,
    ) {
        let next: Vec<serde_json::Value> = ignored_list(&self.provider_settings, key)
            .into_iter()
            .filter(|entry| entry.get(field).and_then(|v| v.as_str()) != Some(id))
            .collect();
        self.set_setting(key, serde_json::Value::Array(next), cx);
    }

    /// `EventChip`'s context menu: `Delete Event`, or `Delete This Event` +
    /// `Delete All Recurring Events` for a recurring event.
    pub(super) fn close_calendar_context_menu(&mut self) -> bool {
        self.calendar
            .as_mut()
            .is_some_and(|state| state.context_menu.take().is_some())
    }

    pub(super) fn render_calendar_context_menu(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        use super::menu::{Align, Entry, MenuSpec, Trailing};
        let state = self.calendar.as_ref()?;
        let (event_id, position) = state.context_menu.clone()?;
        let event = self.event_rows.iter().find(|event| event.id == event_id)?;
        let tracking_id = event.tracking_id_event.clone();
        let series_id: Option<String> =
            Some(event.recurrence_series_id.clone()).filter(|id: &String| !id.is_empty());
        let mut entries = vec![Entry::Item {
            icon: None,
            dim_icon: false,
            label: if series_id.is_some() {
                "Delete This Event".into()
            } else {
                "Delete Event".into()
            },
            trailing: Trailing::None,
            destructive: false,
            on_select: Some(Box::new(move |this, _, cx| {
                if !tracking_id.is_empty() {
                    this.ignore_calendar_entry("ignored_events", "tracking_id", &tracking_id, cx);
                }
            })),
            submenu: None,
        }];
        if let Some(series_id) = series_id {
            entries.push(Entry::Item {
                icon: None,
                dim_icon: false,
                label: "Delete All Recurring Events".into(),
                trailing: Trailing::None,
                destructive: false,
                on_select: Some(Box::new(move |this, _, cx| {
                    this.ignore_calendar_entry("ignored_recurring_series", "id", &series_id, cx);
                })),
                submenu: None,
            });
        }
        let spec = MenuSpec {
            id: "calendar-event-menu",
            width: 224.0,
            entries,
            open_sub: None,
            on_hover_sub: |_, _, _| {},
            on_close: |this, cx| {
                if let Some(state) = this.calendar.as_mut() {
                    state.context_menu = None;
                    cx.notify();
                }
            },
        };
        Some(self.render_app_menu(spec, position, Align::Start, window, cx))
    }

    pub(crate) fn calendar_popover_open(&self) -> bool {
        self.calendar
            .as_ref()
            .is_some_and(|state| state.popover.is_some())
    }

    pub(crate) fn close_calendar_popover(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = self.calendar.as_mut()
            && state.popover.take().is_some()
        {
            cx.notify();
        }
    }

    /// `goToPrev` / `goToNext`: a month in the month view, `cols` days from
    /// the strip's current left edge in the compact view (`advanceCompact`).
    fn shift_calendar(&mut self, direction: i32, cx: &mut Context<Self>) {
        let Some(state) = self.calendar.as_mut() else {
            return;
        };
        let cols = state.compact_cols.get();
        if cols == 7 {
            self.shift_month(direction, cx);
            return;
        }
        let base = compact_visible_start(state);
        state.compact_start = base + Duration::days(direction as i64 * cols as i64);
        state.compact_scroll_reset.set(true);
        state.popover = None;
        cx.notify();
    }

    fn shift_month(&mut self, months: i32, cx: &mut Context<Self>) {
        let Some(state) = self.calendar.as_mut() else {
            return;
        };
        let (year, month0) = (state.month.year(), state.month.month0() as i32 + months);
        let year = year + month0.div_euclid(12);
        let month = month0.rem_euclid(12) as u32 + 1;
        if let Some(next) = NaiveDate::from_ymd_opt(year, month, 1) {
            state.month = next;
        }
        state.popover = None;
        cx.notify();
    }

    fn week_starts_on_monday(&self) -> bool {
        self.provider_settings
            .string_setting("week_start", &["general", "week_start"])
            .is_some_and(|value| value == "monday")
    }

    /// `CalendarSidebarContent`: the Google / Outlook accordion rows.
    pub(super) fn render_calendar_sidebar(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let Some(state) = self.calendar.as_ref() else {
            return div();
        };
        let providers: [(&'static str, &'static str, &'static str); 2] = [
            ("google", "Google", "brands/google-calendar.svg"),
            ("outlook", "Outlook", "brands/outlook.svg"),
        ];
        let mut list = div().flex().flex_col().px_2();
        for (id, label, asset) in providers {
            let open = state.expanded.contains(id);
            list = list
                .child(
                    // `-mx-2 rounded-full px-2 hover:bg-accent` grid row with the `py-3` trigger.
                    div()
                        .id(SharedString::from(format!("calendar-provider-{id}")))
                        .relative()
                        .mx(px(-8.0))
                        .flex()
                        .items_center()
                        .gap_1()
                        .rounded(px(8.0))
                        .px_2()
                        .cursor_pointer()
                        .hover(move |style| style.bg(theme.accent))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if let Some(state) = this.calendar.as_mut() {
                                if !state.expanded.remove(id) {
                                    state.expanded.insert(id);
                                }
                                cx.notify();
                            }
                        }))
                        .child(
                            div()
                                .flex()
                                .min_w_0()
                                .flex_grow()
                                .items_center()
                                .gap_2()
                                .py_3()
                                .tw_text_sm()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(theme.foreground)
                                .child(
                                    div()
                                        .flex()
                                        .size(px(20.0))
                                        .flex_shrink_0()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            gpui::img(super::note::embedded(asset)).size(px(16.0)),
                                        ),
                                )
                                .child(div().truncate().child(label)),
                        )
                        .child(icon(
                            if open { "caret-down" } else { "caret-right" },
                            px(16.0),
                            theme.muted_foreground,
                        )),
                )
                .when(open, |list| {
                    // `OAuthProviderContent` while signed out: the disabled connect line.
                    list.child(
                        div().pb_3().child(
                            div()
                                .pt_1()
                                .pb_2()
                                .tw_text_xs()
                                .text_color(theme.muted_foreground)
                                .opacity(0.5)
                                .cursor_not_allowed()
                                .child(SharedString::from(format!("Connect {label} Calendar"))),
                        ),
                    )
                });
        }
        div()
            .flex()
            .flex_col()
            .h_full()
            .w(px(self.custom_sidebar_width()))
            .flex_shrink_0()
            .pr_1()
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .h(px(48.0))
                    .flex_shrink_0()
                    .items_start()
                    .pt(px(9.0))
                    .pr_1()
                    .pl_2()
                    .child(
                        self.tracked_chrome_button("calendar-back", cx)
                            .on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| this.close_calendar(cx)),
                            )
                            .child(icon(
                                "arrow-left",
                                px(16.0),
                                self.chrome_icon_color("calendar-back"),
                            )),
                    ),
            )
            .child(
                div()
                    .id("calendar-providers")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(list),
            )
    }

    /// `CalendarView` in the seven-column month layout.
    pub(super) fn render_calendar_main(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        let Some(state) = self.calendar.as_ref() else {
            return div().into_any_element();
        };
        let monday_first = self.week_starts_on_monday();
        let headers: [&str; 7] = if monday_first {
            ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
        } else {
            ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
        };
        let month_start = state.month;
        let month_end = {
            let (y, m) = (month_start.year(), month_start.month());
            let next = if m == 12 {
                NaiveDate::from_ymd_opt(y + 1, 1, 1)
            } else {
                NaiveDate::from_ymd_opt(y, m + 1, 1)
            };
            next.map(|d| d - Duration::days(1)).unwrap_or(month_start)
        };
        let week_start = if monday_first {
            Weekday::Mon
        } else {
            Weekday::Sun
        };
        let days_back = (month_start.weekday().num_days_from_monday() as i64
            - week_start.num_days_from_monday() as i64)
            .rem_euclid(7);
        let cal_start = month_start - Duration::days(days_back);
        let days_forward = (week_start.num_days_from_monday() as i64 + 6
            - month_end.weekday().num_days_from_monday() as i64)
            .rem_euclid(7);
        let cal_end = month_end + Duration::days(days_forward);
        let total_days = (cal_end - cal_start).num_days() as usize + 1;
        let rows = total_days / 7;
        let today = Local::now().date_naive();
        let data = build_calendar_data(
            &self.session_rows,
            &self.event_rows,
            &self.provider_settings,
        );

        // The `auto-rows-fr` grid: the surface height minus the header (48)
        // and the weekday row (31) splits evenly across the rows.
        let viewport = window.viewport_size();
        // The `minmax(0, 1fr)` columns: the surface (viewport minus the
        // sidebar and its `pl-1` gutter, minus the left border) over `cols`.
        // Custom sidebars render no resize handle, so the surface starts at
        // 205 for the 200px sidebar (measured on the Tauri app: x=205, 595
        // wide at an 800px window).
        let surface_width = f32::from(viewport.width)
            - if self.sidebar_expanded && !self.is_standalone() {
                self.custom_sidebar_width() + 4.0 + 1.0
            } else {
                0.0
            };
        let cols = visible_cols(surface_width);
        let cell_width = surface_width / cols as f32;
        let title_bar = if self.is_standalone() { 0.0 } else { 36.0 };
        let grid_height = (f32::from(viewport.height) - title_bar - 48.0 - 31.0).max(0.0);
        let row_height = if cols == 7 {
            grid_height / rows.max(1) as f32
        } else {
            grid_height
        };
        // `p-1.5` (12) and the `h-7 mb-1` day number (32).
        let items_available = (row_height - 12.0 - 32.0).max(0.0);

        // The compact strip: 84 day columns, `cols` of them visible, scrolled
        // so `compact_start` sits at the left edge after a reset (`scrollTo`).
        let compact_days = (COMPACT_SCROLL_PAST_DAYS + COMPACT_SCROLL_FUTURE_DAYS) as usize;
        if cols != state.compact_cols.get() {
            state.compact_cols.set(cols);
            state.compact_scroll_reset.set(true);
        }
        state.compact_day_width.set(cell_width);
        if cols != 7 && state.compact_scroll_reset.get() {
            state.compact_scroll.set_offset(gpui::point(
                px(-(COMPACT_SCROLL_PAST_DAYS as f32) * cell_width),
                px(0.0),
            ));
            state.compact_scroll_reset.set(false);
        }
        let compact_first = state.compact_start - Duration::days(COMPACT_SCROLL_PAST_DAYS);
        let header_title = if cols == 7 {
            month_start.format("%B %Y").to_string()
        } else {
            compact_visible_start(state).format("%B %Y").to_string()
        };

        let header = div()
            .flex()
            .h(px(48.0))
            .flex_shrink_0()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(theme.border)
            .py_2()
            .px_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .tw_text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .child(SharedString::from(header_title)),
                    )
                    .child(
                        // `CalendarSyncHeaderControls`: the idle refresh button.
                        div()
                            .id("calendar-refresh")
                            .flex()
                            .size(px(24.0))
                            .items_center()
                            .justify_center()
                            .rounded(px(8.0))
                            .cursor_pointer()
                            .hover(move |style| style.bg(theme.accent))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.reload_sessions(cx);
                            }))
                            .child(icon("arrows-clockwise", px(14.0), theme.foreground)),
                    ),
            )
            .child(self.render_calendar_nav(cx));

        let weekday_row = div()
            .flex()
            .flex_shrink_0()
            .border_b_1()
            .border_color(theme.border)
            .children(headers.iter().enumerate().map(|(index, day)| {
                let weekend = *day == "Sat" || *day == "Sun";
                div()
                    .flex_1()
                    .min_w_0()
                    .py_2()
                    .text_center()
                    .tw_text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(if weekend {
                        theme.muted_foreground
                    } else {
                        theme.foreground
                    })
                    .when(index < 6, |cell| {
                        cell.border_r_1().border_color(theme.border)
                    })
                    .child(SharedString::from(*day))
            }));

        let mut grid = div().flex().flex_col().flex_1().min_h_0().overflow_hidden();
        for row in 0..rows {
            let mut week = div().flex().flex_1().min_h_0();
            for column in 0..7 {
                let day = cal_start + Duration::days((row * 7 + column) as i64);
                week = week.child(self.render_day_cell(
                    day,
                    day.month() == month_start.month() && day.year() == month_start.year(),
                    day == today,
                    &data,
                    items_available,
                    cell_width,
                    false,
                    state,
                    window,
                    cx,
                ));
            }
            grid = grid.child(week);
        }

        if cols != 7 {
            // `grid-rows-[auto_minmax(0,1fr)]` over `days.length` columns, each
            // `surface / cols` wide, in an `overflow-x-auto` strip.
            // `width: compactContentWidth` (`days.length / cols * 100%`): the
            // scroll container measures the strip, so it must size itself.
            let mut strip = div()
                .flex()
                .h_full()
                .w(px(cell_width * compact_days as f32));
            for offset in 0..compact_days {
                let day = compact_first + Duration::days(offset as i64);
                let label = day.format("%a").to_string();
                let weekend = matches!(day.weekday(), Weekday::Sat | Weekday::Sun);
                strip = strip.child(
                    div()
                        .w(px(cell_width))
                        .flex_shrink_0()
                        .h_full()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .w_full()
                                .flex_shrink_0()
                                .flex()
                                .justify_center()
                                .py_2()
                                .tw_text_xs()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .border_r_1()
                                .border_b_1()
                                .border_color(theme.border)
                                .text_color(if weekend {
                                    theme.muted_foreground
                                } else {
                                    theme.foreground
                                })
                                .child(SharedString::from(label)),
                        )
                        .child(
                            self.render_day_cell(
                                day,
                                true,
                                day == today,
                                &data,
                                items_available,
                                cell_width,
                                true,
                                state,
                                window,
                                cx,
                            )
                            .flex_1()
                            .min_h_0(),
                        ),
                );
            }
            return div()
                .flex()
                .h_full()
                .flex_col()
                .overflow_hidden()
                .child(header)
                .child(
                    div()
                        .id("calendar-compact")
                        .flex_1()
                        .min_h_0()
                        .overflow_x_scroll()
                        .track_scroll(&state.compact_scroll)
                        .child(strip),
                )
                .into_any_element();
        }

        div()
            .flex()
            .h_full()
            .flex_col()
            .overflow_hidden()
            .child(header)
            .child(weekday_row)
            .child(grid)
            .into_any_element()
    }

    /// The `ButtonGroup` (`h-7 rounded-full border bg-card`): ‹ · Today · ›.
    fn render_calendar_nav(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let segment = |id: &'static str, content: AnyElement, width: Option<f32>| {
            div()
                .id(id)
                .flex()
                .h_full()
                .items_center()
                .justify_center()
                .when_some(width, |segment, width| segment.w(px(width)))
                .when(width.is_none(), |segment| segment.px_2())
                .cursor_pointer()
                .hover(move |style| style.bg(theme.accent))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(content)
        };
        let separator = || div().w(px(1.0)).h_full().bg(theme.accent);
        div()
            .relative()
            .flex()
            .h(px(28.0))
            .overflow_hidden()
            .child(crate::squircle::squircle(
                crate::squircle::CONTROL_RADIUS,
                Some(theme.card),
                Some((1.0, theme.border)),
            ))
            .child(
                segment(
                    "calendar-prev",
                    icon("caret-left", px(14.0), theme.foreground).into_any_element(),
                    Some(28.0),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.shift_calendar(-1, cx))),
            )
            .child(separator())
            .child(
                segment(
                    "calendar-today",
                    div()
                        .tw_text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .child("Today")
                        .into_any_element(),
                    None,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if let Some(state) = this.calendar.as_mut() {
                        let today = Local::now().date_naive();
                        state.month = today.with_day(1).unwrap_or(today);
                        state.compact_start = today;
                        state.compact_scroll_reset.set(true);
                        state.popover = None;
                        cx.notify();
                    }
                })),
            )
            .child(separator())
            .child(
                segment(
                    "calendar-next",
                    icon("caret-right", px(14.0), theme.foreground).into_any_element(),
                    Some(28.0),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.shift_calendar(1, cx))),
            )
    }

    /// `DayCell`
    #[allow(clippy::too_many_arguments)]
    fn render_day_cell(
        &self,
        day: NaiveDate,
        in_month: bool,
        is_today: bool,
        data: &CalendarData,
        items_available: f32,
        cell_width: f32,
        fixed_width: bool,
        state: &CalendarState,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let weekend = matches!(day.weekday(), Weekday::Sat | Weekday::Sun);
        // Chips span the cell minus `p-1.5`.
        let chip_width = cell_width - 12.0;
        let events = data.events_by_date.get(&day).cloned().unwrap_or_default();
        let sessions = data.sessions_by_date.get(&day).cloned().unwrap_or_default();
        let total = events.len() + sessions.len();
        let max_visible = visible_item_count(items_available, total);
        let visible_events = events.len().min(max_visible);
        let visible_sessions = sessions.len().min(max_visible - visible_events);
        let overflow = total - visible_events - visible_sessions;

        let number_color = if is_today {
            theme.primary_foreground
        } else if !in_month {
            alpha(theme.muted_foreground, 0.7)
        } else if weekend {
            theme.muted_foreground
        } else {
            theme.foreground
        };

        let mut items = div()
            .flex()
            .min_h_0()
            .flex_1()
            .flex_col()
            .gap(px(CHIP_GAP))
            .overflow_hidden();
        for index in &events[..visible_events] {
            items = items.child(self.render_event_chip(
                &self.event_rows[*index],
                chip_width,
                state,
                window,
                cx,
            ));
        }
        for index in &sessions[..visible_sessions] {
            items = items.child(self.render_session_chip(
                &self.session_rows[*index],
                chip_width,
                state,
                window,
                cx,
            ));
        }
        if overflow > 0 {
            let more_open = state.popover == Some(Popover::More(day));
            items = items.child(
                div()
                    .relative()
                    .child(
                        div()
                            .id(SharedString::from(format!("calendar-more-{day}")))
                            .flex_shrink_0()
                            .pl_1()
                            .tw_text_xs()
                            .text_color(theme.muted_foreground)
                            .cursor_pointer()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if let Some(state) = this.calendar.as_mut() {
                                    state.popover = Some(Popover::More(day));
                                    cx.notify();
                                }
                            }))
                            .child(SharedString::from(format!("+{overflow} more"))),
                    )
                    .when(more_open, |anchor| {
                        let mut list = div().flex().flex_col().gap(px(CHIP_GAP));
                        // The `w-[220px] p-2` panel's chips.
                        for index in &events {
                            list = list.child(self.render_event_chip(
                                &self.event_rows[*index],
                                204.0,
                                state,
                                window,
                                cx,
                            ));
                        }
                        for index in &sessions {
                            list = list.child(self.render_session_chip(
                                &self.session_rows[*index],
                                204.0,
                                state,
                                window,
                                cx,
                            ));
                        }
                        anchor.child(
                            self.render_calendar_popover(
                                220.0,
                                cx,
                                div()
                                    .p_2()
                                    .child(
                                        div()
                                            .mb_2()
                                            .tw_text_sm()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(theme.foreground)
                                            .child(SharedString::from(
                                                day.format("%b %-d, %Y").to_string(),
                                            )),
                                    )
                                    .child(list)
                                    .into_any_element(),
                            ),
                        )
                    }),
            );
        }

        div()
            .when(fixed_width, |cell| cell.w(px(cell_width)).flex_shrink_0())
            .when(!fixed_width, |cell| cell.flex_1())
            .min_w_0()
            .flex()
            .flex_col()
            .p(px(6.0))
            .border_r_1()
            .border_b_1()
            .border_color(theme.border)
            .when(weekend, |cell| cell.bg(theme.muted))
            .child(
                div().flex().flex_shrink_0().justify_end().child(
                    div()
                        .mb_1()
                        .flex()
                        .size(px(28.0))
                        .items_center()
                        .justify_center()
                        // `.rounded-full` is `0.5rem` in the desktop app.
                        .rounded(px(8.0))
                        .when(is_today, |pill| pill.bg(theme.primary))
                        .tw_text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(number_color)
                        .child(SharedString::from(day.day().to_string())),
                ),
            )
            .child(items)
    }

    /// `EventChip`: the colour bar, title, and `font-mono` start time; all-day
    /// events fill the chip with the calendar colour.
    fn render_event_chip(
        &self,
        event: &EventRow,
        chip_width: f32,
        state: &CalendarState,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let color =
            super::note::parse_hex_color(&event.calendar_color).unwrap_or(gpui::rgb(0x888888));
        let id = event.id.clone();
        let open = state.popover == Some(Popover::Event(event.id.clone()));
        let time = format_time(&event.started_at);
        let time_width = time
            .as_deref()
            .map(|time| mono_text_width(time, self.mono_font_family.as_ref(), window) + 4.0)
            .unwrap_or(0.0);
        // `pl-0.5`, the bar, `gap-1`, and the time (with its gap) leave the title width.
        let title_width = (chip_width - 2.0 - 2.5 - 4.0 - time_width).max(0.0);
        let chip = if event.is_all_day != 0 {
            div()
                .id(SharedString::from(format!("event-chip-{}", event.id)))
                .w_full()
                .rounded(px(4.0))
                .px(px(6.0))
                .py(px(2.0))
                .bg(color)
                .text_size(px(12.0))
                .line_height(px(CHIP_HEIGHT))
                .text_color(theme.primary_foreground)
                .child(
                    div()
                        .w(px((chip_width - 12.0).max(0.0)))
                        .truncate()
                        .child(SharedString::from(event.title.clone())),
                )
        } else {
            div()
                .id(SharedString::from(format!("event-chip-{}", event.id)))
                .flex()
                .w_full()
                .items_center()
                .gap_1()
                .rounded(px(4.0))
                .pl(px(2.0))
                .text_size(px(12.0))
                .line_height(px(CHIP_HEIGHT))
                .text_color(theme.foreground)
                .child(
                    div()
                        .w(px(2.5))
                        .h(px(CHIP_HEIGHT))
                        .flex_shrink_0()
                        .rounded(px(8.0))
                        .bg(color),
                )
                .child(
                    div().min_w_0().flex_grow().flex().flex_col().child(
                        div()
                            .w(px(title_width))
                            .truncate()
                            .child(SharedString::from(event.title.clone())),
                    ),
                )
                .when_some(time, |chip, time| {
                    chip.child(
                        div()
                            .flex_shrink_0()
                            .text_color(theme.muted_foreground)
                            .when_some(self.mono_font_family.clone(), |t, family| {
                                t.font_family(family)
                            })
                            .child(SharedString::from(time)),
                    )
                })
        };
        div()
            .relative()
            .child(
                chip.cursor_pointer()
                    .hover(|style| style.opacity(0.8))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_down(MouseButton::Right, {
                        let id = id.clone();
                        cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            if let Some(state) = this.calendar.as_mut() {
                                state.context_menu = Some((id.clone(), event.position));
                                cx.notify();
                            }
                        })
                    })
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(state) = this.calendar.as_mut() {
                            state.popover = Some(Popover::Event(id.clone()));
                            cx.notify();
                        }
                    })),
            )
            .when(open, |anchor| {
                anchor.child(self.render_calendar_popover(
                    320.0,
                    cx,
                    self.render_event_popover(event, cx),
                ))
            })
    }

    /// `SessionChip`: the bordered bar, title, and `font-mono` created time.
    fn render_session_chip(
        &self,
        session: &SessionRow,
        chip_width: f32,
        state: &CalendarState,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let id = session.id.clone();
        let open = state.popover == Some(Popover::Session(session.id.clone()));
        let time = format_time(&session.created_at);
        let time_width = time
            .as_deref()
            .map(|time| mono_text_width(time, self.mono_font_family.as_ref(), window) + 4.0)
            .unwrap_or(0.0);
        let title_width = (chip_width - 2.0 - 4.0 - 4.0 - time_width).max(0.0);
        div()
            .relative()
            .child(
                div()
                    .id(SharedString::from(format!("session-chip-{}", session.id)))
                    .flex()
                    .w_full()
                    .items_center()
                    .gap_1()
                    .rounded(px(4.0))
                    .pl(px(2.0))
                    .text_size(px(12.0))
                    .line_height(px(CHIP_HEIGHT))
                    .text_color(theme.foreground)
                    .cursor_pointer()
                    .hover(|style| style.opacity(0.8))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(state) = this.calendar.as_mut() {
                            state.popover = Some(Popover::Session(id.clone()));
                            cx.notify();
                        }
                    }))
                    .child(
                        div()
                            .w(px(4.0))
                            .h(px(CHIP_HEIGHT))
                            .flex_shrink_0()
                            .rounded(px(8.0))
                            .border_1()
                            .border_color(theme.border),
                    )
                    .child(
                        div().min_w_0().flex_grow().flex().flex_col().child(
                            div()
                                .w(px(title_width))
                                .truncate()
                                .child(SharedString::from(session.title.clone())),
                        ),
                    )
                    .when_some(time, |chip, time| {
                        chip.child(
                            div()
                                .flex_shrink_0()
                                .text_color(theme.muted_foreground)
                                .when_some(self.mono_font_family.clone(), |t, family| {
                                    t.font_family(family)
                                })
                                .child(SharedString::from(time)),
                        )
                    }),
            )
            .when(open, |anchor| {
                anchor.child(self.render_calendar_popover(
                    280.0,
                    cx,
                    self.render_session_popover(session, cx),
                ))
            })
    }

    /// `EventPopoverContent`: `EventDisplay` plus the Open note button.
    fn render_event_popover(&self, event: &EventRow, cx: &Context<Self>) -> AnyElement {
        let session_event = crate::timeline::SessionEvent {
            tracking_id: Some(event.tracking_id_event.clone()),
            title: Some(event.title.clone()),
            description: Some(event.description.clone()),
            started_at: Some(event.started_at.clone()),
            ended_at: Some(event.ended_at.clone()),
            meeting_link: Some(event.meeting_link.clone()),
            location: Some(event.location.clone()),
            participants: Vec::new(),
        };
        let id = event.id.clone();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(self.render_event_display(&session_event, cx))
            .child(self.open_note_button(
                "event-open-note",
                move |this, cx| {
                    this.close_calendar(cx);
                    this.open_event(id.clone(), cx);
                },
                cx,
            ))
            .into_any_element()
    }

    /// `SessionPopoverContent`
    fn render_session_popover(&self, session: &SessionRow, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        let id = session.id.clone();
        let created = crate::timeline::parse_date(&session.created_at, &Local).map(|utc| {
            utc.with_timezone(&Local)
                .format("%b %-d, %Y %-I:%M %p")
                .to_string()
        });
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(
                div()
                    .tw_text_base()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .child(SharedString::from(session.title.clone())),
            )
            .child(div().h(px(1.0)).bg(theme.accent))
            .when_some(created, |column, created| {
                column.child(
                    div()
                        .tw_text_sm()
                        .text_color(theme.muted_foreground)
                        .child(SharedString::from(created)),
                )
            })
            .child(self.open_note_button(
                "session-open-note",
                move |this, cx| {
                    this.close_calendar(cx);
                    this.select_session_from_calendar(id.clone(), cx);
                },
                cx,
            ))
            .into_any_element()
    }

    pub(crate) fn select_session_from_calendar(&mut self, id: String, cx: &mut Context<Self>) {
        self.open_tab(id, false, cx);
    }

    /// `Button size="sm" bg-primary min-h-8 w-full`
    fn open_note_button(
        &self,
        id: &'static str,
        on_click: impl Fn(&mut Workspace, &mut Context<Workspace>) + 'static,
        cx: &Context<Self>,
    ) -> gpui::Stateful<Div> {
        let theme = self.theme;
        let hovered = self.hovered == Some(id);
        div()
            .id(id)
            .relative()
            .flex()
            .min_h(px(32.0))
            .w_full()
            .items_center()
            .justify_center()
            .px_3()
            .child(crate::squircle::squircle(
                crate::squircle::CONTROL_RADIUS,
                Some(if hovered {
                    alpha(theme.primary, 0.9)
                } else {
                    theme.primary
                }),
                None,
            ))
            .tw_text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(theme.primary_foreground)
            .cursor_pointer()
            .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                this.set_hovered(id, *hovering, cx);
            }))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| on_click(this, cx)))
            .child("Open note")
    }

    /// A `variant="app"` popover panel anchored under its trigger, closing
    /// on an outside press.
    fn render_calendar_popover(
        &self,
        width: f32,
        cx: &Context<Self>,
        content: AnyElement,
    ) -> AnyElement {
        let theme = self.theme;
        div()
            .absolute()
            .top(px(CHIP_HEIGHT + 4.0))
            .left_0()
            .child(
                gpui::deferred(
                    gpui::anchored()
                        .anchor(gpui::Corner::TopLeft)
                        .snap_to_window_with_margin(px(16.0))
                        .child(
                            div()
                                .id("calendar-popover")
                                .occlude()
                                .relative()
                                .w(px(width))
                                .child(crate::squircle::squircle(
                                    crate::squircle::PANEL_RADIUS,
                                    Some(theme.floating_panel),
                                    Some((1.0, theme.floating_border)),
                                ))
                                .shadow(vec![gpui::BoxShadow {
                                    color: gpui::hsla(0.0, 0.0, 0.0, 0.1),
                                    offset: gpui::point(px(0.0), px(8.0)),
                                    blur_radius: px(24.0),
                                    spread_radius: px(0.0),
                                }])
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_mouse_down_out(cx.listener(
                                    |this, _: &MouseDownEvent, _, cx| {
                                        this.close_calendar_popover(cx);
                                    },
                                ))
                                .child(content),
                        ),
                )
                .with_priority(2),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignoring_replaces_the_same_id_and_appends_last_seen() {
        let list = vec![
            serde_json::json!({ "tracking_id": "a", "last_seen": "old" }),
            serde_json::json!({ "tracking_id": "b", "last_seen": "old" }),
        ];
        let next = ignore_entry(list, "tracking_id", "a", "now");
        assert_eq!(
            next,
            vec![
                serde_json::json!({ "tracking_id": "b", "last_seen": "old" }),
                serde_json::json!({ "tracking_id": "a", "last_seen": "now" }),
            ]
        );
        assert_eq!(
            ignore_entry(Vec::new(), "id", "series-1", "now"),
            vec![serde_json::json!({ "id": "series-1", "last_seen": "now" })]
        );
    }
    use chrono::{TimeZone, Utc};

    #[test]
    fn view_breakpoints_match_the_app() {
        assert_eq!(visible_cols(816.0), 7);
        assert_eq!(visible_cols(700.0), 7);
        assert_eq!(visible_cols(699.0), 4);
        assert_eq!(visible_cols(400.0), 4);
        assert_eq!(visible_cols(399.0), 2);
        assert_eq!(visible_cols(200.0), 2);
        assert_eq!(visible_cols(150.0), 1);
    }

    #[test]
    fn compact_start_index_rounds_and_clamps_like_handle_compact_scroll() {
        // 84 days, 4 visible: the reset scroll (42 columns) lands on day 42,
        // a half-column drag rounds, and the end clamps to `days.length - cols`.
        assert_eq!(compact_start_index(42.0 * 150.0, 150.0, 84, 4), 42);
        assert_eq!(compact_start_index(42.6 * 150.0, 150.0, 84, 4), 43);
        assert_eq!(compact_start_index(1_000_000.0, 150.0, 84, 4), 80);
        assert_eq!(compact_start_index(-10.0, 150.0, 84, 4), 0);
        assert_eq!(compact_start_index(300.0, 0.0, 84, 4), 0);
    }

    #[test]
    fn visible_item_count_keeps_room_for_the_more_line() {
        assert_eq!(visible_item_count(100.0, 0), 0);
        // Three chips fit outright: 15 + 2 + 15 + 2 + 15 = 49.
        assert_eq!(visible_item_count(49.0, 3), 3);
        // Five chips need 83px; in 49px two chips fit with the 17px reserved
        // for the more line, in 66px three do.
        assert_eq!(visible_item_count(49.0, 5), 2);
        assert_eq!(visible_item_count(66.0, 5), 3);
        assert_eq!(visible_item_count(5.0, 5), 1);
    }

    #[test]
    fn all_day_events_keep_their_stored_date() {
        let event = EventRow {
            started_at: "2026-09-05".into(),
            is_all_day: 1,
            ..EventRow::default()
        };
        assert_eq!(event_day(&event), NaiveDate::from_ymd_opt(2026, 9, 5));
        let bad = EventRow {
            started_at: "nope".into(),
            is_all_day: 1,
            ..EventRow::default()
        };
        assert_eq!(event_day(&bad), None);
    }

    #[test]
    fn timed_events_use_the_local_day() {
        let event = EventRow {
            started_at: "2026-09-05T12:00:00.000Z".into(),
            ..EventRow::default()
        };
        let expected = Utc
            .with_ymd_and_hms(2026, 9, 5, 12, 0, 0)
            .unwrap()
            .with_timezone(&Local)
            .date_naive();
        assert_eq!(event_day(&event), Some(expected));
    }
}
