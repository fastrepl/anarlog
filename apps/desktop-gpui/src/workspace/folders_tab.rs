//! The Folders tab: `folders/{index,sidebar,folder-editor,selection}.tsx`,
//! `sidebar/folder-name-dialog.tsx`, and `shared/ui/destructive-confirmation-dialog.tsx`.

use gpui::{
    AnyElement, ClickEvent, Context, Div, Entity, Focusable as _, MouseButton, MouseDownEvent,
    SharedString, Window, div, prelude::*, px,
};

use super::Workspace;
use super::toast::FlashVariant;
use crate::folders::{Catalog, Material, display_name};
use crate::text_area::{TextArea, TextAreaEvent, TextAreaStyle};
use crate::text_input::{TextInput, TextInputEvent, TextInputStyle};
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

pub(crate) struct FoldersState {
    pub(super) catalog: Catalog,
    /// `useFolderSelection.selectedPath`
    selected: Option<String>,
    /// `useFolderSelection.deletedPrefixes`
    deleted_prefixes: Vec<String>,
    search: Entity<TextInput>,
    /// The New folder dialog while open.
    creating: Option<NameDialog>,
    editor: Option<FolderEditor>,
}

struct NameDialog {
    input: Entity<TextInput>,
    error: Option<&'static str>,
    busy: bool,
}

struct FolderEditor {
    path: String,
    name: Entity<TextInput>,
    instructions: Entity<TextArea>,
    saved_instructions: String,
    materials: Vec<Material>,
    actions_open: bool,
    deleting: bool,
    busy: bool,
}

impl Workspace {
    fn input_style(&self) -> TextInputStyle {
        TextInputStyle {
            text: self.theme.foreground,
            placeholder: self.theme.muted_foreground,
            selection: self.theme.selection,
            underline_when_focused: false,
            masked: false,
        }
    }

    /// `openNew({ type: "folders" })`
    pub(crate) fn open_folders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_settings(cx);
        self.close_templates(cx);
        self.close_calendar(cx);
        self.close_contacts(cx);
        if self.folders.is_none() {
            let style = self.input_style();
            let search = cx.new(|cx| TextInput::new("Search folders...", style, window, cx));
            cx.subscribe(&search, |this, input, event: &TextInputEvent, cx| {
                match event {
                    // `onKeyDown Escape` clears the search.
                    TextInputEvent::Escape => input.update(cx, |input, cx| input.set_text("", cx)),
                    TextInputEvent::Changed => {}
                    _ => return,
                }
                if this.folders.is_some() {
                    cx.notify();
                }
            })
            .detach();
            self.folders = Some(FoldersState {
                catalog: Catalog::default(),
                selected: None,
                deleted_prefixes: Vec::new(),
                search,
                creating: None,
                editor: None,
            });
        }
        self.reload_folders(window, cx);
        cx.notify();
    }

    /// `leaveOverlayTab`
    pub(crate) fn close_folders(&mut self, cx: &mut Context<Self>) {
        if self.folders.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn folders_open(&self) -> bool {
        self.folders.is_some()
    }

    pub(crate) fn folder_dialog_open(&self) -> bool {
        self.folders.as_ref().is_some_and(|state| {
            state.creating.is_some() || state.editor.as_ref().is_some_and(|editor| editor.deleting)
        })
    }

    pub(crate) fn close_folder_dialogs(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = self.folders.as_mut() {
            state.creating = None;
            if let Some(editor) = state.editor.as_mut() {
                editor.deleting = false;
            }
            cx.notify();
        }
    }

    /// The change watcher has no window; refresh the catalog and the open
    /// editor's details without rebuilding inputs.
    pub(crate) fn reload_folders_from_watcher(&mut self, cx: &mut Context<Self>) {
        if self.folders.is_none() {
            return;
        }
        let task = self.store.load_folder_catalog();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(catalog)) = task.await else {
                return;
            };
            this.update(cx, |this, cx| {
                let Some(state) = this.folders.as_mut() else {
                    return;
                };
                state.catalog = catalog;
                cx.notify();
                if let Some(path) = state.editor.as_ref().map(|editor| editor.path.clone()) {
                    this.refresh_folder_details(path, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn reload_folders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.folders.is_none() {
            return;
        }
        let task = self.store.load_folder_catalog();
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(catalog)) = task.await else {
                return;
            };
            this.update_in(cx, |this, window, cx| {
                let Some(state) = this.folders.as_mut() else {
                    return;
                };
                state.catalog = catalog;
                cx.notify();
                this.sync_folder_editor(window, cx);
            })
            .ok();
        })
        .detach();
    }

    /// `useActiveFolderPath`
    fn active_folder(&self) -> Option<String> {
        let state = self.folders.as_ref()?;
        let is_deleted = |path: &str| {
            state
                .deleted_prefixes
                .iter()
                .any(|prefix| path == prefix || path.starts_with(&format!("{prefix}/")))
        };
        if let Some(selected) = &state.selected
            && !is_deleted(selected)
        {
            return Some(selected.clone());
        }
        state
            .catalog
            .paths
            .iter()
            .find(|path| !is_deleted(path))
            .cloned()
    }

    /// `setSelectedPath`
    fn select_folder(&mut self, path: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.folders.as_mut() else {
            return;
        };
        if let Some(path) = &path {
            state
                .deleted_prefixes
                .retain(|prefix| path != prefix && !path.starts_with(&format!("{prefix}/")));
        }
        state.selected = path;
        cx.notify();
        self.sync_folder_editor(window, cx);
    }

    /// `markFolderDeleted`
    fn mark_folder_deleted(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = self.folders.as_mut() else {
            return;
        };
        if state
            .selected
            .as_deref()
            .is_some_and(|selected| selected == path || selected.starts_with(&format!("{path}/")))
        {
            state.selected = None;
        }
        if !state.deleted_prefixes.iter().any(|prefix| prefix == path) {
            state.deleted_prefixes.push(path.to_string());
        }
        cx.notify();
        self.sync_folder_editor(window, cx);
    }

    /// `<FolderEditor key={activeFolder}>`: rebuilt whenever the active path
    /// changes, refreshed in place otherwise.
    fn sync_folder_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = self.active_folder();
        let Some(path) = active else {
            if let Some(state) = self.folders.as_mut() {
                state.editor = None;
            }
            return;
        };
        if self
            .folders
            .as_ref()
            .and_then(|state| state.editor.as_ref())
            .is_some_and(|editor| editor.path == path)
        {
            self.refresh_folder_details(path, cx);
            return;
        }
        let theme = self.theme;
        let style = self.input_style();
        let name = cx.new(|cx| {
            let mut input = TextInput::new("Folder name", style, window, cx);
            input.set_text(display_name(&path), cx);
            input
        });
        cx.subscribe_in(
            &name,
            window,
            |this, _, event: &TextInputEvent, window, cx| match event {
                TextInputEvent::Committed => this.commit_folder_name(window, cx),
                TextInputEvent::Escape => this.revert_folder_name(window, cx),
                _ => {}
            },
        )
        .detach();
        let instructions = cx.new(|cx| {
            TextArea::new(
                "Add context for this folder",
                TextAreaStyle {
                    text: theme.foreground,
                    placeholder: theme.muted_foreground,
                    selection: theme.selection,
                    font_size: px(14.0),
                    line_height: px(20.0),
                    rows: 4,
                },
                window,
                cx,
            )
        });
        cx.subscribe(&instructions, |this, _, event: &TextAreaEvent, cx| {
            if *event == TextAreaEvent::Blurred {
                this.save_folder_instructions(cx);
            }
        })
        .detach();
        if let Some(state) = self.folders.as_mut() {
            state.editor = Some(FolderEditor {
                path: path.clone(),
                name,
                instructions,
                saved_instructions: String::new(),
                materials: Vec::new(),
                actions_open: false,
                deleting: false,
                busy: false,
            });
        }
        self.refresh_folder_details(path, cx);
    }

    /// `useFolderInstructions` + `useFolderMaterials`
    fn refresh_folder_details(&mut self, path: String, cx: &mut Context<Self>) {
        let task = self.store.load_folder_details(path.clone());
        cx.spawn(async move |this, cx| {
            let Ok(Ok((instructions, materials))) = task.await else {
                return;
            };
            this.update(cx, |this, cx| {
                let Some(editor) = this
                    .folders
                    .as_mut()
                    .and_then(|state| state.editor.as_mut())
                    .filter(|editor| editor.path == path)
                else {
                    return;
                };
                // `useEffect(() => setValue(saved), [folderPath, saved])`
                if editor.saved_instructions != instructions {
                    editor.saved_instructions = instructions.clone();
                    editor
                        .instructions
                        .update(cx, |area, cx| area.set_text(instructions, cx));
                }
                editor.materials = materials;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn folder_editor_mut(&mut self) -> Option<&mut FolderEditor> {
        self.folders
            .as_mut()
            .and_then(|state| state.editor.as_mut())
    }

    /// `commitTitle`: rename within the same parent on blur / Enter.
    fn commit_folder_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.folder_editor_mut() else {
            return;
        };
        let path = editor.path.clone();
        let draft = editor.name.read(cx).text().trim().to_string();
        let current = display_name(&path);
        let normalized = crate::timeline::normalize_folder_path(&draft);
        let Some(normalized) = normalized.filter(|name| !name.contains('/')) else {
            editor
                .name
                .update(cx, |input, cx| input.set_text(current, cx));
            return;
        };
        let renamed = match path.rfind('/') {
            Some(index) => format!("{}/{normalized}", &path[..index]),
            None => normalized,
        };
        if renamed == path {
            editor
                .name
                .update(cx, |input, cx| input.set_text(current, cx));
            return;
        }
        editor.busy = true;
        cx.notify();
        let task = self.store.rename_folder(path.clone(), renamed);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update_in(cx, |this, window, cx| {
                if let Some(editor) = this.folder_editor_mut() {
                    editor.busy = false;
                }
                match result {
                    Ok(renamed) => this.select_folder(Some(renamed), window, cx),
                    Err(_) => {
                        if let Some(editor) = this.folder_editor_mut() {
                            editor
                                .name
                                .update(cx, |input, cx| input.set_text(display_name(&path), cx));
                        }
                    }
                }
                this.reload_folders(window, cx);
            })
            .ok();
        })
        .detach();
    }

    /// `Escape`: `skipTitleCommit` + reset the draft.
    fn revert_folder_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.folder_editor_mut() else {
            return;
        };
        let current = display_name(&editor.path);
        editor
            .name
            .update(cx, |input, cx| input.set_text(current, cx));
        window.blur();
    }

    /// `FolderInstructionsField onBlur`
    fn save_folder_instructions(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.folder_editor_mut() else {
            return;
        };
        let value = editor.instructions.read(cx).text().to_string();
        if value == editor.saved_instructions {
            return;
        }
        editor.saved_instructions = value.clone();
        let path = editor.path.clone();
        let task = self.store.update_folder_instructions(path, value);
        cx.spawn(async move |this, cx| {
            if let Ok(Err(error)) = task.await {
                this.update(cx, |this, cx| {
                    this.flash(FlashVariant::Error, error.to_string(), cx);
                })
                .ok();
            }
        })
        .detach();
    }

    /// `createNamedFolder` from the New folder dialog.
    fn submit_new_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = self
            .folders
            .as_mut()
            .and_then(|state| state.creating.as_mut())
        else {
            return;
        };
        let value = dialog.input.read(cx).text().trim().to_string();
        let normalized = crate::timeline::normalize_folder_path(&value);
        let Some(normalized) = normalized.filter(|name| !name.contains('/')) else {
            dialog.error = Some("Enter a valid folder name.");
            cx.notify();
            return;
        };
        dialog.busy = true;
        dialog.error = None;
        cx.notify();
        let task = self.store.create_folder(normalized);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(created) => {
                        if let Some(state) = this.folders.as_mut() {
                            state.creating = None;
                        }
                        this.select_folder(Some(created), window, cx);
                        this.reload_folders(window, cx);
                    }
                    Err(error) => {
                        if let Some(dialog) = this
                            .folders
                            .as_mut()
                            .and_then(|state| state.creating.as_mut())
                        {
                            dialog.busy = false;
                            dialog.error =
                                Some(if error.to_string().contains("folder_target_exists") {
                                    "A folder with this name already exists."
                                } else {
                                    "Could not save the folder."
                                });
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn open_new_folder_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let style = self.input_style();
        let input = cx.new(|cx| TextInput::new("", style, window, cx));
        cx.subscribe_in(
            &input,
            window,
            |this, _, event: &TextInputEvent, window, cx| match event {
                TextInputEvent::Enter => this.submit_new_folder(window, cx),
                TextInputEvent::Escape => this.close_new_folder_dialog(cx),
                TextInputEvent::Changed => {
                    if let Some(dialog) = this
                        .folders
                        .as_mut()
                        .and_then(|state| state.creating.as_mut())
                    {
                        dialog.error = None;
                        cx.notify();
                    }
                }
                _ => {}
            },
        )
        .detach();
        input.read(cx).focus_handle(cx).focus(window);
        if let Some(state) = self.folders.as_mut() {
            state.creating = Some(NameDialog {
                input,
                error: None,
                busy: false,
            });
        }
        cx.notify();
    }

    fn close_new_folder_dialog(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = self.folders.as_mut()
            && state.creating.take().is_some()
        {
            cx.notify();
        }
    }

    /// `deleteNamedFolder` from the confirmation dialog.
    fn confirm_delete_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.folder_editor_mut() else {
            return;
        };
        editor.busy = true;
        let path = editor.path.clone();
        cx.notify();
        let task = self.store.delete_folder(path.clone());
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update_in(cx, |this, window, cx| {
                if let Some(editor) = this.folder_editor_mut() {
                    editor.busy = false;
                    editor.deleting = false;
                }
                match result {
                    Ok(()) => this.mark_folder_deleted(&path, window, cx),
                    Err(error) => this.flash(FlashVariant::Error, error.to_string(), cx),
                }
                this.reload_folders(window, cx);
            })
            .ok();
        })
        .detach();
    }

    /// The hidden `<input type="file">`: a native open dialog, then `upload`.
    fn pick_folder_material(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.folder_editor_mut() else {
            return;
        };
        let path = editor.path.clone();
        let picker = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picker.await else {
                return;
            };
            let Some(file) = paths.into_iter().next() else {
                return;
            };
            let task = this
                .update(cx, |this, cx| {
                    if let Some(editor) = this.folder_editor_mut() {
                        editor.busy = true;
                    }
                    cx.notify();
                    this.store.add_folder_material(path.clone(), file)
                })
                .ok();
            let Some(task) = task else {
                return;
            };
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update_in(cx, |this, window, cx| {
                if let Some(editor) = this.folder_editor_mut() {
                    editor.busy = false;
                }
                if let Err(error) = result {
                    this.flash(FlashVariant::Error, error.to_string(), cx);
                }
                this.reload_folders(window, cx);
            })
            .ok();
        })
        .detach();
    }

    /// `deleteLocalFolderMaterial`
    fn remove_folder_material(
        &mut self,
        attachment_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = self.folder_editor_mut() else {
            return;
        };
        editor.busy = true;
        let path = editor.path.clone();
        cx.notify();
        let task = self.store.remove_folder_material(path, attachment_id);
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update_in(cx, |this, window, cx| {
                if let Some(editor) = this.folder_editor_mut() {
                    editor.busy = false;
                }
                if let Err(error) = result {
                    this.flash(FlashVariant::Error, error.to_string(), cx);
                }
                this.reload_folders(window, cx);
            })
            .ok();
        })
        .detach();
    }

    /// `FoldersSidebar`
    pub(super) fn render_folders_sidebar(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let Some(state) = self.folders.as_ref() else {
            return div();
        };
        let query = state.search.read(cx).text().trim().to_lowercase();
        let filtered: Vec<&String> = state
            .catalog
            .paths
            .iter()
            .filter(|path| query.is_empty() || path.to_lowercase().contains(&query))
            .collect();
        let active = self.active_folder();
        let searching = !query.is_empty();
        let search_input = state.search.clone();
        let focus_input = search_input.clone();

        div()
            .flex()
            .flex_col()
            .h_full()
            .w(px(self.custom_sidebar_width()))
            .flex_shrink_0()
            .pr_1()
            .overflow_hidden()
            .child(
                // `CustomSidebarHeader`: `h-12 pt-[9px] pr-1 pl-2`, back + New folder.
                div()
                    .flex()
                    .h(px(48.0))
                    .flex_shrink_0()
                    .items_start()
                    .pt(px(9.0))
                    .pr_1()
                    .pl_2()
                    .child(
                        div()
                            .flex()
                            .min_w_0()
                            .flex_1()
                            .items_center()
                            .gap_1()
                            .child(
                                self.tracked_chrome_button("folders-back", cx)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.close_folders(cx);
                                    }))
                                    .child(icon(
                                        "arrow-left",
                                        px(16.0),
                                        self.chrome_icon_color("folders-back"),
                                    )),
                            )
                            .child(div().flex_1())
                            .child(
                                self.tracked_chrome_button("folders-new", cx)
                                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                        this.open_new_folder_dialog(window, cx);
                                    }))
                                    .child(icon(
                                        "plus",
                                        px(16.0),
                                        self.chrome_icon_color("folders-new"),
                                    )),
                            ),
                    ),
            )
            .child(
                div().pb_2().child(
                    // `h-8 rounded-lg border bg-accent/50 px-3 gap-2` squircle search box.
                    div()
                        .id("folders-search")
                        .relative()
                        .flex()
                        .h(px(32.0))
                        .w_full()
                        .flex_shrink_0()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .cursor_text()
                        .child(crate::squircle::squircle(
                            crate::squircle::CONTROL_RADIUS,
                            Some(alpha(theme.accent, 0.5)),
                            Some((1.0, theme.border)),
                        ))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                            focus_input.read(cx).focus_handle(cx).focus(window);
                        }))
                        .child(icon("search", px(16.0), theme.muted_foreground))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .tw_text_sm()
                                .child(state.search.clone()),
                        )
                        .when(searching, |row| {
                            row.child(
                                div()
                                    .id("folders-search-clear")
                                    .size(px(16.0))
                                    .flex_shrink_0()
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                                        search_input.update(cx, |input, cx| input.set_text("", cx));
                                    }))
                                    .child(icon("x", px(16.0), theme.muted_foreground)),
                            )
                        }),
                ),
            )
            .child(
                div()
                    .id("folders-list")
                    .min_h_0()
                    .flex_1()
                    .overflow_y_scroll()
                    .pt_1()
                    .child(if filtered.is_empty() {
                        // `px-3 py-8 text-center` with the 32px folder glyph.
                        div()
                            .px_3()
                            .py_8()
                            .flex()
                            .flex_col()
                            .items_center()
                            .text_color(theme.muted_foreground)
                            .child(div().mb_2().child(icon(
                                "folder",
                                px(32.0),
                                alpha(theme.muted_foreground, 0.7),
                            )))
                            .child(div().tw_text_sm().child(if searching {
                                "No folders found"
                            } else {
                                "No folders yet"
                            }))
                            .into_any_element()
                    } else {
                        div()
                            .flex()
                            .flex_col()
                            .children(filtered.into_iter().map(|path| {
                                let selected = active.as_deref() == Some(path.as_str());
                                let glyph = state
                                    .catalog
                                    .icons
                                    .iter()
                                    .find(|(icon_path, _)| icon_path == path)
                                    .map(|(_, icon)| icon.clone())
                                    .unwrap_or_else(crate::db::TemplateIcon::default_folder);
                                let target = path.clone();
                                div()
                                    .id(SharedString::from(format!("folder-row-{path}")))
                                    .w_full()
                                    .rounded_lg()
                                    .px_3()
                                    .py_2()
                                    .tw_text_sm()
                                    .cursor_pointer()
                                    .when(selected, |row| {
                                        row.bg(theme.accent)
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(theme.foreground)
                                    })
                                    .when(!selected, |row| {
                                        row.text_color(theme.muted_foreground).hover(move |style| {
                                            style
                                                .bg(alpha(theme.accent, 0.5))
                                                .text_color(theme.foreground)
                                        })
                                    })
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, window, cx| {
                                            this.select_folder(Some(target.clone()), window, cx);
                                        },
                                    ))
                                    .child(
                                        div()
                                            .flex()
                                            .min_w_0()
                                            .items_center()
                                            .gap_2()
                                            .child(self.template_icon_glyph(&glyph, px(16.0)))
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .flex_grow()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div()
                                                            // See the templates list: the nowrap
                                                            // text needs its exact width up front.
                                                            .w(px(
                                                                self.custom_sidebar_width() - 52.0
                                                            ))
                                                            .truncate()
                                                            .child(SharedString::from(
                                                                path.clone(),
                                                            )),
                                                    ),
                                            ),
                                    )
                            }))
                            .into_any_element()
                    }),
            )
    }

    /// `FoldersMain`
    pub(super) fn render_folders_main(&self, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        let Some(editor) = self
            .folders
            .as_ref()
            .and_then(|state| state.editor.as_ref())
        else {
            return div()
                .flex()
                .h_full()
                .items_center()
                .justify_center()
                .px_6()
                .tw_text_sm()
                .text_color(theme.muted_foreground)
                .child("No folders yet. Create one to group notes and materials.")
                .into_any_element();
        };
        let path = editor.path.clone();
        let glyph = self
            .folders
            .as_ref()
            .and_then(|state| {
                state
                    .catalog
                    .icons
                    .iter()
                    .find(|(icon_path, _)| *icon_path == path)
                    .map(|(_, icon)| icon.clone())
            })
            .unwrap_or_else(crate::db::TemplateIcon::default_folder);
        let busy = editor.busy;
        let name_input = editor.name.clone();
        let focus_name = name_input.clone();
        let draft = editor.name.read(cx).text().to_string();

        let header = div()
            .flex()
            .h(px(48.0))
            .items_center()
            .justify_between()
            .gap_3()
            .pr_1()
            .pl_3()
            .child(
                div()
                    .flex()
                    .min_w_0()
                    .flex_1()
                    .items_center()
                    .gap_2()
                    .child(
                        // `TemplateIconPicker size="sm"`: `size-7 rounded-md hover:bg-accent`.
                        div()
                            .id("folder-icon")
                            .relative()
                            .flex()
                            .size(px(28.0))
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .cursor_pointer()
                            .hover(move |style| style.bg(theme.accent))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click({
                                let target = super::icon_picker::IconTarget::Folder(path.clone());
                                let current = glyph.clone();
                                cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    this.toggle_icon_picker(target.clone(), &current, window, cx);
                                })
                            })
                            .child(self.template_icon_glyph(&glyph, px(16.0)))
                            .children(self.render_icon_picker(
                                &super::icon_picker::IconTarget::Folder(path.clone()),
                                &glyph,
                                cx,
                            )),
                    )
                    .child(
                        // The name input sized to its text (`whitespace-pre` twin).
                        div()
                            .id("folder-name")
                            .relative()
                            .min_w_0()
                            .max_w_full()
                            .cursor_text()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                                focus_name.read(cx).focus_handle(cx).focus(window);
                            }))
                            .child(
                                div()
                                    .invisible()
                                    .tw_text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .whitespace_nowrap()
                                    .child(SharedString::from(format!(
                                        "{} ",
                                        if draft.is_empty() {
                                            "Folder name"
                                        } else {
                                            draft.as_str()
                                        }
                                    ))),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .flex()
                                    .items_center()
                                    .tw_text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .when(busy, |input| input.opacity(0.5))
                                    .child(name_input),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(
                        // `ResourceShareButton`: ghost `size="sm"` with the share glyph.
                        div()
                            .id("folder-share")
                            .flex()
                            .h(px(32.0))
                            .items_center()
                            .gap(px(6.0))
                            .px_3()
                            .rounded(px(8.0))
                            .tw_text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.muted_foreground)
                            .cursor_pointer()
                            .hover(move |style| style.bg(theme.accent).text_color(theme.foreground))
                            .child(icon("share-network", px(16.0), theme.muted_foreground))
                            .child("Share"),
                    )
                    .child(self.render_folder_actions(editor, cx)),
            );

        let body = div()
            .id("folder-body")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_3()
            .pt_3()
            .pb_6()
            .child(
                div()
                    .flex()
                    .max_w(px(672.0))
                    .flex_col()
                    .gap_6()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .tw_text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(theme.foreground)
                                    .child("Context"),
                            )
                            .child(
                                div()
                                    .tw_text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("What these notes are usually about"),
                            )
                            .child(
                                // `FolderInstructionsField rows={4}`: `rounded-md border-border/60 px-3 py-2.5 text-sm leading-5`.
                                div()
                                    .id("folder-instructions")
                                    .w_full()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(alpha(theme.border, 0.6))
                                    .px_3()
                                    .py(px(10.0))
                                    .cursor_text()
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .on_click({
                                        let area = editor.instructions.clone();
                                        move |_: &ClickEvent, window, cx| {
                                            area.update(cx, |area, cx| area.focus_end(window, cx));
                                        }
                                    })
                                    .child(editor.instructions.clone()),
                            ),
                    )
                    .child(self.render_folder_materials(editor, cx)),
            );

        div()
            .flex()
            .h_full()
            .flex_1()
            .flex_col()
            .child(header)
            .child(body)
            .into_any_element()
    }

    /// The `DotsThree` ghost button and its `align="end"` Delete menu.
    fn render_folder_actions(&self, editor: &FolderEditor, cx: &Context<Self>) -> Div {
        use super::menu::{Align, Entry, MenuSpec, Trailing};
        let theme = self.theme;
        let open = editor.actions_open;
        let busy = editor.busy;
        div()
            .relative()
            .child(
                div()
                    .id("folder-actions")
                    .flex()
                    .size(px(32.0))
                    .items_center()
                    .justify_center()
                    .rounded(px(8.0))
                    .when(open, |button| button.bg(theme.muted))
                    .when(busy, |button| button.opacity(0.5))
                    .when(!busy, |button| {
                        button
                            .cursor_pointer()
                            .hover(move |style| style.bg(theme.accent))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                if let Some(editor) = this.folder_editor_mut() {
                                    editor.actions_open = !editor.actions_open;
                                    cx.notify();
                                }
                            }))
                    })
                    .child(icon(
                        "more-horizontal",
                        px(16.0),
                        if open {
                            theme.foreground
                        } else {
                            theme.muted_foreground
                        },
                    )),
            )
            .when(open, |anchor| {
                let spec = MenuSpec {
                    id: "folder-actions-menu",
                    width: 160.0,
                    entries: vec![Entry::Item {
                        icon: None,
                        dim_icon: false,
                        label: "Delete".into(),
                        trailing: Trailing::None,
                        destructive: true,
                        on_select: (!busy).then(|| {
                            Box::new(
                                |this: &mut Workspace,
                                 _: &mut Window,
                                 cx: &mut Context<Workspace>| {
                                    if let Some(editor) = this.folder_editor_mut() {
                                        editor.deleting = true;
                                        cx.notify();
                                    }
                                },
                            ) as super::menu::Select
                        }),
                        submenu: None,
                    }],
                    open_sub: None,
                    on_hover_sub: |_, _, _| {},
                    on_close: |this, cx| {
                        if let Some(editor) = this.folder_editor_mut() {
                            editor.actions_open = false;
                            cx.notify();
                        }
                    },
                };
                anchor.child(
                    div()
                        .absolute()
                        .top(px(36.0))
                        .right_0()
                        .child(self.render_menu_inline(spec, Align::End, cx)),
                )
            })
    }

    /// Materials: the dashed Add file tile and one `aspect-[4/3]` tile per file.
    fn render_folder_materials(&self, editor: &FolderEditor, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let busy = editor.busy;
        let tile = |content: gpui::Stateful<Div>| {
            // `aspect-[4/3]` inside a three-column `gap-2` grid of a 672px column.
            content
                .w_full()
                .h(px((672.0 - 16.0) / 3.0 * 0.75))
                .rounded_lg()
        };
        let mut tiles: Vec<AnyElement> = vec![
            tile(
                div()
                    .id("folder-add-file")
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(6.0))
                    .border_1()
                    .border_dashed()
                    .border_color(theme.border)
                    .text_color(theme.muted_foreground)
                    .when(busy, |button| button.opacity(0.5))
                    .when(!busy, |button| {
                        button
                            .cursor_pointer()
                            .hover(move |style| style.bg(theme.accent).text_color(theme.foreground))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.pick_folder_material(window, cx);
                            }))
                    })
                    .child(icon("plus", px(24.0), theme.muted_foreground))
                    .child(
                        div()
                            .tw_text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child("Add file"),
                    ),
            )
            .into_any_element(),
        ];
        for material in &editor.materials {
            let attachment_id = material.attachment_id().to_string();
            tiles.push(
                tile(
                    div()
                        .id(SharedString::from(format!("material-{}", material.id)))
                        .relative()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(6.0))
                        .border_1()
                        .border_color(theme.border)
                        .px_3()
                        .py_2()
                        .child(icon("file", px(24.0), theme.muted_foreground))
                        .child(
                            div()
                                .w_full()
                                .truncate()
                                .text_center()
                                .tw_text_xs()
                                .text_color(theme.foreground)
                                .child(SharedString::from(material.filename.clone())),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!(
                                    "material-remove-{}",
                                    material.id
                                )))
                                .absolute()
                                .top(px(6.0))
                                .right(px(6.0))
                                .flex()
                                .size(px(24.0))
                                .items_center()
                                .justify_center()
                                .rounded(px(8.0))
                                .when(busy, |button| button.opacity(0.5))
                                .when(!busy, |button| {
                                    button
                                        .cursor_pointer()
                                        .hover(move |style| style.bg(theme.accent))
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation()
                                        })
                                        .on_click(cx.listener(
                                            move |this, _: &ClickEvent, window, cx| {
                                                this.remove_folder_material(
                                                    attachment_id.clone(),
                                                    window,
                                                    cx,
                                                );
                                            },
                                        ))
                                })
                                .child(icon("x", px(12.0), theme.muted_foreground)),
                        ),
                )
                .into_any_element(),
            );
        }
        let rows: Vec<AnyElement> = tiles
            .chunks_mut(3)
            .map(|chunk| {
                let mut row = div().flex().gap_2();
                for tile in chunk.iter_mut() {
                    row = row.child(
                        div()
                            .flex_1()
                            .child(std::mem::replace(tile, div().into_any_element())),
                    );
                }
                let missing = 3 - chunk.len();
                for _ in 0..missing {
                    row = row.child(div().flex_1());
                }
                row.into_any_element()
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .tw_text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .child("Materials"),
            )
            .child(div().flex().flex_col().gap_2().children(rows))
    }

    /// `FolderNameDialog` and `DestructiveConfirmationDialog` on the glass
    /// overlay (`bg-black/40`, 320px `rounded-[26px] p-5` card).
    pub(super) fn render_folder_dialogs(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let state = self.folders.as_ref()?;
        let theme = self.theme;
        let glass_button =
            |id: &'static str, label: &'static str, variant: GlassButton, disabled: bool| {
                let (background, border, text) = match variant {
                    GlassButton::Cancel => (
                        alpha(theme.background, 0.5),
                        Some(alpha(theme.border, 0.7)),
                        theme.foreground,
                    ),
                    GlassButton::Primary => (theme.primary, None, theme.primary_foreground),
                    GlassButton::Destructive => (theme.destructive, None, gpui::rgb(0xffffff)),
                };
                div()
                    .id(id)
                    .flex()
                    .h(px(32.0))
                    .items_center()
                    .justify_center()
                    .px_4()
                    .rounded(px(8.0))
                    .bg(background)
                    .when_some(border, |button, border| {
                        button.border_1().border_color(border)
                    })
                    .tw_text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(text)
                    .when(disabled, |button| button.opacity(0.5))
                    .when(!disabled, |button| button.cursor_pointer())
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(label)
            };
        let card = |children: Vec<AnyElement>| {
            div()
                .id("glass-card")
                .w(px(320.0))
                .flex()
                .flex_col()
                .gap_4()
                .rounded(px(26.0))
                .p_5()
                .border_1()
                .border_color(alpha(theme.border, 0.45))
                .bg(crate::theme::glass_card_fill(theme))
                .shadow(vec![gpui::BoxShadow {
                    color: gpui::hsla(0.0, 0.0, 0.0, 0.32),
                    offset: gpui::point(px(0.0), px(24.0)),
                    blur_radius: px(70.0),
                    spread_radius: px(0.0),
                }])
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .children(children)
        };
        let title = |text: &'static str| {
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
                        .child(text),
                )
        };

        let content: AnyElement = if let Some(dialog) = &state.creating {
            let input = dialog.input.clone();
            let busy = dialog.busy;
            card(vec![
                title("New folder").into_any_element(),
                div()
                    .flex()
                    .flex_col()
                    .child(
                        // `Input`: `h-9 rounded-md border px-3 md:text-sm`.
                        div()
                            .id("new-folder-input")
                            .flex()
                            .h(px(36.0))
                            .w_full()
                            .items_center()
                            .rounded_md()
                            .border_1()
                            .border_color(theme.border)
                            .px_3()
                            .tw_text_sm()
                            .cursor_text()
                            .when(busy, |field| field.opacity(0.5))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                                input.read(cx).focus_handle(cx).focus(window);
                            }))
                            .child(div().min_w_0().flex_1().child(dialog.input.clone())),
                    )
                    .when_some(dialog.error, |form, error| {
                        form.child(
                            div()
                                .pt_2()
                                .text_center()
                                .tw_text_xs()
                                .text_color(theme.destructive)
                                .child(error),
                        )
                    })
                    .child(
                        div()
                            .mt_4()
                            .flex()
                            .gap_2()
                            .child(
                                div().flex_1().child(
                                    glass_button(
                                        "new-folder-cancel",
                                        "Cancel",
                                        GlassButton::Cancel,
                                        busy,
                                    )
                                    .on_click(cx.listener(
                                        |this, _: &ClickEvent, _, cx| {
                                            this.close_new_folder_dialog(cx);
                                        },
                                    )),
                                ),
                            )
                            .child(
                                div().flex_1().child(
                                    glass_button(
                                        "new-folder-create",
                                        "Create",
                                        GlassButton::Primary,
                                        busy,
                                    )
                                    .on_click(cx.listener(
                                        |this, _: &ClickEvent, window, cx| {
                                            this.submit_new_folder(window, cx);
                                        },
                                    )),
                                ),
                            ),
                    )
                    .into_any_element(),
            ])
            .into_any_element()
        } else if let Some(editor) = state.editor.as_ref().filter(|editor| editor.deleting) {
            let busy = editor.busy;
            card(vec![
                title("Delete folder")
                    .child(
                        div()
                            .w_full()
                            .text_center()
                            .text_size(px(13.0))
                            .line_height(px(17.0))
                            .text_color(theme.foreground)
                            .child("Notes stay in All notes. This folder, its nested folders, and all their materials will be deleted."),
                    )
                    .into_any_element(),
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div().flex_1().child(
                            glass_button("delete-folder-cancel", "Cancel", GlassButton::Cancel, busy)
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    if let Some(editor) = this.folder_editor_mut() {
                                        editor.deleting = false;
                                        cx.notify();
                                    }
                                })),
                        ),
                    )
                    .child(
                        div().flex_1().child(
                            glass_button("delete-folder-confirm", "Delete folder", GlassButton::Destructive, busy)
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    this.confirm_delete_folder(window, cx);
                                })),
                        ),
                    )
                    .into_any_element(),
            ])
            .into_any_element()
        } else {
            return None;
        };

        Some(
            gpui::deferred(
                div()
                    .id("glass-overlay")
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
                            this.close_new_folder_dialog(cx);
                            if let Some(editor) = this.folder_editor_mut() {
                                editor.deleting = false;
                            }
                            cx.notify();
                        }),
                    )
                    .child(content),
            )
            .with_priority(5)
            .into_any_element(),
        )
    }
}

#[derive(Clone, Copy)]
enum GlassButton {
    Cancel,
    Primary,
    Destructive,
}
