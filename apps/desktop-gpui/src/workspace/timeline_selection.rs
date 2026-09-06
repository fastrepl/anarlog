//! The timeline's multi-selection, context menus, and bulk delete:
//! `store/zustand/timeline-selection.ts`, the row menus in
//! `sidebar/timeline/item.tsx`, and the list menu, shortcuts, and
//! `DestructiveConfirmationDialog` in `sidebar/timeline/index.tsx`.

use gpui::{
    AnyElement, ClickEvent, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};

use super::Workspace;
use super::menu::{Align, Entry, MenuSpec, Trailing};
use crate::theme::alpha;
use crate::timeline::ItemKind;
use crate::ui::TailwindText;

/// Item keys as the store spells them: `session-<id>` / `event-<id>`.
pub(crate) fn item_key(kind: ItemKind, id: &str) -> String {
    match kind {
        ItemKind::Session => format!("session-{id}"),
        ItemKind::Event => format!("event-{id}"),
    }
}

pub(crate) fn is_session_key(key: &str) -> bool {
    key.starts_with("session-")
}

fn session_id_of(key: &str) -> Option<&str> {
    key.strip_prefix("session-")
}

/// `useTimelineSelection`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct TimelineSelection {
    pub selected: Vec<String>,
    pub anchor: Option<String>,
}

impl TimelineSelection {
    pub fn is_selected(&self, key: &str) -> bool {
        self.selected.iter().any(|selected| selected == key)
    }

    pub fn has_selection(&self) -> bool {
        !self.selected.is_empty()
    }

    pub fn selected_session_ids(&self) -> Vec<String> {
        self.selected
            .iter()
            .filter_map(|key| session_id_of(key))
            .map(str::to_string)
            .collect()
    }

    /// `setAnchor`: a plain click restarts the selection from this row.
    pub fn set_anchor(&mut self, key: &str) {
        self.anchor = Some(key.to_string());
        self.selected.clear();
    }

    /// `toggleSelect`: cmd/ctrl-click. The first toggle folds the anchor in
    /// so the previously clicked row joins the selection.
    pub fn toggle(&mut self, key: &str) {
        if let Some(index) = self.selected.iter().position(|selected| selected == key) {
            self.selected.remove(index);
            if self.selected.is_empty() {
                self.anchor = None;
            }
            return;
        }
        if self.selected.is_empty()
            && let Some(anchor) = self.anchor.clone()
            && anchor != key
        {
            self.selected.push(anchor);
        }
        self.selected.push(key.to_string());
        if self.anchor.is_none() {
            self.anchor = Some(key.to_string());
        }
    }

    /// `selectRange`: shift-click selects from the anchor to this row in
    /// the flat item order, or just this row without an anchor.
    pub fn select_range(&mut self, flat_keys: &[String], key: &str) {
        let Some(anchor) = self.anchor.as_deref() else {
            self.anchor = Some(key.to_string());
            self.selected = vec![key.to_string()];
            return;
        };
        let anchor_index = flat_keys.iter().position(|item| item == anchor);
        let target_index = flat_keys.iter().position(|item| item == key);
        match (anchor_index, target_index) {
            (Some(a), Some(b)) => {
                let (start, end) = (a.min(b), a.max(b));
                self.selected = flat_keys[start..=end].to_vec();
            }
            _ => {
                self.anchor = Some(key.to_string());
                self.selected = vec![key.to_string()];
            }
        }
    }

    /// `selectAll`: keeps the anchor when it is among the ids.
    pub fn select_all(&mut self, keys: Vec<String>) {
        self.anchor = match self.anchor.take() {
            Some(anchor) if keys.contains(&anchor) => Some(anchor),
            _ => keys.first().cloned(),
        };
        self.selected = keys;
    }

    pub fn clear(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }

    /// `hasSidebarNoteSelectionContext`: mod+A only selects notes when the
    /// open note was anchored from the sidebar or a note is already selected.
    pub fn has_note_context(&self, open_session_id: Option<&str>) -> bool {
        let current_key = open_session_id.map(|id| item_key(ItemKind::Session, id));
        self.anchor.is_some() && self.anchor == current_key
            || self.selected.iter().any(|key| is_session_key(key))
    }
}

/// Which context menu is open over the timeline and where it was requested.
pub(crate) enum TimelineMenu {
    /// A row's own menu (`InteractiveButton contextMenu`), suppressed while a
    /// multi-selection exists.
    Row {
        kind: ItemKind,
        id: String,
        position: gpui::Point<gpui::Pixels>,
    },
    /// The list's menu: `Delete Selected (N)` with a selection, otherwise the
    /// `Show Deleted Events` / `Hide Deleted Events` toggle.
    List { position: gpui::Point<gpui::Pixels> },
}

impl Workspace {
    /// Keys of every listed item in bucket order (`getFlatItemKeys`).
    fn flat_timeline_keys(&self) -> Vec<String> {
        let super::Sessions::Ready(timeline) = &self.sessions else {
            return Vec::new();
        };
        timeline
            .buckets
            .iter()
            .flat_map(|bucket| bucket.items.iter())
            .map(|item| item_key(item.kind, &item.id))
            .collect()
    }

    fn flat_session_keys(&self) -> Vec<String> {
        self.flat_timeline_keys()
            .into_iter()
            .filter(|key| is_session_key(key))
            .collect()
    }

    /// `InteractiveButton`: shift-click extends the range, cmd/ctrl-click
    /// toggles, a plain click anchors and opens.
    pub(super) fn timeline_row_click(
        &mut self,
        kind: ItemKind,
        id: String,
        event: &ClickEvent,
        cx: &mut Context<Self>,
    ) {
        let key = item_key(kind, &id);
        let modifiers = event.modifiers();
        if modifiers.shift {
            let keys = self.flat_timeline_keys();
            self.timeline_selection.select_range(&keys, &key);
            cx.notify();
            return;
        }
        if modifiers.platform || modifiers.control {
            self.timeline_selection.toggle(&key);
            cx.notify();
            return;
        }
        self.timeline_selection.set_anchor(&key);
        match kind {
            ItemKind::Event => self.open_event(id, cx),
            ItemKind::Session => self.select(id, cx),
        }
    }

    pub(super) fn open_timeline_menu(&mut self, menu: TimelineMenu, cx: &mut Context<Self>) {
        // A row press while a selection exists shows the list menu instead,
        // like `contextMenu={hasSelection ? undefined : contextMenu}`.
        self.timeline_menu = Some(match menu {
            TimelineMenu::Row { position, .. } if self.timeline_selection.has_selection() => {
                TimelineMenu::List { position }
            }
            other => other,
        });
        cx.notify();
    }

    /// `shouldClearTimelineSelectionOnPointerDown`: a press outside the
    /// timeline (and its dialog) drops the selection.
    pub(super) fn clear_timeline_selection(&mut self, cx: &mut Context<Self>) {
        if self.timeline_selection.has_selection() || self.timeline_selection.anchor.is_some() {
            self.timeline_selection.clear();
            cx.notify();
        }
    }

    /// mod+A with sidebar note context selects every listed note.
    pub(super) fn select_all_timeline_notes(&mut self, cx: &mut Context<Self>) -> bool {
        let keys = self.flat_session_keys();
        if keys.is_empty()
            || !self
                .timeline_selection
                .has_note_context(self.selected.as_deref())
        {
            return false;
        }
        self.timeline_selection.select_all(keys);
        cx.notify();
        true
    }

    /// Backspace / Delete with selected notes asks for confirmation.
    pub(super) fn request_delete_selected(&mut self, cx: &mut Context<Self>) -> bool {
        let ids = self.timeline_selection.selected_session_ids();
        if ids.is_empty() {
            return false;
        }
        self.pending_delete_selected = ids;
        cx.notify();
        true
    }

    fn cancel_delete_selected(&mut self, cx: &mut Context<Self>) {
        self.pending_delete_selected.clear();
        cx.notify();
    }

    /// `handleConfirmDeleteSelected`: one batch id for more than one note,
    /// then the selection clears.
    fn confirm_delete_selected(&mut self, cx: &mut Context<Self>) {
        let ids = std::mem::take(&mut self.pending_delete_selected);
        let batch_id = (ids.len() > 1).then(|| uuid::Uuid::new_v4().to_string());
        for id in ids {
            self.delete_session_by_id(id, batch_id.clone(), cx);
        }
        self.timeline_selection.clear();
        cx.notify();
    }

    pub(super) fn toggle_show_ignored_events(&mut self, cx: &mut Context<Self>) {
        self.show_ignored_events = !self.show_ignored_events;
        self.rebuild_timeline(cx);
    }

    fn session_menu_entries(&self, id: String) -> Vec<Entry> {
        let plain = |label: &'static str, on_select: super::menu::Select| Entry::Item {
            icon: None,
            dim_icon: false,
            label: label.into(),
            trailing: Trailing::None,
            destructive: false,
            on_select: Some(on_select),
            submenu: None,
        };
        let open_id = id.clone();
        let show_id = id.clone();
        // `Lock Note` is offered only with `authAvailable`, which the shell
        // treats as unresolved.
        vec![
            plain(
                "Open in New Window",
                Box::new(move |this, _, cx| this.open_note_window(open_id.clone(), cx)),
            ),
            plain(
                if cfg!(target_os = "macos") {
                    "Show in Finder"
                } else {
                    "Show in folder"
                },
                Box::new(move |this, _, cx| {
                    let path = this.store.session_dir(&show_id);
                    cx.open_url(&format!("file://{}", path.display()));
                }),
            ),
            Entry::Separator,
            plain(
                "Delete Note",
                Box::new(move |this, _, cx| this.delete_session_by_id(id.clone(), None, cx)),
            ),
        ]
    }

    fn event_menu_entries(&self, id: &str) -> Option<Vec<Entry>> {
        let event = self.event_rows.iter().find(|event| event.id == id)?;
        let tracking_id = event.tracking_id_event.clone();
        let series_id = Some(event.recurrence_series_id.clone()).filter(|id| !id.is_empty());
        let ignored = self
            .sessions_item(ItemKind::Event, id)
            .is_some_and(|item| item.ignored);
        let plain = |label: &'static str, on_select: super::menu::Select| Entry::Item {
            icon: None,
            dim_icon: false,
            label: label.into(),
            trailing: Trailing::None,
            destructive: false,
            on_select: Some(on_select),
            submenu: None,
        };
        let mut entries = Vec::new();
        if ignored {
            let unignore_id = tracking_id.clone();
            entries.push(plain(
                if series_id.is_some() {
                    "Show This Event"
                } else {
                    "Show Event"
                },
                Box::new(move |this, _, cx| {
                    if !unignore_id.is_empty() {
                        this.unignore_calendar_entry(
                            "ignored_events",
                            "tracking_id",
                            &unignore_id,
                            cx,
                        );
                    }
                }),
            ));
            if let Some(series_id) = series_id {
                entries.push(plain(
                    "Show All Recurring Events",
                    Box::new(move |this, _, cx| {
                        this.unignore_calendar_entry(
                            "ignored_recurring_series",
                            "id",
                            &series_id,
                            cx,
                        );
                    }),
                ));
            }
            return Some(entries);
        }
        let ignore_id = tracking_id;
        entries.push(plain(
            if series_id.is_some() {
                "Delete This Event"
            } else {
                "Delete Event"
            },
            Box::new(move |this, _, cx| {
                if !ignore_id.is_empty() {
                    this.ignore_calendar_entry("ignored_events", "tracking_id", &ignore_id, cx);
                }
            }),
        ));
        if let Some(series_id) = series_id {
            entries.push(plain(
                "Delete All Recurring Events",
                Box::new(move |this, _, cx| {
                    this.ignore_calendar_entry("ignored_recurring_series", "id", &series_id, cx);
                }),
            ));
        }
        Some(entries)
    }

    fn sessions_item(&self, kind: ItemKind, id: &str) -> Option<&crate::timeline::Item> {
        let super::Sessions::Ready(timeline) = &self.sessions else {
            return None;
        };
        timeline
            .buckets
            .iter()
            .flat_map(|bucket| bucket.items.iter())
            .find(|item| item.kind == kind && item.id == id)
    }

    fn list_menu_entries(&self) -> Vec<Entry> {
        let plain = |label: String, on_select: super::menu::Select| Entry::Item {
            icon: None,
            dim_icon: false,
            label: label.into(),
            trailing: Trailing::None,
            destructive: false,
            on_select: Some(on_select),
            submenu: None,
        };
        if self.timeline_selection.has_selection() {
            let count = self.timeline_selection.selected_session_ids().len();
            // `disabled: sessionCount === 0`: an events-only selection keeps
            // the item but it does nothing.
            return vec![plain(
                format!("Delete Selected ({count})"),
                Box::new(|this, _, cx| {
                    this.request_delete_selected(cx);
                }),
            )];
        }
        vec![plain(
            if self.show_ignored_events {
                "Hide Deleted Events".to_string()
            } else {
                "Show Deleted Events".to_string()
            },
            Box::new(|this, _, cx| this.toggle_show_ignored_events(cx)),
        )]
    }

    pub(super) fn render_timeline_context_menu(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let menu = self.timeline_menu.as_ref()?;
        let (entries, position) = match menu {
            TimelineMenu::Row {
                kind: ItemKind::Session,
                id,
                position,
            } => (self.session_menu_entries(id.clone()), *position),
            TimelineMenu::Row {
                kind: ItemKind::Event,
                id,
                position,
            } => (self.event_menu_entries(id)?, *position),
            TimelineMenu::List { position } => (self.list_menu_entries(), *position),
        };
        let spec = MenuSpec {
            id: "timeline-context-menu",
            width: 224.0,
            entries,
            open_sub: None,
            on_hover_sub: |_, _, _| {},
            on_close: |this, cx| {
                this.timeline_menu = None;
                cx.notify();
            },
        };
        Some(self.render_app_menu(spec, position, Align::Start, window, cx))
    }

    /// `DestructiveConfirmationDialog` for the selection: `Delete 1 selected
    /// note?` / `Delete N selected notes?`, `You can undo this action for a
    /// short time.`, Cancel / Delete.
    pub(super) fn render_delete_selected_dialog(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let count = self.pending_delete_selected.len();
        if count == 0 {
            return None;
        }
        let theme = self.theme;
        let title = if count == 1 {
            "Delete 1 selected note?".to_string()
        } else {
            format!("Delete {count} selected notes?")
        };
        let button = |id: &'static str, label: &'static str, destructive: bool| {
            div()
                .id(id)
                .flex()
                .h(px(32.0))
                .items_center()
                .justify_center()
                .px_4()
                .rounded(px(8.0))
                .when(destructive, |button| button.bg(theme.destructive))
                .when(!destructive, |button| {
                    button
                        .bg(alpha(theme.background, 0.5))
                        .border_1()
                        .border_color(alpha(theme.border, 0.7))
                })
                .tw_text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(if destructive {
                    gpui::rgb(0xffffff)
                } else {
                    theme.foreground
                })
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(label)
        };
        Some(
            gpui::deferred(
                div()
                    .id("delete-selected-overlay")
                    .occlude()
                    .absolute()
                    .inset_0()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui::hsla(0.0, 0.0, 0.0, 0.4))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseDownEvent, _, cx| {
                            this.cancel_delete_selected(cx);
                        }),
                    )
                    .child(
                        div()
                            .id("delete-selected-card")
                            .w(px(320.0))
                            .flex()
                            .flex_col()
                            .gap_4()
                            .rounded(px(26.0))
                            .p_5()
                            .border_1()
                            .border_color(alpha(theme.border, 0.45))
                            .bg(alpha(theme.card, 0.94))
                            .shadow(vec![gpui::BoxShadow {
                                color: gpui::hsla(0.0, 0.0, 0.0, 0.32),
                                offset: gpui::point(px(0.0), px(24.0)),
                                blur_radius: px(70.0),
                                spread_radius: px(0.0),
                            }])
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap_2()
                                    .text_center()
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .line_height(px(20.0))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(theme.foreground)
                                            .child(gpui::SharedString::from(title)),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .text_size(px(13.0))
                                            .line_height(px(17.0))
                                            .text_color(theme.foreground)
                                            .child("You can undo this action for a short time."),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(div().flex_1().child(
                                        button("delete-selected-cancel", "Cancel", false).on_click(
                                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.cancel_delete_selected(cx);
                                            }),
                                        ),
                                    ))
                                    .child(div().flex_1().child(
                                        button("delete-selected-confirm", "Delete", true).on_click(
                                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.confirm_delete_selected(cx);
                                            }),
                                        ),
                                    )),
                            ),
                    ),
            )
            .with_priority(5)
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn toggle_folds_the_anchor_into_the_first_selection() {
        let mut selection = TimelineSelection::default();
        selection.set_anchor("session-a");
        selection.toggle("session-b");
        assert_eq!(selection.selected, keys(&["session-a", "session-b"]));
        assert_eq!(selection.anchor.as_deref(), Some("session-a"));

        selection.toggle("session-a");
        assert_eq!(selection.selected, keys(&["session-b"]));
        selection.toggle("session-b");
        assert!(selection.selected.is_empty());
        assert_eq!(selection.anchor, None);
    }

    #[test]
    fn toggling_the_anchor_itself_selects_only_it() {
        let mut selection = TimelineSelection::default();
        selection.set_anchor("session-a");
        selection.toggle("session-a");
        assert_eq!(selection.selected, keys(&["session-a"]));
    }

    #[test]
    fn range_runs_between_anchor_and_target_in_flat_order() {
        let flat = keys(&["event-x", "session-a", "session-b", "session-c"]);
        let mut selection = TimelineSelection::default();
        selection.select_range(&flat, "session-b");
        assert_eq!(selection.selected, keys(&["session-b"]));
        assert_eq!(selection.anchor.as_deref(), Some("session-b"));

        selection.set_anchor("session-c");
        selection.select_range(&flat, "event-x");
        assert_eq!(
            selection.selected,
            keys(&["event-x", "session-a", "session-b", "session-c"])
        );
        // The anchor is kept for the next shift-click.
        assert_eq!(selection.anchor.as_deref(), Some("session-c"));

        // An anchor missing from the list restarts from the target.
        selection.set_anchor("session-gone");
        selection.select_range(&flat, "session-a");
        assert_eq!(selection.selected, keys(&["session-a"]));
        assert_eq!(selection.anchor.as_deref(), Some("session-a"));
    }

    #[test]
    fn select_all_keeps_a_listed_anchor() {
        let mut selection = TimelineSelection::default();
        selection.set_anchor("session-b");
        selection.select_all(keys(&["session-a", "session-b"]));
        assert_eq!(selection.anchor.as_deref(), Some("session-b"));
        selection.set_anchor("session-z");
        selection.select_all(keys(&["session-a", "session-b"]));
        assert_eq!(selection.anchor.as_deref(), Some("session-a"));
        assert_eq!(
            selection.selected_session_ids(),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    #[test]
    fn note_context_requires_an_anchored_or_selected_note() {
        let mut selection = TimelineSelection::default();
        assert!(!selection.has_note_context(Some("a")));
        selection.set_anchor("session-a");
        assert!(selection.has_note_context(Some("a")));
        assert!(!selection.has_note_context(Some("b")));
        selection.set_anchor("event-e");
        selection.toggle("session-b");
        assert!(selection.has_note_context(None));
    }
}
