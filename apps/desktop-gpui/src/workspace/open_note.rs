//! `apps/desktop/src/shared/open-note-dialog.tsx`: the "Find a note..."
//! command palette opened by the sidebar search button and `mod+k`.

use gpui::{
    AnyElement, BoxShadow, ClickEvent, Context, Entity, Focusable as _, MouseButton,
    MouseDownEvent, SharedString, Window, anchored, deferred, div, hsla, point, prelude::*, px,
};

use super::Workspace;
use crate::text_input::{TextInput, TextInputEvent, TextInputStyle};
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

const MAX_RECENT_DISPLAY: usize = 5;
/// `MAX_RECENT_SESSIONS` in `store/zustand/tabs/recently-opened.ts`.
pub(super) const MAX_RECENT_SESSIONS: usize = 10;

pub(crate) struct OpenNoteDialog {
    pub(super) input: Entity<TextInput>,
    /// cmdk keeps one item highlighted; the first by default.
    pub(super) selected: usize,
}

struct NoteResult {
    id: String,
    title: SharedString,
}

impl Workspace {
    pub(crate) fn open_note_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open_note.is_some() {
            return;
        }
        let theme = self.theme;
        let input = cx.new(|cx| {
            TextInput::new(
                "Find a note...",
                TextInputStyle {
                    text: theme.foreground,
                    placeholder: theme.muted_foreground,
                    selection: theme.selection,
                    underline_when_focused: false,
                    masked: false,
                },
                window,
                cx,
            )
        });
        cx.subscribe_in(
            &input,
            window,
            |this, _, event: &TextInputEvent, window, cx| match event {
                TextInputEvent::Changed => {
                    if let Some(dialog) = &mut this.open_note {
                        dialog.selected = 0;
                        cx.notify();
                    }
                }
                TextInputEvent::Navigate(delta) => this.move_open_note_selection(*delta, cx),
                TextInputEvent::Enter => this.choose_open_note_result(window, cx),
                TextInputEvent::Escape => this.close_open_note_dialog(window, cx),
                TextInputEvent::Committed
                | TextInputEvent::BackspaceEmpty
                | TextInputEvent::ShiftEnter
                | TextInputEvent::ModEnter => {}
            },
        )
        .detach();
        input.read(cx).focus_handle(cx).focus(window);
        self.open_note = Some(OpenNoteDialog { input, selected: 0 });
        cx.notify();
    }

    pub(crate) fn close_open_note_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open_note.take().is_some() {
            self.focus_handle.focus(window);
            cx.notify();
        }
    }

    fn move_open_note_selection(&mut self, delta: i32, cx: &mut Context<Self>) {
        let count = self
            .open_note_results(cx)
            .iter()
            .map(|group| group.1.len())
            .sum::<usize>();
        let Some(dialog) = &mut self.open_note else {
            return;
        };
        if count == 0 {
            return;
        }
        // cmdk stops at the ends instead of wrapping.
        dialog.selected = (dialog.selected as i32 + delta).clamp(0, count as i32 - 1) as usize;
        cx.notify();
    }

    fn choose_open_note_result(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = &self.open_note else {
            return;
        };
        let selected = dialog.selected;
        let id = self
            .open_note_results(cx)
            .into_iter()
            .flat_map(|(_, results)| results)
            .nth(selected)
            .map(|result| result.id);
        self.close_open_note_dialog(window, cx);
        if let Some(id) = id {
            self.select(id, cx);
        }
    }

    /// `filteredRecentSessions` and `filteredOtherNotes`: recents first (at
    /// most five, in opening order), then every other session newest first,
    /// both filtered by a case-insensitive substring match on the title.
    fn open_note_results(&self, cx: &gpui::App) -> Vec<(&'static str, Vec<NoteResult>)> {
        let query = self
            .open_note
            .as_ref()
            .map(|dialog| dialog.input.read(cx).text().trim().to_lowercase())
            .unwrap_or_default();
        let display_title = |title: &str| -> SharedString {
            if title.is_empty() {
                "Untitled".into()
            } else {
                title.to_string().into()
            }
        };
        let matches =
            |title: &SharedString| query.is_empty() || title.to_lowercase().contains(&query);

        let mut sorted: Vec<&crate::timeline::SessionRow> = self.session_rows.iter().collect();
        sorted.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });

        let recent: Vec<NoteResult> = self
            .recently_opened
            .iter()
            .take(MAX_RECENT_DISPLAY)
            .filter_map(|id| sorted.iter().find(|row| &row.id == id))
            .map(|row| NoteResult {
                id: row.id.clone(),
                title: display_title(&row.title),
            })
            .filter(|result| matches(&result.title))
            .collect();
        let recent_ids: std::collections::HashSet<&str> = self
            .recently_opened
            .iter()
            .take(MAX_RECENT_DISPLAY)
            .map(String::as_str)
            .collect();
        let others: Vec<NoteResult> = sorted
            .iter()
            .filter(|row| !recent_ids.contains(row.id.as_str()))
            .map(|row| NoteResult {
                id: row.id.clone(),
                title: display_title(&row.title),
            })
            .filter(|result| matches(&result.title))
            .collect();

        let mut groups = Vec::new();
        if !recent.is_empty() {
            groups.push(("Recent", recent));
        }
        if !others.is_empty() {
            groups.push(("All Notes", others));
        }
        groups
    }

    pub(super) fn render_open_note_dialog(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let dialog = self.open_note.as_ref()?;
        let theme = self.theme;
        let viewport = window.viewport_size();
        let groups = self.open_note_results(cx);
        let selected = dialog.selected;
        let mut index = 0usize;

        // `DialogContent`: `top-[15%] w-full max-w-lg px-4`, centred.
        let panel_width = px(512.0 - 32.0);
        let left = (viewport.width - px(512.0)) / 2.0 + px(16.0);
        let top = viewport.height * 0.15;

        let header = div()
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(alpha(theme.border, 0.6))
            .child(icon("search", px(16.0), theme.muted_foreground))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .tw_text_sm()
                    .child(dialog.input.clone()),
            )
            .child(
                div()
                    .id("open-note-close")
                    .size(px(20.0))
                    // `.rounded-full` is `0.5rem` in the desktop app.
                    .rounded(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(alpha(theme.accent, 0.8))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.close_open_note_dialog(window, cx)
                    }))
                    .child(icon("x", px(12.0), theme.muted_foreground)),
            );

        let heading = |label: &'static str| {
            div()
                .px_2()
                .py(px(6.0))
                .tw_text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.muted_foreground)
                .child(SharedString::from(label.to_uppercase()))
        };

        let mut list = div()
            .id("open-note-list")
            .max_h(px(320.0))
            .overflow_y_scroll()
            .p_2()
            .flex()
            .flex_col();
        if groups.is_empty() {
            list = list.child(
                div()
                    .py_6()
                    .w_full()
                    .text_center()
                    .tw_text_sm()
                    .text_color(theme.muted_foreground)
                    .child("No notes found."),
            );
        }
        let group_count = groups.len();
        for (group_index, (label, results)) in groups.into_iter().enumerate() {
            let mut group = div().flex().flex_col();
            if group_index == 0 && group_count > 1 {
                group = group.pb(px(6.0));
            }
            if group_index > 0 {
                // `flex flex-col gap-3` heading with the `bg-accent mx-2 h-px` rule.
                group = group
                    .child(div().mx_2().h(px(1.0)).bg(theme.accent))
                    .child(div().h(px(12.0)));
            }
            group = group.child(heading(label));
            for result in results {
                let this_index = index;
                index += 1;
                let is_selected = this_index == selected;
                group = group.child(
                    div()
                        .id(("open-note-result", this_index))
                        .flex()
                        .items_center()
                        .gap_3()
                        .rounded_lg()
                        .px_3()
                        .py(px(10.0))
                        .tw_text_sm()
                        .text_color(theme.muted_foreground)
                        .cursor_pointer()
                        .when(is_selected, |item| item.bg(alpha(theme.accent, 0.6)))
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            if *hovered && let Some(dialog) = &mut this.open_note {
                                dialog.selected = this_index;
                                cx.notify();
                            }
                        }))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            if let Some(dialog) = &mut this.open_note {
                                dialog.selected = this_index;
                            }
                            this.choose_open_note_result(window, cx);
                        }))
                        .child(icon("file-text", px(16.0), theme.muted_foreground))
                        .child(div().min_w_0().flex_1().truncate().child(result.title)),
                );
            }
            list = list.child(group);
        }

        let panel = div()
            .id("open-note-panel")
            .occlude()
            .w(panel_width)
            .flex()
            .flex_col()
            .rounded(px(16.0))
            .border_1()
            .border_color(alpha(theme.border, 0.8))
            .bg(theme.background)
            .shadow(vec![BoxShadow {
                color: hsla(0.0, 0.0, 0.0, 0.25),
                offset: point(px(0.0), px(25.0)),
                blur_radius: px(50.0),
                spread_radius: px(-12.0),
            }])
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(header)
            .child(list);

        // `DialogOverlay`: `bg-black/20` over the whole window; clicking it
        // closes the dialog (the top 15% is also the drag region).
        let overlay = div()
            .id("open-note-overlay")
            .absolute()
            .top_0()
            .left_0()
            .w(viewport.width)
            .h(viewport.height)
            .bg(hsla(0.0, 0.0, 0.0, 0.2))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    if event.position.y < window.viewport_size().height * 0.15 {
                        window.start_window_move();
                    } else {
                        this.close_open_note_dialog(window, cx);
                    }
                }),
            )
            .child(div().absolute().top(top).left(left).child(panel));

        Some(
            deferred(anchored().position(point(px(0.0), px(0.0))).child(overlay))
                .with_priority(2)
                .into_any_element(),
        )
    }
}
