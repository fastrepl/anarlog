use std::{collections::HashSet, sync::Arc};

use anlg_fs_sync_core::normalize_folder_path;
use chrono::{DateTime, Datelike, FixedOffset, Local, NaiveDate, Utc};
use chrono_tz::Tz;
use desktop_runtime::{LibraryPage, SessionId};
use gpui::{
    Context, EventEmitter, FocusHandle, ListAlignment, ListState, Render, Window, div, list,
    prelude::*, px, svg,
};

use crate::ui::theme::{monospace_font, theme};

pub struct LibraryView {
    page: Arc<LibraryPage>,
    selection: Selection,
    active: Option<SessionId>,
    recording: HashSet<SessionId>,
    scroll: ListState,
    rows: Vec<TimelineRow>,
    focus: FocusHandle,
    menu: Option<NoteMenu>,
    menu_index: usize,
    use_24_hour_time: bool,
    timezone: Option<Tz>,
    show_folder: bool,
    show_tags: bool,
}

enum TimelineRow {
    Heading(String),
    Note {
        index: usize,
        time: String,
        folder: String,
        tags: Arc<str>,
    },
}

#[derive(Clone)]
struct NoteMenu {
    position: gpui::Point<gpui::Pixels>,
    ids: Arc<[SessionId]>,
    bulk: bool,
}

impl NoteMenu {
    fn labels(&self) -> Vec<String> {
        if self.bulk {
            vec![format!("Delete Selected ({})", self.ids.len())]
        } else {
            vec![
                "Open in New Window".into(),
                if cfg!(target_os = "macos") {
                    "Show in Finder"
                } else {
                    "Show in folder"
                }
                .into(),
                "Delete Note".into(),
            ]
        }
    }
}

fn bucket(date: NaiveDate, today: NaiveDate) -> String {
    let days = date.signed_duration_since(today).num_days();
    match days {
        0 => "Today".into(),
        -1 => "Yesterday".into(),
        1 => "Tomorrow".into(),
        -6..=-2 => format!("{} days ago", -days),
        2..=6 => format!("in {days} days"),
        -27..=27 => {
            let weeks = (days.unsigned_abs() + 3) / 7;
            match (days < 0, weeks) {
                (true, 1) => "a week ago".into(),
                (true, _) => format!("{weeks} weeks ago"),
                (false, 1) => "next week".into(),
                (false, _) => format!("in {weeks} weeks"),
            }
        }
        _ => {
            let months = ((date.year() - today.year()) * 12 + date.month() as i32
                - today.month() as i32)
                .unsigned_abs()
                .max(1);
            match (days < 0, months) {
                (true, 1) => "a month ago".into(),
                (true, _) => format!("{months} months ago"),
                (false, 1) => "next month".into(),
                (false, _) => format!("in {months} months"),
            }
        }
    }
}

fn local_time(date: DateTime<Utc>, timezone: Option<Tz>) -> DateTime<FixedOffset> {
    match timezone {
        Some(timezone) => date.with_timezone(&timezone).fixed_offset(),
        None => date.with_timezone(&Local).fixed_offset(),
    }
}

pub(super) fn folder_label(path: &str) -> String {
    let normalized = normalize_folder_path(path.trim()).unwrap_or_default();
    if normalized.encode_utf16().count() > 200
        || normalized
            .split('/')
            .any(|segment| segment.encode_utf16().count() > 80)
    {
        String::new()
    } else {
        normalized
    }
}

fn timestamp(date: DateTime<FixedOffset>, today: NaiveDate, use_24_hour_time: bool) -> String {
    let time = date
        .format(if use_24_hour_time {
            "%H:%M"
        } else {
            "%-I:%M %p"
        })
        .to_string();
    if date
        .date_naive()
        .signed_duration_since(today)
        .num_days()
        .abs()
        < 7
    {
        return time;
    }
    let date = date.format(if date.year() == today.year() {
        "%b %-d"
    } else {
        "%b %-d, %Y"
    });
    format!("{date}, {time}")
}

#[derive(Clone)]
pub enum OpenNote {
    Current(SessionId),
    Window(SessionId),
    Reveal(SessionId),
    Delete(Arc<[SessionId]>),
    Move(Arc<[SessionId]>),
}
impl EventEmitter<OpenNote> for LibraryView {}

#[derive(Default)]
struct Selection {
    ids: HashSet<SessionId>,
    anchor: Option<SessionId>,
    cursor: Option<SessionId>,
}

impl Selection {
    fn click(&mut self, id: SessionId, visible: &[SessionId], additive: bool, range: bool) {
        self.cursor = Some(id.clone());
        if range
            && let Some(from) = self
                .anchor
                .as_ref()
                .and_then(|anchor| visible.iter().position(|id| id == anchor))
            && let Some(to) = visible.iter().position(|item| item == &id)
        {
            if !additive {
                self.ids.clear();
            }
            self.ids
                .extend(visible[from.min(to)..=from.max(to)].iter().cloned());
            return;
        }
        if additive {
            if !self.ids.remove(&id) {
                self.ids.insert(id.clone());
            }
        } else {
            self.ids.clear();
            if range {
                self.ids.insert(id.clone());
            }
        }
        self.anchor = Some(id);
    }
}

impl LibraryView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            page: Arc::new(LibraryPage {
                items: Arc::from([]),
                offset: 0,
                has_more: false,
            }),
            selection: Selection::default(),
            active: None,
            recording: HashSet::new(),
            scroll: ListState::new(0, ListAlignment::Top, px(256.)),
            rows: Vec::new(),
            focus: cx.focus_handle(),
            menu: None,
            menu_index: 0,
            use_24_hour_time: false,
            timezone: None,
            show_folder: true,
            show_tags: false,
        }
    }

    pub fn set_clock(
        &mut self,
        use_24_hour_time: bool,
        timezone: Option<Tz>,
        cx: &mut Context<Self>,
    ) {
        if (self.use_24_hour_time, self.timezone) != (use_24_hour_time, timezone) {
            self.use_24_hour_time = use_24_hour_time;
            self.timezone = timezone;
            self.set_page(self.page.clone(), cx);
        }
    }

    pub fn set_metadata(&mut self, show_folder: bool, show_tags: bool, cx: &mut Context<Self>) {
        if (self.show_folder, self.show_tags) != (show_folder, show_tags) {
            self.show_folder = show_folder;
            self.show_tags = show_tags;
            self.set_page(self.page.clone(), cx);
        }
    }

    pub fn set_page(&mut self, page: Arc<LibraryPage>, cx: &mut Context<Self>) {
        self.selection
            .ids
            .retain(|id| page.items.iter().any(|item| &item.id == id));
        let mut rows = Vec::new();
        let today = local_time(Utc::now(), self.timezone).date_naive();
        let mut previous = String::new();
        for (index, item) in page.items.iter().enumerate() {
            let date = DateTime::parse_from_rfc3339(&item.created_at)
                .ok()
                .map(|date| local_time(date.with_timezone(&Utc), self.timezone));
            let heading = date
                .map(|date| bucket(date.date_naive(), today))
                .unwrap_or_else(|| "Undated".into());
            if heading != previous {
                rows.push(TimelineRow::Heading(heading.clone()));
                previous = heading;
            }
            let time = date
                .map(|date| timestamp(date, today, self.use_24_hour_time))
                .unwrap_or_default();
            let folder = if self.show_folder {
                folder_label(&item.folder_path)
            } else {
                String::new()
            };
            rows.push(TimelineRow::Note {
                index,
                time,
                folder,
                tags: if self.show_tags {
                    item.tag_line.clone()
                } else {
                    Arc::default()
                },
            });
        }
        if self.rows.is_empty() || page.offset != self.page.offset {
            self.scroll.reset(rows.len());
        } else {
            self.scroll.splice(0..self.rows.len(), rows.len());
        }
        self.rows = rows;
        self.page = page;
        cx.notify();
    }

    pub fn set_active(&mut self, active: Option<SessionId>, cx: &mut Context<Self>) {
        if self.active != active {
            self.active = active;
            cx.notify();
        }
    }

    pub fn set_recording(&mut self, id: SessionId, active: bool, cx: &mut Context<Self>) {
        let changed = if active {
            self.recording.insert(id)
        } else {
            self.recording.remove(&id)
        };
        if changed {
            cx.notify();
        }
    }

    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if !self.selection.ids.is_empty() {
            self.selection = Selection::default();
            cx.notify();
        }
    }

    fn menu_action(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(menu) = self.menu.take() else {
            return;
        };
        if !menu.ids.is_empty() {
            match (menu.bulk, index) {
                (false, 0) => cx.emit(OpenNote::Window(menu.ids[0].clone())),
                (false, 1) => cx.emit(OpenNote::Reveal(menu.ids[0].clone())),
                (false, 2) | (true, 0) => cx.emit(OpenNote::Delete(menu.ids)),
                _ => {}
            }
        }
        cx.notify();
    }

    fn selected_ids(&self) -> Arc<[SessionId]> {
        self.page
            .items
            .iter()
            .filter(|item| self.selection.ids.contains(&item.id))
            .map(|item| item.id.clone())
            .collect()
    }
}

impl Render for LibraryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        div()
            .size_full()
            .relative()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if let Some(menu) = &this.menu {
                    let count = menu.labels().len();
                    match event.keystroke.key.as_str() {
                        "escape" => this.menu = None,
                        "down" => this.menu_index = (this.menu_index + 1) % count,
                        "up" => this.menu_index = (this.menu_index + count - 1) % count,
                        "enter" => this.menu_action(this.menu_index, cx),
                        _ => return,
                    }
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
                if event.keystroke.key == "delete" || event.keystroke.key == "backspace" {
                    let ids = this.selected_ids();
                    if !ids.is_empty() {
                        cx.emit(OpenNote::Delete(ids));
                    }
                    cx.stop_propagation();
                    return;
                }
                if matches!(event.keystroke.key.as_str(), "up" | "down")
                    && !this.page.items.is_empty()
                {
                    let visible = this
                        .page
                        .items
                        .iter()
                        .map(|item| item.id.clone())
                        .collect::<Vec<_>>();
                    let anchor = this
                        .selection
                        .cursor
                        .as_ref()
                        .and_then(|id| visible.iter().position(|item| item == id));
                    let index = match (event.keystroke.key.as_str(), anchor) {
                        ("up", Some(index)) => index.saturating_sub(1),
                        (_, Some(index)) => (index + 1).min(visible.len() - 1),
                        _ => 0,
                    };
                    this.selection.click(
                        visible[index].clone(),
                        &visible,
                        false,
                        event.keystroke.modifiers.shift,
                    );
                    if !event.keystroke.modifiers.shift {
                        cx.emit(OpenNote::Current(visible[index].clone()));
                    }
                    cx.stop_propagation();
                    cx.notify();
                    return;
                }
                if event.keystroke.key == "enter" {
                    if let Some(id) = this.selection.anchor.clone() {
                        cx.emit(OpenNote::Current(id));
                    }
                    cx.stop_propagation();
                    return;
                }
                if event.keystroke.modifiers.secondary(cx) && event.keystroke.key == "a" {
                    this.selection.ids =
                        this.page.items.iter().map(|item| item.id.clone()).collect();
                    cx.stop_propagation();
                    cx.notify();
                } else if event.keystroke.key == "escape" && !this.selection.ids.is_empty() {
                    this.clear_selection(cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                list(
                    self.scroll.clone(),
                    cx.processor(move |this, row: usize, _, cx| {
                        let Some(row) = this.rows.get(row) else {
                            return div().into_any_element();
                        };
                        let (index, time, folder, tags) = match row {
                            TimelineRow::Heading(title) => {
                                return div()
                                    .h(px(32.))
                                    .px_3()
                                    .pt_2()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(title.clone())
                                    .into_any_element();
                            }
                            TimelineRow::Note {
                                index,
                                time,
                                folder,
                                tags,
                            } => (*index, time.clone(), folder.clone(), tags.clone()),
                        };
                        let item = &this.page.items[index];
                        let id = item.id.clone();
                        let item = div()
                            .id(gpui::SharedString::from(item.id.0.clone()))
                            .flex_1()
                            .min_w_0()
                            .h(px(54.
                                + 18.
                                    * (usize::from(!folder.is_empty())
                                        + usize::from(!tags.is_empty()))
                                        as f32))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .px_3()
                            .py_2()
                            .mx_1()
                            .rounded(px(8.))
                            .hover(|style| style.bg(colors.accent.opacity(0.5)))
                            .when(
                                this.selection.ids.contains(&id)
                                    || this.active.as_ref() == Some(&id),
                                |view| view.bg(colors.accent),
                            )
                            .cursor_pointer()
                            .overflow_hidden()
                            .on_mouse_down(
                                gpui::MouseButton::Right,
                                cx.listener({
                                    let id = item.id.clone();
                                    move |this, event: &gpui::MouseDownEvent, window, cx| {
                                        this.focus.focus(window, cx);
                                        let ids = this.selected_ids();
                                        let bulk = !ids.is_empty();
                                        this.menu = Some(NoteMenu {
                                            position: event.position,
                                            ids: if bulk { ids } else { vec![id.clone()].into() },
                                            bulk,
                                        });
                                        this.menu_index = 0;
                                        cx.stop_propagation();
                                        cx.notify();
                                    }
                                }),
                            )
                            .on_click(cx.listener(
                                move |this, event: &gpui::ClickEvent, window, cx| {
                                    this.focus.focus(window, cx);
                                    let modifiers = event.modifiers();
                                    let visible = this
                                        .page
                                        .items
                                        .iter()
                                        .map(|row| row.id.clone())
                                        .collect::<Vec<_>>();
                                    this.selection.click(
                                        id.clone(),
                                        &visible,
                                        modifiers.secondary(cx),
                                        modifiers.shift,
                                    );
                                    if !modifiers.secondary(cx) && !modifiers.shift {
                                        if event.click_count() == 2 {
                                            cx.emit(OpenNote::Window(id.clone()));
                                        } else if event.click_count() == 1 {
                                            cx.emit(OpenNote::Current(id.clone()));
                                        }
                                    }
                                    cx.notify();
                                },
                            ))
                            .when(!folder.is_empty(), |view| {
                                view.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .min_w_0()
                                        .flex_shrink_0()
                                        .h(px(16.))
                                        .gap(px(4.))
                                        .text_size(px(11.))
                                        .line_height(px(16.))
                                        .text_color(colors.muted_foreground)
                                        .child(
                                            svg()
                                                .path("workspace/Folder01Icon.svg")
                                                .size(px(12.))
                                                .flex_shrink_0()
                                                .text_color(colors.muted_foreground),
                                        )
                                        .child(div().min_w_0().truncate().child(folder)),
                                )
                            })
                            .child(div().text_sm().line_height(px(20.)).truncate().child(
                                if item.title.is_empty() {
                                    "Untitled".into()
                                } else {
                                    gpui::SharedString::from(item.title.clone())
                                },
                            ))
                            .child(
                                div()
                                    .font_family(monospace_font(cx))
                                    .text_xs()
                                    .line_height(px(16.))
                                    .text_color(colors.muted_foreground)
                                    .child(if this.recording.contains(&item.id) {
                                        "Recording".into()
                                    } else {
                                        gpui::SharedString::from(time)
                                    }),
                            )
                            .when(!tags.is_empty(), |view| {
                                view.child(
                                    div()
                                        .w_full()
                                        .min_w_0()
                                        .flex_shrink_0()
                                        .h(px(16.))
                                        .text_size(px(11.))
                                        .line_height(px(16.))
                                        .text_color(colors.muted_foreground)
                                        .line_clamp(1)
                                        .text_ellipsis()
                                        .child(gpui::SharedString::from(tags)),
                                )
                            });
                        div().w_full().flex().child(item).into_any_element()
                    }),
                )
                .size_full(),
            )
            .when_some(self.menu.clone(), |view, menu| {
                view.child(gpui::deferred(
                    gpui::anchored()
                        .position(menu.position)
                        .snap_to_window()
                        .child(
                            div()
                                .id("note-context-menu")
                                .w(px(224.))
                                .p_1()
                                .rounded(px(8.))
                                .bg(colors.card)
                                .border_1()
                                .border_color(colors.border)
                                .shadow_lg()
                                .occlude()
                                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation();
                                })
                                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                    this.menu = None;
                                    cx.notify();
                                }))
                                .children(menu.labels().into_iter().enumerate().map(
                                    |(index, label)| {
                                        div()
                                            .id(index)
                                            .h(px(30.))
                                            .px_3()
                                            .flex()
                                            .items_center()
                                            .text_sm()
                                            .rounded(px(4.))
                                            .cursor_pointer()
                                            .when(index == self.menu_index, |view| {
                                                view.bg(colors.accent)
                                            })
                                            .hover(|style| style.bg(colors.accent))
                                            .child(label)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.menu_action(index, cx);
                                            }))
                                    },
                                )),
                        ),
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidebar_folders_follow_shipping_normalization_and_utf16_limits() {
        for (input, expected) in [
            ("  Work\\日本語/  ", "Work/日本語"),
            ("   ", ""),
            ("/Work", ""),
            ("Work//Meetings", ""),
            ("Work/../Meetings", ""),
            ("./Work", ""),
        ] {
            assert_eq!(folder_label(input), expected);
        }
        assert_eq!(folder_label(&"🚀".repeat(40)), "🚀".repeat(40));
        assert_eq!(folder_label(&"🚀".repeat(41)), "");
        assert_eq!(folder_label(&format!("{0}/{0}/{0}", "a".repeat(70))), "");
    }

    #[test]
    fn timeline_timestamps_keep_time_and_only_show_the_year_when_needed() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        for (date, twelve_hour, twenty_four_hour) in [
            ("2026-01-15T00:05:00Z", "12:05 AM", "00:05"),
            ("2026-01-15T12:05:00Z", "12:05 PM", "12:05"),
            ("2026-01-09T23:59:00Z", "11:59 PM", "23:59"),
            ("2026-01-08T13:02:00Z", "Jan 8, 1:02 PM", "Jan 8, 13:02"),
            ("2026-01-21T13:02:00Z", "1:02 PM", "13:02"),
            ("2026-01-22T13:02:00Z", "Jan 22, 1:02 PM", "Jan 22, 13:02"),
            (
                "2025-12-31T13:02:00Z",
                "Dec 31, 2025, 1:02 PM",
                "Dec 31, 2025, 13:02",
            ),
        ] {
            let date = DateTime::parse_from_rfc3339(date).unwrap();
            assert_eq!(timestamp(date, today, false), twelve_hour);
            assert_eq!(timestamp(date, today, true), twenty_four_hour);
        }
    }

    #[test]
    fn timeline_timezone_applies_to_grouping_year_boundaries_and_dst() {
        let zone = Some(chrono_tz::America::Los_Angeles);
        let date = |value: &str| {
            local_time(
                DateTime::parse_from_rfc3339(value)
                    .unwrap()
                    .with_timezone(&Utc),
                zone,
            )
        };
        let previous_year = date("2026-01-01T00:30:00Z");
        let today = date("2026-01-08T00:00:00Z").date_naive();
        assert_eq!(bucket(previous_year.date_naive(), today), "a week ago");
        assert_eq!(
            timestamp(previous_year, today, false),
            "Dec 31, 2025, 4:30 PM"
        );

        let before = date("2026-03-08T09:59:00Z");
        let after = date("2026-03-08T10:00:00Z");
        assert_eq!(timestamp(before, after.date_naive(), false), "1:59 AM");
        assert_eq!(timestamp(after, after.date_naive(), false), "3:00 AM");
        assert_eq!(bucket(before.date_naive(), after.date_naive()), "Today");

        for instant in ["2026-11-01T08:30:00Z", "2026-11-01T09:30:00Z"] {
            let repeated_hour = date(instant);
            assert_eq!(
                timestamp(repeated_hour, repeated_hour.date_naive(), true),
                "01:30"
            );
        }
    }

    #[test]
    fn opening_a_note_sets_the_range_anchor_without_selecting_it_for_deletion() {
        let ids = ["a", "b", "c"].map(|id| SessionId(id.into()));
        let mut selection = Selection::default();
        selection.click(ids[0].clone(), &ids, false, false);
        assert!(selection.ids.is_empty());
        selection.click(ids[2].clone(), &ids, false, true);
        assert_eq!(selection.ids, HashSet::from(ids.clone()));
        selection.click(ids[1].clone(), &ids, false, false);
        assert!(selection.ids.is_empty());
        assert_eq!(selection.anchor, Some(ids[1].clone()));
    }

    #[test]
    fn timeline_buckets_match_the_source_relative_day_week_and_month_boundaries() {
        let today = NaiveDate::from_ymd_opt(2026, 9, 21).unwrap();
        for (days, label) in [
            (0, "Today"),
            (-1, "Yesterday"),
            (1, "Tomorrow"),
            (-6, "6 days ago"),
            (-7, "a week ago"),
            (-11, "2 weeks ago"),
            (-28, "a month ago"),
            (14, "in 2 weeks"),
        ] {
            assert_eq!(bucket(today + chrono::Duration::days(days), today), label);
        }
    }

    #[test]
    fn range_selection_uses_stable_visible_ids_and_survives_cancelled_actions() {
        let ids = ["a", "b", "c", "d"].map(|id| SessionId(id.into()));
        let mut selection = Selection::default();
        selection.click(ids[1].clone(), &ids, false, false);
        selection.click(ids[3].clone(), &ids, false, true);
        assert_eq!(selection.ids.len(), 3);
        selection.click(ids[2].clone(), &ids, true, false);
        assert_eq!(selection.ids.len(), 2);
        assert!(selection.ids.contains(&ids[1]));
        selection.click(ids[0].clone(), &ids[..1], false, true);
        assert_eq!(selection.ids, HashSet::from([ids[0].clone()]));
    }
}
