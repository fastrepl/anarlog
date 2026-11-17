use std::{collections::HashSet, sync::Arc};

use chrono::{Datelike, Utc};
use chrono_tz::Tz;
use desktop_runtime::{LibraryPage, SessionId};
use gpui::{
    Context, EventEmitter, FocusHandle, Hsla, ListAlignment, ListOffset, ListState, Pixels, Render,
    Window, div, linear_color_stop, linear_gradient, list, prelude::*, px, rgb, svg,
};

pub(super) use super::timeline::folder_label;
use super::timeline::{
    self, EventWindow, Grouping, Row, Source, Timeline, View, local_time, timestamp,
};
use crate::meeting::{
    live::{LiveState, SessionMode},
    widgets::{dancing_sticks, spinner as spinner_icon},
};
use crate::ui::theme::{RADIUS, Theme, monospace_font, theme};

const HEADING_HEIGHT: f32 = 28.;
const RED_500: u32 = 0xfb2c36;

pub struct LibraryView {
    page: Arc<LibraryPage>,
    events: EventWindow,
    view: View,
    timeline: Timeline,
    keys: Vec<RowKey>,
    selection: Selection,
    active: Option<SessionId>,
    recording: HashSet<SessionId>,
    /// The calendar event whose note is being created or found.
    opening: Option<Arc<str>>,
    scroll: ListState,
    focus: FocusHandle,
    menu: Option<NoteMenu>,
    menu_index: usize,
    use_24_hour_time: bool,
    timezone: Option<Tz>,
    show_folder: bool,
    show_tags: bool,
}

#[derive(Clone)]
struct NoteMenu {
    position: gpui::Point<gpui::Pixels>,
    ids: Arc<[SessionId]>,
    bulk: bool,
    /// An event row's tracking id and series, for its delete menu.
    event: Option<(Arc<str>, Arc<str>)>,
}

impl NoteMenu {
    fn labels(&self) -> Vec<String> {
        if let Some((_, series)) = &self.event {
            return super::timeline::event_menu_labels(!series.is_empty());
        }
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

#[derive(Clone)]
pub enum OpenNote {
    Current(SessionId),
    Window(SessionId),
    Reveal(SessionId),
    /// Deletes at once with an Undo toast.
    Delete(Arc<[SessionId]>),
    /// A multi-selection's delete, which asks first.
    DeleteSelected(Arc<[SessionId]>),
    Move(Arc<[SessionId]>),
    /// A calendar event without a note yet; the workspace creates or finds its note.
    Event(Arc<str>),
    /// Hide a calendar event or its recurring series.
    Ignore(anlg_db_app::IgnoredCalendarItemKind, Arc<str>),
    Calendar,
    /// The live item's stop control.
    StopListening,
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

/// Stable row identity, so a new note or a passing minute does not reset the scroll.
#[derive(Clone, PartialEq)]
enum RowKey {
    Spacer,
    Heading(Arc<str>),
    Session(SessionId),
    Event(Arc<str>),
    Now(u32, u32),
}

fn row_keys(timeline: &Timeline, page: &LibraryPage, events: &EventWindow) -> Vec<RowKey> {
    timeline
        .rows
        .iter()
        .map(|row| match row {
            Row::TopSpacer => RowKey::Spacer,
            Row::Heading(label) => RowKey::Heading(label.clone()),
            Row::Now { above, below } => RowKey::Now(*above as u32, *below as u32),
            Row::Item { item, .. } => match timeline.items[*item].source {
                Source::Session(index) => RowKey::Session(page.items[index].id.clone()),
                Source::Event(index) => RowKey::Event(events.events[index].id.clone()),
            },
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq)]
enum Visibility {
    Above,
    Visible,
    Below,
}

impl LibraryView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let scroll = ListState::new(0, ListAlignment::Top, px(256.));
        let entity = cx.entity().downgrade();
        // Chips, the sticky heading and the fade depend on the scroll position.
        scroll.set_scroll_handler(move |_, _, cx| {
            let _ = entity.update(cx, |_, cx| cx.notify());
        });
        Self {
            page: Arc::new(LibraryPage {
                items: Arc::from([]),
                offset: 0,
                has_more: false,
            }),
            events: EventWindow::default(),
            view: View::default(),
            timeline: Timeline::default(),
            keys: Vec::new(),
            selection: Selection::default(),
            active: None,
            recording: HashSet::new(),
            opening: None,
            scroll,
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
            self.rebuild(false, cx);
        }
    }

    pub fn set_metadata(&mut self, show_folder: bool, show_tags: bool, cx: &mut Context<Self>) {
        if (self.show_folder, self.show_tags) != (show_folder, show_tags) {
            self.show_folder = show_folder;
            self.show_tags = show_tags;
            self.scroll.remeasure_items(0..self.keys.len());
            cx.notify();
        }
    }

    pub fn view(&self) -> View {
        self.view
    }

    pub fn set_view(&mut self, view: View, cx: &mut Context<Self>) {
        if self.view != view {
            self.view = view;
            self.timeline = Timeline::default();
            self.rebuild(false, cx);
        }
    }

    pub fn set_page(&mut self, page: Arc<LibraryPage>, cx: &mut Context<Self>) {
        self.selection
            .ids
            .retain(|id| page.items.iter().any(|item| &item.id == id));
        if page.offset != self.page.offset {
            self.timeline = Timeline::default();
        }
        self.page = page;
        self.rebuild(true, cx);
    }

    pub fn set_events(&mut self, events: EventWindow, cx: &mut Context<Self>) {
        if self.events != events {
            self.events = events;
            self.rebuild(false, cx);
        }
    }

    /// Re-derives time-dependent rows and returns when to call again.
    pub fn tick(&mut self, cx: &mut Context<Self>) -> std::time::Duration {
        self.rebuild(false, cx);
        self.timeline.next_refresh()
    }

    /// The meeting the collapsed sidebar badge points at, unless it is the open note.
    pub fn has_upcoming_meeting(&self) -> bool {
        self.timeline.upcoming.as_ref().is_some_and(|upcoming| {
            match self.timeline.items[upcoming.item].source {
                Source::Session(index) => self.active.as_ref() != Some(&self.page.items[index].id),
                Source::Event(_) => true,
            }
        })
    }

    fn rebuild(&mut self, remeasure: bool, cx: &mut Context<Self>) {
        let events = self.events.clone();
        let timeline = timeline::build(
            &self.page.items,
            &events,
            self.view,
            Utc::now(),
            self.timezone,
        );
        let keys = row_keys(&timeline, &self.page, &events);
        if self.timeline.rows.is_empty() {
            self.scroll.reset(keys.len());
        } else {
            // Splice only the rows that changed, so refreshes keep the scroll position.
            let old = &self.keys;
            let prefix = old.iter().zip(&keys).take_while(|(a, b)| a == b).count();
            let suffix = old[prefix..]
                .iter()
                .rev()
                .zip(keys[prefix..].iter().rev())
                .take_while(|(a, b)| a == b)
                .count();
            if prefix + suffix != old.len() || old.len() != keys.len() {
                self.scroll
                    .splice(prefix..old.len() - suffix, keys.len() - prefix - suffix);
            }
            if remeasure {
                self.scroll.remeasure_items(0..keys.len());
            }
        }
        self.keys = keys;
        self.timeline = timeline;
        cx.notify();
    }

    pub fn set_active(&mut self, active: Option<SessionId>, cx: &mut Context<Self>) {
        if self.active != active {
            self.active = active;
            cx.notify();
        }
    }

    pub fn opening(&self) -> bool {
        self.opening.is_some()
    }

    pub fn set_opening(&mut self, event: Option<Arc<str>>, cx: &mut Context<Self>) {
        if self.opening != event {
            self.opening = event;
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
        if let Some((tracking, series)) = menu.event {
            use anlg_db_app::IgnoredCalendarItemKind;
            match index {
                0 => cx.emit(OpenNote::Ignore(IgnoredCalendarItemKind::Events, tracking)),
                1 => cx.emit(OpenNote::Ignore(IgnoredCalendarItemKind::Series, series)),
                _ => {}
            }
        } else if !menu.ids.is_empty() {
            match (menu.bulk, index) {
                (false, 0) => cx.emit(OpenNote::Window(menu.ids[0].clone())),
                (false, 1) => cx.emit(OpenNote::Reveal(menu.ids[0].clone())),
                (false, 2) => cx.emit(OpenNote::Delete(menu.ids)),
                (true, 0) => cx.emit(OpenNote::DeleteSelected(menu.ids)),
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

    /// Session ids in timeline order, for keyboard and range selection.
    fn visible_sessions(&self) -> Vec<SessionId> {
        self.timeline
            .rows
            .iter()
            .filter_map(|row| match row {
                Row::Item { item, .. } => match self.timeline.items[*item].source {
                    Source::Session(index) => Some(self.page.items[index].id.clone()),
                    Source::Event(_) => None,
                },
                _ => None,
            })
            .collect()
    }

    fn visibility(&self, row: usize) -> Visibility {
        let viewport = self.scroll.viewport_bounds();
        if viewport.size.height <= px(0.) {
            return Visibility::Visible;
        }
        if row < self.scroll.logical_scroll_top().item_ix {
            return Visibility::Above;
        }
        match self.scroll.bounds_for_item(row) {
            Some(bounds) if bounds.bottom() <= viewport.top() => Visibility::Above,
            Some(bounds) if bounds.top() >= viewport.bottom() => Visibility::Below,
            Some(_) => Visibility::Visible,
            None => Visibility::Below,
        }
    }

    /// Scrolls a row so its middle sits at `ratio` of the viewport height.
    fn scroll_row_to(&self, row: usize, ratio: f32) {
        let height = self.scroll.viewport_bounds().size.height;
        let row_height = self
            .scroll
            .bounds_for_item(row)
            .map_or(px(0.), |bounds| bounds.size.height);
        self.scroll.scroll_to(ListOffset {
            item_ix: row,
            offset_in_item: px(0.),
        });
        self.scroll.scroll_by(row_height * 0.5 - height * ratio);
    }

    fn scroll_offset(&self) -> (Pixels, Pixels) {
        let offset = -self.scroll.scroll_px_offset_for_scrollbar().y;
        (offset, self.scroll.max_offset_for_scrollbar().y - offset)
    }

    fn render_row(
        &mut self,
        row: usize,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let colors = theme(window);
        let date_grouping = self.view.grouping == Grouping::Date;
        match self.timeline.rows.get(row).cloned() {
            None => div().into_any_element(),
            Some(Row::TopSpacer) => div().h(px(36.)).into_any_element(),
            Some(Row::Heading(label)) => heading(label, colors).into_any_element(),
            Some(Row::Now { above, below }) => div()
                .pt(px(above))
                .pb(px(below))
                .child(self.now_line(None, cx))
                .into_any_element(),
            Some(Row::Item { item, indicator }) => {
                let element = self.render_item(item, date_grouping, colors, cx);
                match indicator {
                    Some(position) => div()
                        .relative()
                        .child(element)
                        .child(self.now_line(Some(position), cx))
                        .into_any_element(),
                    None => element,
                }
            }
        }
    }

    /// The red current-time line; `inside` positions it over an ongoing item. A live recording
    /// in the timeline replaces the line with an invisible anchor, as in the shipping app.
    fn now_line(&self, inside: Option<f32>, cx: &Context<Self>) -> gpui::Div {
        let live = self
            .page
            .items
            .iter()
            .any(|item| self.recording.contains(&item.id));
        let label = self.timeline.now.map(|now| {
            local_time(now, self.timezone)
                .format(if self.use_24_hour_time {
                    "%H:%M"
                } else {
                    "%-I:%M %p"
                })
                .to_string()
        });
        div()
            .h(px(1.))
            .w_full()
            .when_some(inside, |view, position| {
                view.absolute().left_0().top(gpui::relative(position))
            })
            .when(!live, |view| {
                view.child(
                    div()
                        .id("current-time")
                        .group("current-time")
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(px(-10.))
                        .h(px(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .absolute()
                                .left_0()
                                .right_0()
                                .top(px(10.))
                                .h(px(1.))
                                .bg(Hsla::from(rgb(RED_500)).opacity(0.85)),
                        )
                        .when_some(label, |view, label| {
                            view.child(
                                div()
                                    .rounded_full()
                                    .border_1()
                                    .border_color(rgb(RED_500))
                                    .bg(rgb(RED_500))
                                    .px_2()
                                    .py(px(2.))
                                    .font_family(monospace_font(cx))
                                    .text_size(px(11.))
                                    .line_height(px(14.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(gpui::white())
                                    .shadow_xs()
                                    .opacity(0.)
                                    .group_hover("current-time", |style| style.opacity(1.))
                                    .child(label),
                            )
                        }),
                )
            })
    }

    fn render_item(
        &mut self,
        item: usize,
        date_grouping: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let entry = self.timeline.items[item].clone();
        let now = self.timeline.now.unwrap_or_else(Utc::now);
        let today = local_time(now, self.timezone).date_naive();
        let time = entry
            .start
            .map(|start| {
                timestamp(
                    local_time(start, self.timezone),
                    entry.precision,
                    today.year(),
                    self.use_24_hour_time,
                )
            })
            .unwrap_or_default();
        let muted = entry.start.is_some_and(|start| start > now);
        let upcoming = self
            .timeline
            .upcoming
            .clone()
            .filter(|upcoming| upcoming.item == item);
        let event_opening = match entry.source {
            Source::Event(index) => self.opening.as_ref() == Some(&self.events.events[index].id),
            Source::Session(_) => false,
        };
        let event_ids = match entry.source {
            Source::Event(index) => Some(&self.events.events[index])
                .filter(|event| !event.tracking.is_empty())
                .map(|event| (event.tracking.clone(), event.series.clone())),
            Source::Session(_) => None,
        };
        let (id, title, folder, tags, session) = match entry.source {
            Source::Session(index) => {
                let session = &self.page.items[index];
                (
                    gpui::SharedString::from(session.id.0.clone()),
                    session.title.clone(),
                    if self.show_folder && date_grouping {
                        folder_label(&session.folder_path)
                    } else {
                        String::new()
                    },
                    if self.show_tags {
                        session.tag_line.clone()
                    } else {
                        Arc::default()
                    },
                    Some(session.id.clone()),
                )
            }
            Source::Event(index) => {
                let event = &self.events.events[index];
                (
                    gpui::SharedString::from(format!("event-{}", event.id)),
                    event.title.clone(),
                    String::new(),
                    Arc::default(),
                    None,
                )
            }
        };
        let mode = session
            .as_ref()
            .map(|id| LiveState::mode(cx, id))
            .unwrap_or_default();
        // Shipping turns only an actively listening note red; finalizing shows a spinner.
        let live = match LiveState::global(cx) {
            Some(_) => mode == SessionMode::Active,
            None => session
                .as_ref()
                .is_some_and(|id| self.recording.contains(id)),
        };
        let amplitude = session
            .as_ref()
            .zip(LiveState::global(cx))
            .and_then(|(id, state)| state.read(cx).session(id).map(|live| live.amplitude))
            .unwrap_or_default()
            .max(0.25);
        let selected = session
            .as_ref()
            .is_some_and(|id| self.selection.ids.contains(id) || self.active.as_ref() == Some(id));
        let spinner = event_opening || !selected && !live && mode == SessionMode::Finalizing;
        let extra_rows = usize::from(!folder.is_empty()) + usize::from(!tags.is_empty());
        // Shipping's stylesheet never generates the item's `bg-destructive/8`, `bg-destructive/20`
        // and `text-destructive-foreground/65` classes, so upcoming items have no tint or gauge
        // track and live metadata stays full white. Match what ships.
        let secondary = if live {
            colors.destructive_foreground
        } else {
            colors.muted_foreground
        };
        let content = div()
            .id(id)
            .w_full()
            .min_w_0()
            .h(px(54. + 18. * extra_rows as f32))
            .flex()
            .flex_col()
            .gap(px(2.))
            .px_3()
            .py_2()
            .when(upcoming.is_some(), |view| view.pl_4())
            .when(live, |view| view.pr_10())
            .rounded(px(8.))
            .cursor_pointer()
            .overflow_hidden()
            .when(!selected && !live, |view| {
                view.hover(|style| style.bg(colors.accent.opacity(0.5)))
            })
            .when(selected, |view| view.bg(colors.accent))
            .when(live, |view| {
                view.bg(colors.destructive)
                    .text_color(colors.destructive_foreground)
                    .hover(|style| style.bg(colors.destructive.opacity(0.9)))
            })
            .when(muted && !live && upcoming.is_none(), |view| {
                view.opacity(0.65)
            })
            .when_some(session.clone(), |view, session| {
                view.on_mouse_down(
                    gpui::MouseButton::Right,
                    cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                        this.focus.focus(window, cx);
                        let ids = this.selected_ids();
                        let bulk = !ids.is_empty();
                        this.menu = Some(NoteMenu {
                            position: event.position,
                            ids: if bulk {
                                ids
                            } else {
                                vec![session.clone()].into()
                            },
                            bulk,
                            event: None,
                        });
                        this.menu_index = 0;
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
            })
            .when_some(event_ids, |view, event_ids| {
                view.on_mouse_down(
                    gpui::MouseButton::Right,
                    cx.listener(move |this, event: &gpui::MouseDownEvent, window, cx| {
                        this.focus.focus(window, cx);
                        this.menu = Some(NoteMenu {
                            position: event.position,
                            ids: Arc::from([]),
                            bulk: false,
                            event: Some(event_ids.clone()),
                        });
                        this.menu_index = 0;
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
            })
            .on_click(cx.listener({
                let source = entry.source;
                move |this, event: &gpui::ClickEvent, window, cx| {
                    this.focus.focus(window, cx);
                    let id = match source {
                        Source::Session(index) => this.page.items[index].id.clone(),
                        Source::Event(index) => {
                            if event.click_count() == 1 {
                                cx.emit(OpenNote::Event(this.events.events[index].id.clone()));
                            }
                            return;
                        }
                    };
                    let modifiers = event.modifiers();
                    let visible = this.visible_sessions();
                    this.selection.click(
                        id.clone(),
                        &visible,
                        modifiers.secondary(cx),
                        modifiers.shift,
                    );
                    if !modifiers.secondary(cx) && !modifiers.shift {
                        if event.click_count() == 2 {
                            cx.emit(OpenNote::Window(id));
                        } else if event.click_count() == 1 {
                            cx.emit(OpenNote::Current(id));
                        }
                    }
                    cx.notify();
                }
            }))
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
                        .text_color(secondary)
                        .child(
                            svg()
                                .path("workspace/Folder01Icon.svg")
                                .size(px(12.))
                                .flex_shrink_0()
                                .text_color(secondary),
                        )
                        .child(div().min_w_0().truncate().child(folder)),
                )
            })
            .child(
                div()
                    .text_sm()
                    .line_height(px(20.))
                    .truncate()
                    .child(if title.is_empty() {
                        "Untitled".into()
                    } else {
                        gpui::SharedString::from(title)
                    }),
            )
            .when(!time.is_empty(), |view| {
                view.child(
                    div()
                        .font_family(monospace_font(cx))
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(secondary)
                        .child(time),
                )
            })
            .when(!tags.is_empty(), |view| {
                view.child(
                    div()
                        .w_full()
                        .min_w_0()
                        .flex_shrink_0()
                        .h(px(16.))
                        .text_size(px(11.))
                        .line_height(px(16.))
                        .text_color(secondary)
                        .line_clamp(1)
                        .text_ellipsis()
                        .child(gpui::SharedString::from(tags)),
                )
            });
        div()
            .w_full()
            .relative()
            .group("sidebar-live-item")
            .child(content)
            .when(spinner, |view| {
                view.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right_3()
                        .flex()
                        .items_center()
                        .child(spinner_icon(
                            "sidebar-finalizing",
                            14.,
                            colors.muted_foreground,
                        )),
                )
            })
            .when(live, |view| {
                let white = gpui::white();
                view.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .right_3()
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .id("sidebar-live-stop")
                                .size_5()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(RADIUS - 4.))
                                .text_color(white.opacity(0.8))
                                .hover(|style| style.bg(white.opacity(0.15)).text_color(white))
                                .cursor_pointer()
                                .on_click(cx.listener(|_, _, _, cx| {
                                    cx.stop_propagation();
                                    cx.emit(OpenNote::StopListening);
                                }))
                                .child(
                                    div()
                                        .group_hover("sidebar-live-item", |style| style.invisible())
                                        .child(dancing_sticks(
                                            "sidebar-live",
                                            amplitude,
                                            white.opacity(0.8),
                                            (13., 14.),
                                            (2., 2.),
                                        )),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .invisible()
                                        .group_hover("sidebar-live-item", |style| style.visible())
                                        .child(
                                            svg()
                                                .path("meeting/SquareIcon.svg")
                                                .size(px(10.))
                                                .text_color(white),
                                        ),
                                ),
                        ),
                )
            })
            .when_some(upcoming.filter(|_| !live), |view, upcoming| {
                view.child(
                    div()
                        .absolute()
                        .top_2()
                        .bottom_2()
                        .left(px(6.))
                        .w(px(2.))
                        .rounded_full()
                        .overflow_hidden()
                        .child(
                            div()
                                .absolute()
                                .bottom_0()
                                .left_0()
                                .w_full()
                                .h(gpui::relative(upcoming.progress))
                                .rounded_full()
                                .bg(colors.destructive),
                        ),
                )
            })
            .into_any_element()
    }

    /// The shipping chip stack above the timeline and the "Now" chip below it.
    fn chips(
        &self,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> (Option<gpui::Div>, Option<gpui::Div>) {
        if self.view.grouping != Grouping::Date {
            return (None, None);
        }
        let (offset, _) = self.scroll_offset();
        let open_calendar = self.timeline.later && offset <= px(12.);
        let upcoming = self.timeline.upcoming.as_ref().and_then(|upcoming| {
            let row = self.timeline.item_row(upcoming.item)?;
            (self.visibility(row) != Visibility::Visible).then(|| (row, upcoming.label.clone()))
        });
        let now = self
            .timeline
            .now_row()
            .map(|row| (row, self.visibility(row)))
            .filter(|_| upcoming.is_none());
        let bottom = now
            .filter(|(_, visibility)| *visibility == Visibility::Below)
            .map(|(row, _)| {
                div()
                    .absolute()
                    .bottom_2()
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(self.now_chip(row, false, colors, cx))
            });
        let now = now.filter(|(_, visibility)| *visibility == Visibility::Above);
        if !open_calendar && upcoming.is_none() && now.is_none() {
            return (None, bottom);
        }
        let top = Some(
            div()
                .absolute()
                .top_1()
                .left_0()
                .right_0()
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .when(open_calendar, |view| {
                    view.child(
                        chip("open-calendar", colors)
                            .text_color(colors.muted_foreground)
                            .font_weight(crate::ui::theme::MEDIUM)
                            .shadow_xs()
                            .hover(|style| style.bg(colors.accent).text_color(colors.foreground))
                            .child(
                                chip_icon("workspace/Calendar03Icon.svg", 12.)
                                    .text_color(colors.muted_foreground)
                                    .group_hover("open-calendar", |style| {
                                        style.text_color(colors.foreground)
                                    }),
                            )
                            .child("Open calendar")
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(OpenNote::Calendar))),
                    )
                })
                .when_some(upcoming, |view, (row, label)| {
                    view.child(
                        chip("upcoming-meeting", colors)
                            .w(px(112.))
                            .justify_center()
                            .border_color(colors.destructive)
                            .bg(colors.destructive)
                            .text_color(colors.destructive_foreground)
                            .font_weight(crate::ui::theme::MEDIUM)
                            .shadow_md()
                            .child(
                                chip_icon("workspace/ArrowUp02IconBold.svg", 12.)
                                    .text_color(colors.destructive_foreground),
                            )
                            .child(label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.scroll_row_to(row, 0.45);
                                cx.notify();
                            })),
                    )
                })
                .when_some(now, |view, (row, _)| {
                    view.child(self.now_chip(row, true, colors, cx))
                }),
        );
        (top, bottom)
    }

    fn now_chip(
        &self,
        row: usize,
        up: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let arrow = |path| chip_icon(path, 12.).text_color(colors.foreground);
        chip(if up { "now-up" } else { "now-down" }, colors)
            .text_color(colors.foreground)
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .shadow_md()
            .hover(|style| style.bg(colors.accent))
            .when(up, |view| view.child(arrow("workspace/ArrowUp02Icon.svg")))
            .child(chip_icon("workspace/Sun01Icon.svg", 13.).text_color(rgb(0xfac800)))
            .child("Now")
            .when(!up, |view| {
                view.child(arrow("workspace/ArrowDown02Icon.svg"))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.scroll_row_to(row, 0.5);
                cx.notify();
            }))
    }

    fn sticky_heading(&self, colors: Theme) -> Option<gpui::Div> {
        let top = self.scroll.logical_scroll_top();
        if top.item_ix == 0 && top.offset_in_item <= px(0.) {
            return None;
        }
        let (index, label) = self.timeline.heading_for(top.item_ix)?;
        if index == top.item_ix && top.offset_in_item <= px(0.) {
            return None;
        }
        let viewport = self.scroll.viewport_bounds();
        // The next bucket's heading pushes this one out, like CSS sticky inside its bucket.
        let shift = self
            .timeline
            .rows
            .get(top.item_ix + 1..)
            .unwrap_or_default()
            .iter()
            .position(|row| matches!(row, Row::Heading(_)))
            .and_then(|next| self.scroll.bounds_for_item(top.item_ix + 1 + next))
            .map_or(px(0.), |bounds| {
                (bounds.top() - viewport.top() - px(HEADING_HEIGHT)).min(px(0.))
            });
        Some(
            div()
                .absolute()
                .top(shift)
                .left_0()
                .right_0()
                .child(heading(label.clone(), colors)),
        )
    }
}

fn heading(label: Arc<str>, colors: Theme) -> gpui::Div {
    div()
        .h(px(HEADING_HEIGHT))
        .pr_1()
        .pb_1()
        .pl_3()
        .bg(colors.background)
        .text_color(colors.foreground)
        .text_base()
        .line_height(px(24.))
        .font_weight(gpui::FontWeight::BOLD)
        .child(gpui::SharedString::from(label))
}

fn chip(id: &'static str, colors: Theme) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .group(id)
        .h_6()
        .flex()
        .items_center()
        .gap_1()
        .px(px(10.))
        .rounded(px(RADIUS))
        .border_1()
        .border_color(colors.border)
        .bg(colors.card)
        .text_xs()
        .line_height(px(16.))
        .cursor_pointer()
}

fn chip_icon(path: &'static str, size: f32) -> gpui::Svg {
    svg().path(path).size(px(size)).flex_shrink_0()
}

fn fade(color: Hsla) -> gpui::Div {
    div()
        .absolute()
        .left_0()
        .right_0()
        .bottom_0()
        .h(px(28.))
        .bg(linear_gradient(
            180.,
            linear_color_stop(color.opacity(0.), 0.),
            linear_color_stop(color, 1.),
        ))
}

impl Render for LibraryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let (_, remaining) = self.scroll_offset();
        let (chips, bottom_now) = self.chips(colors, cx);
        let sticky = self.sticky_heading(colors);
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
                        cx.emit(OpenNote::DeleteSelected(ids));
                    }
                    cx.stop_propagation();
                    return;
                }
                if matches!(event.keystroke.key.as_str(), "up" | "down") {
                    let visible = this.visible_sessions();
                    if visible.is_empty() {
                        return;
                    }
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
                    cx.processor(move |this, row: usize, window, cx| {
                        this.render_row(row, window, cx)
                    }),
                )
                .size_full(),
            )
            .when(remaining > px(12.), |view| {
                view.child(fade(colors.background))
            })
            .when_some(sticky, |view, sticky| view.child(sticky))
            .when_some(chips, |view, chips| view.child(chips))
            .when_some(bottom_now, |view, chip| view.child(chip))
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
