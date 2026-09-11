//! `FolderPicker` (`session/components/folder-picker.tsx`): the folder button
//! beside the note header's actions (#7476) and its popover — search, the
//! folders in use with the current one checked, `Create "…"`, and `See all
//! folders`.

use gpui::{
    AnyElement, ClickEvent, Context, Entity, Focusable as _, MouseButton, MouseDownEvent,
    SharedString, Window, div, prelude::*, px,
};

use super::Workspace;
use crate::db::NotePreview;
use crate::text_input::{TextInput, TextInputEvent, TextInputStyle};
use crate::ui::{TailwindText as _, icon};

pub(crate) struct FolderPicker {
    search: Entity<TextInput>,
    scroll: gpui::ScrollHandle,
    /// cmdk's `value`: the row under the pointer or keyboard; `None` falls
    /// back to the current folder, else the first match.
    highlighted: Option<String>,
}

/// `@max-[480px]`: the note surface (the `@container`) at or under 480px
/// collapses the trigger to its icon.
const COMPACT_SURFACE_PX: f32 = 480.0;

impl Workspace {
    pub(crate) fn folder_picker_open(&self) -> bool {
        self.folder_picker.is_some()
    }

    pub(crate) fn close_folder_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.folder_picker.take().is_some() {
            self.focus_handle.focus(window);
            cx.notify();
        }
    }

    fn toggle_folder_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.folder_picker.is_some() {
            self.close_folder_picker(window, cx);
            return;
        }
        let theme = self.theme;
        let search = cx.new(|cx| {
            TextInput::new(
                "Search or create folder",
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
            &search,
            window,
            |this, input, event: &TextInputEvent, window, cx| match event {
                TextInputEvent::Changed => {
                    if let Some(picker) = this.folder_picker.as_mut() {
                        picker.highlighted = None;
                    }
                    cx.notify();
                }
                TextInputEvent::Escape => this.close_folder_picker(window, cx),
                // cmdk's Enter selects the highlighted row: the first match, or
                // the create row when the query names a new folder.
                TextInputEvent::Enter => {
                    let query = input.read(cx).text().trim().to_string();
                    let Some(current) = this.current_folder_path() else {
                        return;
                    };
                    let folders = this.picker_folders(&current);
                    let lower = query.to_lowercase();
                    let matches: Vec<String> = folders
                        .into_iter()
                        .filter(|path| path.to_lowercase().contains(&lower))
                        .collect();
                    let highlighted = this
                        .folder_picker
                        .as_ref()
                        .and_then(|picker| picker.highlighted.clone())
                        .filter(|path| matches.contains(path))
                        .or_else(|| matches.iter().find(|path| **path == current).cloned())
                        .or_else(|| matches.first().cloned());
                    match highlighted {
                        Some(path) => {
                            let target = if path == current { String::new() } else { path };
                            this.pick_folder(target, window, cx);
                        }
                        None => {
                            if let Some(normalized) = crate::timeline::normalize_folder_path(&query)
                                .filter(|path| !path.is_empty())
                            {
                                this.pick_folder(normalized, window, cx);
                            }
                        }
                    }
                }
                _ => {}
            },
        )
        .detach();
        search.read(cx).focus_handle(cx).focus(window);
        self.folder_picker = Some(FolderPicker {
            search,
            scroll: gpui::ScrollHandle::new(),
            highlighted: None,
        });
        self.reload_folder_catalog(cx);
        cx.notify();
    }

    /// `useFolderPaths` / `useFolderIcons` for the header and the picker.
    pub(crate) fn reload_folder_catalog(&mut self, cx: &mut Context<Self>) {
        let task = self.store.load_folder_catalog();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(catalog)) = task.await else {
                return;
            };
            this.update(cx, |this, cx| {
                this.folder_catalog = catalog;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn current_folder_path(&self) -> Option<String> {
        match &self.note {
            super::Note::Ready { preview, .. } => Some(
                crate::timeline::normalize_folder_path(&preview.session.folder_id)
                    .unwrap_or_default(),
            ),
            _ => None,
        }
    }

    /// `folders`: the catalogue's paths, with a current path the catalogue
    /// does not list yet folded in (`collectWithCurrent`).
    fn picker_folders(&self, current: &str) -> Vec<String> {
        let mut folders = self.folder_catalog.paths.clone();
        if !current.is_empty() && !folders.iter().any(|path| path == current) {
            folders.push(current.to_string());
            folders.sort();
        }
        folders
    }

    /// `handleSelect`: close, then — for a folder the catalogue lacks — create
    /// it and select it in the Folders tab like `setSelectedPath`, and write
    /// `updateSession({ folder_id })`.
    fn pick_folder(&mut self, folder: String, window: &mut Window, cx: &mut Context<Self>) {
        self.close_folder_picker(window, cx);
        let Some(current) = self.current_folder_path() else {
            return;
        };
        if folder == current {
            return;
        }
        let Some(session_id) = self.selected.clone() else {
            return;
        };
        let known = self.folder_catalog.paths.contains(&folder);
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            if !folder.is_empty() && !known {
                let created = store
                    .create_folder(folder.clone())
                    .await
                    .map_err(anyhow::Error::from)
                    .and_then(|result| result);
                if let Err(error) = created {
                    tracing::error!(%error, "[folder-picker] failed to update folder");
                    return;
                }
                this.update(cx, |this, _| {
                    this.pending_folder_selection = Some(folder.clone())
                })
                .ok();
            }
            match store.update_folder(session_id.clone(), folder).await {
                Ok(Ok(())) => {
                    this.update(cx, |this, cx| {
                        this.reload_sessions(cx);
                        this.reload_note(session_id, cx);
                        this.reload_folder_catalog(cx);
                    })
                    .ok();
                }
                Ok(Err(error)) => {
                    tracing::error!(%error, "[folder-picker] failed to update folder")
                }
                Err(error) => tracing::error!(%error, "[folder-picker] failed to update folder"),
            }
        })
        .detach();
    }

    /// The width of the note surface — the `@container` the trigger's
    /// `@max-[480px]` variants measure.
    fn note_surface_width(&self, window: &Window) -> f32 {
        let main = self.main_layout(window);
        let sidebar = if self.sidebar_expanded && !self.is_standalone() {
            self.custom_sidebar_width()
                + if self.custom_sidebar_open() {
                    0.0
                } else {
                    crate::sidebar_layout::HANDLE_PX
                }
        } else {
            0.0
        };
        (main.body - sidebar).max(0.0)
    }

    /// The `h-7 rounded-full` trigger: `w-7` with the Folder glyph for a note
    /// without a folder, else `max-w-36 gap-1 px-1.5` with the folder's icon
    /// and its path in `text-xs text-neutral-600` (icon-only under 480px).
    pub(super) fn render_folder_picker_trigger(
        &self,
        preview: &NotePreview,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let current =
            crate::timeline::normalize_folder_path(&preview.session.folder_id).unwrap_or_default();
        let open = self.folder_picker.is_some();
        let hovered = self.hovered == Some("folder-picker");
        let compact = self.note_surface_width(window) <= COMPACT_SURFACE_PX;
        let color = if open || hovered {
            theme.foreground
        } else {
            theme.muted_foreground
        };
        let glyph: AnyElement = if current.is_empty() {
            icon("folder", px(16.0), color).into_any_element()
        } else {
            let folder_icon = self
                .folder_catalog
                .icons
                .iter()
                .find(|(path, _)| *path == current)
                .map(|(_, icon)| icon.clone())
                .unwrap_or_else(crate::db::TemplateIcon::default_folder);
            self.template_icon_glyph(&folder_icon, px(16.0))
        };
        let show_label = !current.is_empty() && !compact;

        let mut trigger = div()
            .id("folder-picker")
            .relative()
            .flex()
            .h(px(28.0))
            .flex_shrink_0()
            .items_center()
            .cursor_pointer()
            .when(show_label, |trigger| {
                trigger.max_w(px(144.0)).min_w_0().gap_1().px(px(6.0))
            })
            .when(!show_label, |trigger| trigger.w(px(28.0)).justify_center())
            .when(open || hovered, |trigger| {
                trigger.child(crate::squircle::squircle(
                    crate::squircle::CONTROL_RADIUS,
                    Some(theme.accent),
                    None,
                ))
            })
            .on_hover(cx.listener(|this, hovering: &bool, _, cx| {
                this.set_hovered("folder-picker", *hovering, cx);
            }))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.toggle_folder_picker(window, cx);
            }))
            .child(glyph);
        if show_label {
            trigger = trigger.child(
                div()
                    .min_w_0()
                    .truncate()
                    .tw_text_xs()
                    .text_color(theme.folder_label)
                    .child(SharedString::from(current.clone())),
            );
        }

        div()
            .relative()
            .flex_shrink_0()
            .child(trigger)
            .children(self.render_folder_picker_popover(&current, cx))
            .into_any_element()
    }

    /// `PopoverContent variant="app" align="end"` (`w-56 overflow-hidden pb-0`):
    /// the app panel with cmdk's `h-8 px-2.5` input row and `p-1` list, then
    /// the `See all folders ›` footer in the chrome.
    fn render_folder_picker_popover(
        &self,
        current: &str,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let picker = self.folder_picker.as_ref()?;
        let theme = self.theme;
        let query = picker.search.read(cx).text().trim().to_string();
        let lower = query.to_lowercase();
        let folders = self.picker_folders(current);
        let matches: Vec<&String> = folders
            .iter()
            .filter(|path| lower.is_empty() || path.to_lowercase().contains(&lower))
            .collect();
        let normalized_query = crate::timeline::normalize_folder_path(&query);
        let can_create = normalized_query
            .as_ref()
            .is_some_and(|path| !path.is_empty() && !folders.iter().any(|f| f == path));
        let focus_input = picker.search.clone();

        // cmdk keeps one row selected: the hovered one, else the current
        // folder when listed, else the first match.
        let selected_key: Option<String> = picker
            .highlighted
            .clone()
            .filter(|key| {
                matches.iter().any(|path| **path == *key) || (can_create && key == "create-folder")
            })
            .or_else(|| {
                matches
                    .iter()
                    .find(|path| **path == current)
                    .map(|p| (*p).clone())
            })
            .or_else(|| matches.first().map(|p| (*p).clone()))
            .or_else(|| can_create.then(|| "create-folder".to_string()));
        let item = |key: String,
                    glyph: AnyElement,
                    label: String,
                    checked: bool,
                    on_click: super::menu::Select| {
            let on_click = std::rc::Rc::new(on_click);
            let selected = selected_key.as_deref() == Some(key.as_str());
            let hover_key = key.clone();
            // `CommandItem`: `flex items-center gap-2 px-2 py-1.5 text-sm` on the
            // app floating item's `rounded-[14px]`, `bg-accent` when selected.
            div()
                .id(SharedString::from(format!("folder-pick-{key}")))
                .flex()
                .w_full()
                .items_center()
                .gap_2()
                .px_2()
                .py(px(6.0))
                .rounded(px(14.0))
                .tw_text_sm()
                .text_color(theme.foreground)
                .cursor_pointer()
                .when(selected, |row| row.bg(theme.accent))
                .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                    if *hovering
                        && let Some(picker) = this.folder_picker.as_mut()
                        && picker.highlighted.as_deref() != Some(hover_key.as_str())
                    {
                        picker.highlighted = Some(hover_key.clone());
                        cx.notify();
                    }
                }))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(
                    cx.listener(move |this, _: &ClickEvent, window, cx| on_click(this, window, cx)),
                )
                .child(glyph)
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .child(SharedString::from(label)),
                )
                .when(checked, |row| {
                    row.child(icon("check", px(16.0), theme.foreground))
                })
        };

        let mut list = div().flex().flex_col();
        // `CommandGroup` wraps the rows in another `p-1`.
        let mut group = div().flex().flex_col().p_1();
        if matches.is_empty() && !can_create {
            // `CommandEmpty`: `px-2.5 py-2 text-left text-sm text-muted-foreground`.
            let message = if query.is_empty() {
                "No folders yet."
            } else if normalized_query.is_none() {
                "Enter a valid folder name."
            } else {
                "No folders found."
            };
            list = list.child(
                div()
                    .px(px(10.0))
                    .py_2()
                    .tw_text_sm()
                    .text_color(theme.muted_foreground)
                    .child(message),
            );
        }
        for path in &matches {
            let checked = *path == current;
            let folder_icon = self
                .folder_catalog
                .icons
                .iter()
                .find(|(icon_path, _)| icon_path == *path)
                .map(|(_, icon)| icon.clone())
                .unwrap_or_else(crate::db::TemplateIcon::default_folder);
            let target = if checked {
                String::new()
            } else {
                (*path).clone()
            };
            group = group.child(item(
                (*path).clone(),
                div()
                    .opacity(0.7)
                    .child(self.template_icon_glyph(&folder_icon, px(16.0)))
                    .into_any_element(),
                (*path).clone(),
                checked,
                Box::new(move |this, window, cx| this.pick_folder(target.clone(), window, cx)),
            ));
        }
        if can_create && let Some(name) = normalized_query.clone() {
            let target = name.clone();
            group = group.child(item(
                "create-folder".to_string(),
                icon("plus", px(16.0), theme.foreground).into_any_element(),
                format!("Create \"{name}\""),
                false,
                Box::new(move |this, window, cx| this.pick_folder(target.clone(), window, cx)),
            ));
        }

        if !matches.is_empty() || can_create {
            list = list.child(group);
        }

        let panel = super::menu::menu_chrome(theme, "folder-picker", 224.0)
            .pb_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, window, cx| {
                this.close_folder_picker(window, cx);
            }))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded(px(20.0))
                    .child(crate::squircle::squircle(
                        crate::squircle::PANEL_RADIUS,
                        Some(theme.floating_panel),
                        Some((1.0, theme.floating_border)),
                    ))
                    .child(
                        // `[cmdk-input-wrapper]`: `flex h-8 items-center border-b px-2.5`
                        // with the `mr-2 h-4 w-4 opacity-50` glass.
                        div()
                            .id("folder-picker-search")
                            .flex()
                            .h(px(32.0))
                            .items_center()
                            .border_b_1()
                            .border_color(theme.border)
                            .px(px(10.0))
                            .cursor_text()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                                focus_input.read(cx).focus_handle(cx).focus(window);
                            }))
                            .child(div().mr_2().opacity(0.5).child(icon(
                                "search",
                                px(16.0),
                                theme.foreground,
                            )))
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .tw_text_sm()
                                    .child(picker.search.clone()),
                            ),
                    )
                    .child(
                        // `CommandList className="p-1"` (`max-h-[300px] overflow-y-auto`).
                        div()
                            .relative()
                            .child(
                                div()
                                    .id("folder-picker-results")
                                    .max_h(px(300.0))
                                    .overflow_y_scroll()
                                    .track_scroll(&picker.scroll)
                                    .pl_1()
                                    .pr(px(4.0) + crate::ui::scrollbar_gutter(&picker.scroll))
                                    .py_1()
                                    .child(list),
                            )
                            .child(crate::ui::webkit_scrollbar(
                                picker.scroll.clone(),
                                theme.scrollbar_thumb,
                            )),
                    ),
            )
            .child(
                // `See all folders ›` (`px-3 py-1.5 text-xs font-medium`).
                div()
                    .id("folder-picker-see-all")
                    .flex()
                    .w_full()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .px_3()
                    .py(px(6.0))
                    .tw_text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.muted_foreground)
                    .cursor_pointer()
                    .hover(move |style| style.bg(theme.accent).text_color(theme.foreground))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        // `handleSeeAllFolders`: the current folder selected.
                        let current = this.current_folder_path().unwrap_or_default();
                        this.close_folder_picker(window, cx);
                        if !current.is_empty() {
                            this.pending_folder_selection = Some(current);
                        }
                        this.open_folders(window, cx);
                    }))
                    .child("See all folders")
                    .child(icon("caret-right", px(14.0), theme.muted_foreground)),
            );

        // `align="end" sideOffset={4}` under the `h-7` trigger.
        Some(
            div()
                .absolute()
                .top(px(32.0))
                .right_0()
                .child(
                    gpui::deferred(
                        gpui::anchored()
                            .anchor(gpui::Corner::TopRight)
                            .snap_to_window_with_margin(px(8.0))
                            .child(panel),
                    )
                    .with_priority(2),
                )
                .into_any_element(),
        )
    }
}
