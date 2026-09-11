//! `MainChatPanels` (`shared/main/chat-panels.tsx`): the body / right-chat
//! split of the main surface, the `w-0` handle with the library's hit area,
//! the share `autoSaveId="main-chat"` persists (shared with the web view's
//! localStorage like the sidebar's), and `useNoteSurfaceWindowWidthGuard`.

use gpui::{AnyElement, Context, MouseButton, Pixels, Window, div, prelude::*, px, size};

use super::Workspace;
use crate::chat_panel_layout::{self as layout, GuardStep, Measured, PanelState, Surface};
use crate::store_file::StoreFile;

/// The shell's own `store.json` scope/key for the share when the webview has
/// no layout to share.
const STORE_SCOPE: &str = "gpui";
const STORE_KEY: &str = "chat_panel_fraction";

pub(super) struct ChatPanelDrag {
    start_x: Pixels,
    start_fraction: f64,
    group_width: f32,
}

/// The panel group's widths this frame: the body (sidebar + surface) and,
/// when the chat is docked, the chat panel, both in the group's coordinates
/// (the window minus the scaffold's `pl-1`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct MainLayout {
    pub group: f32,
    pub body: f32,
    pub chat: Option<f32>,
}

impl Workspace {
    /// `currentTab.type` for `getMainBodyMinWidth` / `usesNoteSurfaceMinWidth`.
    fn main_surface(&self) -> Surface {
        if self.automations_open() {
            Surface::Automations
        } else if self.settings_open() {
            Surface::Settings
        } else if self.edit_review_open()
            || self.contacts_open()
            || self.calendar_open()
            || self.templates_open()
            || self.folders_open()
        {
            Surface::Other
        } else if self.is_standalone() {
            Surface::StandaloneNote
        } else {
            Surface::Note
        }
    }

    fn left_sidebar_open(&self) -> bool {
        self.sidebar_expanded && !self.is_standalone()
    }

    pub(super) fn main_layout(&self, window: &Window) -> MainLayout {
        // `shell-scaffold`'s `pl-1` exists only with the left chrome.
        let inset = if self.left_sidebar_open() {
            crate::sidebar_layout::GROUP_INSET_PX
        } else {
            0.0
        };
        let group = (f32::from(window.viewport_size().width) - inset).max(1.0);
        if !self.chat_in_right_panel() {
            return MainLayout {
                group,
                body: group,
                chat: None,
            };
        }
        let body_min = layout::body_min_width(self.main_surface(), self.left_sidebar_open(), group);
        let fraction = self.chat_panel_fraction.unwrap_or(layout::DEFAULT_FRACTION);
        let (body, chat) = layout::split(group, fraction, body_min);
        MainLayout {
            group,
            body,
            chat: Some(chat),
        }
    }

    /// The `ResizablePanelGroup` row for a surface's content: the body panel
    /// (the content, at the body width less the sidebar beside it) and the
    /// chat panel, overflowing the surface (clipped at the window's edge) when
    /// their minimums do not fit, with the `w-0` handle between them.
    pub(super) fn render_with_chat_panel(
        &mut self,
        content: AnyElement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let main = self.main_layout(window);
        let Some(chat) = main.chat else {
            return content;
        };
        // The body panel holds the sidebar group: sidebar, its handle (only
        // the timeline's is rendered), then this content.
        let sidebar = if self.left_sidebar_open() {
            self.custom_sidebar_width()
                + if self.custom_sidebar_open() {
                    0.0
                } else {
                    crate::sidebar_layout::HANDLE_PX
                }
        } else {
            0.0
        };
        let body = (main.body - sidebar).max(0.0);
        div()
            .relative()
            .flex()
            .size_full()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .w(px(body))
                    .flex_shrink_0()
                    .min_h_0()
                    .flex_col()
                    .overflow_hidden()
                    .bg(self.theme.card)
                    .child(content),
            )
            // `[data-chat-right-panel]`
            .child(self.render_chat_right_panel(chat, window, cx))
            .child(
                div()
                    .id("chat-panel-resize-handle")
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(body - layout::HANDLE_HIT_AREA_PX))
                    .w(px(layout::HANDLE_HIT_AREA_PX * 2.0))
                    .cursor_col_resize()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.begin_chat_panel_drag(event.position.x, window, cx);
                        }),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn chat_panel_dragging(&self) -> bool {
        self.chat_panel_drag.is_some()
    }

    fn begin_chat_panel_drag(&mut self, x: Pixels, window: &Window, cx: &mut Context<Self>) {
        let main = self.main_layout(window);
        self.chat_panel_drag = Some(ChatPanelDrag {
            start_x: x,
            start_fraction: self.chat_panel_fraction.unwrap_or(layout::DEFAULT_FRACTION),
            group_width: main.group,
        });
        cx.notify();
    }

    pub(super) fn update_chat_panel_drag(&mut self, x: Pixels, cx: &mut Context<Self>) {
        if let Some(drag) = &self.chat_panel_drag {
            let fraction = layout::dragged_fraction(
                drag.start_fraction,
                f32::from(x - drag.start_x),
                drag.group_width,
            );
            if self.chat_panel_fraction != Some(fraction) {
                self.chat_panel_fraction = Some(fraction);
                cx.notify();
            }
        }
    }

    pub(super) fn end_chat_panel_drag(&mut self, cx: &mut Context<Self>) {
        if self.chat_panel_drag.take().is_some() {
            if let Some(fraction) = self.chat_panel_fraction {
                self.save_chat_panel_fraction(fraction);
            }
            cx.notify();
        }
    }

    fn save_chat_panel_fraction(&self, fraction: f64) {
        if let Err(error) = self
            .store_file
            .set_scoped_f64(STORE_SCOPE, STORE_KEY, fraction)
        {
            tracing::warn!(%error, "failed to save the chat panel width");
        }
        let files =
            crate::webkit_local_storage::origin_files(self.store.path(), self.store.identifier());
        if files.is_empty() {
            return;
        }
        self.store.runtime().spawn(async move {
            let current = crate::webkit_local_storage::read(&files, layout::STORAGE_KEY).await;
            let next = layout::with_layout_fraction(current.as_deref(), fraction);
            crate::webkit_local_storage::write(&files, layout::STORAGE_KEY, &next).await;
        });
    }

    /// The saved share: the webview's layout first, then the shell's copy.
    pub(super) fn load_chat_panel_fraction(
        store: &crate::db::Store,
        store_file: &StoreFile,
    ) -> Option<f64> {
        let files = crate::webkit_local_storage::origin_files(store.path(), store.identifier());
        let shared = if files.is_empty() {
            None
        } else {
            store
                .runtime()
                .block_on(crate::webkit_local_storage::read(
                    &files,
                    layout::STORAGE_KEY,
                ))
                .and_then(|document| layout::read_layout_fraction(&document))
        };
        shared
            .or_else(|| store_file.scoped_f64(STORE_SCOPE, STORE_KEY))
            .map(layout::clamp_fraction)
    }

    /// `useNoteSurfaceWindowWidthGuard`, run once the frame's layout is
    /// known: its layout effect on a panel-state change, then its resize
    /// handler while the sidebar is open on a note-surface tab.
    pub(super) fn run_width_guard(
        &mut self,
        main: MainLayout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let surface = self.main_surface();
        let current = PanelState {
            enabled: surface.uses_note_surface_min_width(),
            left_open: self.left_sidebar_open(),
            right_open: self.chat_in_right_panel(),
        };
        let note_min = match surface {
            Surface::StandaloneNote => layout::STANDALONE_NOTE_SURFACE_MIN_WIDTH_PX,
            _ => layout::NOTE_SURFACE_MIN_WIDTH_PX,
        };
        let left_sidebar = if current.left_open {
            self.custom_sidebar_width()
        } else {
            0.0
        };
        let right_panel = main.chat.unwrap_or(0.0);
        let body_visible = main.body.min((main.group - right_panel).max(0.0));
        let previous = std::mem::replace(&mut self.width_guard_state, current);
        if previous != current {
            // The effect re-runs: the resize handler's baseline resets.
            self.width_guard_last_body = None;
            let steps = layout::guard_steps(
                previous,
                current,
                Measured {
                    body_visible,
                    left_sidebar,
                    right_panel,
                },
                note_min,
            );
            for step in steps {
                match step {
                    GuardStep::RestoreExpansions => self.restore_width_expansions(window),
                    GuardStep::CollapseLeft => {
                        self.sidebar_expanded = false;
                        cx.notify();
                    }
                    GuardStep::ExpandWindow {
                        deficit,
                        restore_on_close,
                        ..
                    } => self.expand_window_width(deficit, restore_on_close, window),
                }
            }
            return;
        }
        if !(current.enabled && current.left_open) {
            return;
        }
        let last = self.width_guard_last_body.replace(body_visible);
        if layout::shrink_collapses_left(last, body_visible, left_sidebar, note_min) {
            self.sidebar_expanded = false;
            cx.notify();
        }
    }

    /// `window_expand_width(deficit, None, false, _, restore_on_close)`: the
    /// window grows by the deficit (to the right: GPUI sizes the content
    /// without moving the frame, as the plugin does off macOS), recorded for
    /// the restore when asked.
    fn expand_window_width(&mut self, deficit: f32, restore_on_close: bool, window: &mut Window) {
        let viewport = window.viewport_size();
        let previous = f32::from(viewport.width);
        let expanded = previous + deficit;
        window.resize(size(px(expanded), viewport.height));
        if restore_on_close {
            self.width_expansions.push((previous, expanded));
        }
    }

    /// `restoreWidthExpansions` → `window_restore_width` per recorded
    /// expansion: undone only while the window still has the expanded width.
    fn restore_width_expansions(&mut self, window: &mut Window) {
        let mut width = f32::from(window.viewport_size().width);
        while let Some((previous, expanded)) = self.width_expansions.pop() {
            if (width - expanded).abs() < 1.0 {
                window.resize(size(px(previous), window.viewport_size().height));
                width = previous;
            }
        }
    }
}
