//! The folders route: `FoldersSidebar` in the left panel and `FolderEditor` (name, icon,
//! context and materials) in the main surface.

use std::sync::Arc;

use desktop_runtime::{CancellationToken, Reply, RuntimeHandle};
use gpui::{
    Anchor, AnyElement, Context, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent,
    PathPromptOptions, Render, SharedString, Subscription, Window, div, prelude::*, px,
};

use super::{
    folders::{self, Library, Material, MaterialCommand, Update},
    icon_picker::{IconPicker, IconPickerEvent},
    resource_list::{
        self as list, Glyph, MenuOwner, dropdown, floating_menu, floating_menu_body,
        ghost_icon_button, icon, menu_item,
    },
};
use crate::{
    product::{
        cloud::resources::ResourceKind,
        sharing::{
            resource_dialog::{ResourceDialogEvent, ResourceShareDialog},
            shared_library::SharedLibrary,
        },
    },
    ui::{
        input::{InputEvent, TextInput},
        theme::{Theme, theme},
    },
};

pub enum FoldersEvent {
    Back,
    SignIn,
    Navigate(crate::workspace::navigation::Route),
}

impl EventEmitter<FoldersEvent> for FoldersView {}

/// A confirmation dialog over the route.
enum Dialog {
    Create {
        input: Entity<TextInput>,
        error: Option<&'static str>,
    },
    Delete(Arc<str>),
}

/// The selected folder's fields.
struct Editor {
    path: Arc<str>,
    name: Entity<TextInput>,
    instructions: Entity<TextInput>,
    saved_instructions: Arc<str>,
    materials: Vec<Material>,
    access: Entity<crate::product::sharing::folder_access::FolderAccess>,
    _subscriptions: Vec<Subscription>,
}

pub struct FoldersView {
    runtime: RuntimeHandle,
    library: Arc<Library>,
    selected: Option<Arc<str>>,
    search: Entity<TextInput>,
    editor: Option<Editor>,
    dialog: Option<Dialog>,
    picker: Option<(Entity<IconPicker>, Subscription)>,
    /// Icons chosen but not yet read back, so the change shows immediately.
    icon_overrides: std::collections::HashMap<Arc<str>, Glyph>,
    actions_open: bool,
    signed_in: bool,
    sharing: Option<(Entity<ResourceShareDialog>, Subscription)>,
    shared_library: Entity<SharedLibrary>,
    busy: bool,
    message: String,
    cancel: CancellationToken,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl FoldersView {
    pub fn new(runtime: RuntimeHandle, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextInput::new("Search folders...", cx).inline());
        let subscription = cx.subscribe(&search, |_, _, event, cx| {
            if matches!(event, InputEvent::Changed) {
                cx.notify();
            }
        });
        let shared_library = cx.new(|cx| {
            SharedLibrary::new(runtime.clone(), ResourceKind::Folder, search.clone(), cx)
        });
        Self {
            runtime,
            library: Arc::default(),
            selected: None,
            search,
            editor: None,
            dialog: None,
            picker: None,
            icon_overrides: std::collections::HashMap::new(),
            actions_open: false,
            signed_in: false,
            sharing: None,
            shared_library,
            busy: false,
            message: String::new(),
            cancel: CancellationToken::new(),
            focus: cx.focus_handle(),
            _subscriptions: vec![subscription],
        }
    }

    /// Shipping's `ResourceShareButton` dialog for the selected folder.
    fn share(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.editor.as_ref().map(|editor| editor.path.clone()) else {
            return;
        };
        let title = folders::display_name(&path).into();
        let runtime = self.runtime.clone();
        let dialog =
            cx.new(|cx| ResourceShareDialog::new(runtime, ResourceKind::Folder, path, title, cx));
        let subscription = cx.subscribe(&dialog, |this, _, event, cx| {
            this.sharing = None;
            if let ResourceDialogEvent::Navigate(route) = event {
                cx.emit(FoldersEvent::Navigate(route.clone()));
            }
            cx.notify();
        });
        self.sharing = Some((dialog, subscription));
        cx.notify();
    }

    pub fn set_signed_in(&mut self, signed_in: bool, cx: &mut Context<Self>) {
        self.signed_in = signed_in;
        cx.notify();
    }

    pub fn activate(&mut self, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
        list::watch(
            &self.runtime,
            folders::LIBRARY_SQL,
            self.cancel.clone(),
            cx,
            |this, rows, cx| {
                match rows {
                    Ok(rows) => {
                        this.message.clear();
                        let library = folders::decode_library(&rows);
                        this.icon_overrides
                            .retain(|path, icon| library.icon(path) != *icon);
                        this.library = Arc::new(library);
                        this.sync_editor(cx);
                    }
                    Err(error) => this.message = format!("Could not load folders: {error}"),
                }
                cx.notify();
            },
        );
    }

    pub fn suspend(&mut self, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.actions_open = false;
        self.commit_instructions(cx);
    }

    /// Selects a folder by catalog row id (pinned folder tabs).
    pub fn select_id(&mut self, id: Arc<str>, cx: &mut Context<Self>) {
        let reply = self
            .runtime
            .read(CancellationToken::new(), move |services| async move {
                let rows = services
                    .executor
                    .execute(
                        "SELECT path FROM folders WHERE id=? AND deleted_at IS NULL".into(),
                        vec![serde_json::json!(id)],
                    )
                    .await
                    .map_err(super::mutations::failure)?;
                Ok(rows
                    .first()
                    .and_then(|row| row["path"].as_str())
                    .map(Arc::<str>::from))
            });
        self.run(
            reply,
            |this, path, cx| {
                if let Some(path) = path {
                    this.select(path, cx);
                }
            },
            cx,
        );
    }

    fn icon(&self, path: &str) -> Glyph {
        self.icon_overrides
            .get(path)
            .cloned()
            .unwrap_or_else(|| self.library.icon(path))
    }

    fn open_picker(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else { return };
        if self.picker.take().is_some() {
            cx.notify();
            return;
        }
        let path = editor.path.clone();
        let picker = cx.new(|cx| IconPicker::new(self.icon(&path), cx));
        let subscription = cx.subscribe(&picker, move |this, _, event, cx| match event {
            IconPickerEvent::Changed { icon, close } => {
                this.icon_overrides.insert(path.clone(), icon.clone());
                let reply =
                    folders::update(&this.runtime, path.clone(), Update::Icon(icon.clone()));
                this.run(reply, |_, (), _| {}, cx);
                if *close {
                    this.picker = None;
                }
                cx.notify();
            }
        });
        self.picker = Some((picker, subscription));
        cx.notify();
    }

    /// `useActiveFolderPath`: the chosen folder, else the first one.
    fn active(&self) -> Option<Arc<str>> {
        self.selected
            .clone()
            .filter(|path| self.library.paths.contains(path))
            .or_else(|| self.library.paths.first().cloned())
    }

    fn select(&mut self, path: Arc<str>, cx: &mut Context<Self>) {
        self.selected = Some(path);
        self.actions_open = false;
        self.sync_editor(cx);
        cx.notify();
    }

    fn sync_editor(&mut self, cx: &mut Context<Self>) {
        let active = self.active();
        if self.editor.as_ref().map(|editor| &editor.path) == active.as_ref() {
            return;
        }
        self.commit_instructions(cx);
        self.editor = active.map(|path| self.build_editor(path, cx));
    }

    fn build_editor(&mut self, path: Arc<str>, cx: &mut Context<Self>) -> Editor {
        let name = cx.new(|cx| {
            let mut input = TextInput::new("Folder name", cx).inline();
            input.set_text(folders::display_name(&path).into(), cx);
            input
        });
        let instructions = cx.new(|cx| {
            TextInput::new("Add context for this folder", cx)
                .inline()
                .multiline()
                .rows(4, Some(4))
        });
        let subscriptions = vec![cx.subscribe(&name, |this, _, event, cx| {
            if matches!(event, InputEvent::Submitted) {
                this.commit_name(cx);
            }
        })];
        let runtime = self.runtime.clone();
        let access_path = path.clone();
        let editor = Editor {
            path: path.clone(),
            name,
            instructions,
            saved_instructions: "".into(),
            materials: Vec::new(),
            access: cx.new(|cx| {
                crate::product::sharing::folder_access::FolderAccess::new(runtime, access_path, cx)
            }),
            _subscriptions: subscriptions,
        };
        let reply = folders::instructions(&self.runtime, path.clone());
        let for_instructions = path.clone();
        self.run(
            reply,
            move |this, text, cx| {
                if let Some(editor) = this
                    .editor
                    .as_mut()
                    .filter(|editor| editor.path == for_instructions)
                {
                    editor.saved_instructions = text.clone();
                    editor
                        .instructions
                        .update(cx, |input, cx| input.set_text(text.to_string(), cx));
                }
            },
            cx,
        );
        self.refresh_materials(path, cx);
        editor
    }

    fn refresh_materials(&mut self, path: Arc<str>, cx: &mut Context<Self>) {
        let reply = folders::materials(&self.runtime, path.clone());
        self.run(
            reply,
            move |this, materials, _| {
                if let Some(editor) = this.editor.as_mut().filter(|editor| editor.path == path) {
                    editor.materials = materials;
                }
            },
            cx,
        );
    }

    fn run<T: 'static>(
        &mut self,
        reply: desktop_runtime::Result<Reply<T>>,
        then: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(value) => then(this, value, cx),
                    Err(error) => this.message = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The name field renames on Enter or blur; a blank or nested name restores the old one.
    fn commit_name(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else { return };
        let path = editor.path.clone();
        let current = folders::display_name(&path).to_owned();
        let draft = editor.name.read(cx).buffer.text.trim().to_owned();
        let parent = path.rsplit_once('/').map(|(parent, _)| parent);
        let renamed = match parent {
            Some(parent) => format!("{parent}/{draft}"),
            None => draft.clone(),
        };
        if draft.is_empty()
            || draft.contains('/')
            || draft == "."
            || draft == ".."
            || renamed == *path
        {
            let name = editor.name.clone();
            name.update(cx, |input, cx| input.set_text(current, cx));
            return;
        }
        self.commit_instructions(cx);
        self.busy = true;
        let reply = folders::rename(&self.runtime, path, renamed.into());
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(path) => {
                        this.editor = None;
                        this.select(path, cx);
                    }
                    Err(_) => {
                        if let Some(editor) = &this.editor {
                            editor
                                .name
                                .update(cx, |input, cx| input.set_text(current, cx));
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// `FolderInstructionsField` saves on blur when the text changed.
    fn commit_instructions(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let text: Arc<str> = editor.instructions.read(cx).buffer.text.as_str().into();
        if text == editor.saved_instructions {
            return;
        }
        editor.saved_instructions = text.clone();
        let reply = folders::update(
            &self.runtime,
            editor.path.clone(),
            Update::Instructions(text),
        );
        self.run(reply, |_, (), _| {}, cx);
    }

    fn add_material(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else { return };
        if self.busy {
            return;
        }
        let path = editor.path.clone();
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Add file".into()),
        });
        let runtime = self.runtime.clone();
        self.busy = true;
        cx.spawn(async move |this, cx| {
            let result = async {
                let paths = picker
                    .await
                    .map_err(super::mutations::failure)?
                    .map_err(super::mutations::failure)?;
                let Some(file) = paths.and_then(|paths| paths.into_iter().next()) else {
                    return Ok(());
                };
                folders::material_command(&runtime, path.clone(), MaterialCommand::Import(file))?
                    .receive()
                    .await
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                if let Err(error) = result {
                    this.message = error.to_string();
                }
                if let Some(path) = this.editor.as_ref().map(|editor| editor.path.clone()) {
                    this.refresh_materials(path, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn remove_material(&mut self, id: Arc<str>, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else { return };
        if self.busy {
            return;
        }
        let path = editor.path.clone();
        self.busy = true;
        let reply =
            folders::material_command(&self.runtime, path.clone(), MaterialCommand::Remove(id));
        self.run(
            reply,
            move |this, (), cx| {
                this.busy = false;
                this.refresh_materials(path, cx);
            },
            cx,
        );
    }

    fn open_create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| TextInput::new("", cx).inline());
        let subscription = cx.subscribe(&input, |this, _, event, cx| match event {
            InputEvent::Submitted => this.submit_create(cx),
            InputEvent::Changed => {
                if let Some(Dialog::Create { error, .. }) = &mut this.dialog {
                    *error = None;
                }
                cx.notify();
            }
            InputEvent::Rejected => {}
        });
        input.focus_handle(cx).focus(window, cx);
        self._subscriptions.truncate(1);
        self._subscriptions.push(subscription);
        self.dialog = Some(Dialog::Create { input, error: None });
        cx.notify();
    }

    fn submit_create(&mut self, cx: &mut Context<Self>) {
        let Some(Dialog::Create { input, error }) = &mut self.dialog else {
            return;
        };
        if self.busy {
            return;
        }
        let name = input.read(cx).buffer.text.trim().to_owned();
        if name.is_empty() || name.contains('/') || matches!(name.as_str(), "." | "..") {
            *error = Some("Enter a valid folder name.");
            cx.notify();
            return;
        }
        self.busy = true;
        let reply = folders::create(&self.runtime, name.into());
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(path) => {
                        this.dialog = None;
                        this.select(path, cx);
                    }
                    Err(_) => {
                        if let Some(Dialog::Create { error, .. }) = &mut this.dialog {
                            *error = Some("Could not save the folder.");
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        let Some(Dialog::Delete(path)) = &self.dialog else {
            return;
        };
        if self.busy {
            return;
        }
        let path = path.clone();
        self.busy = true;
        let reply = folders::delete(&self.runtime, path.clone());
        self.run(
            reply,
            move |this, (), cx| {
                this.busy = false;
                this.dialog = None;
                if this.selected.as_ref() == Some(&path) {
                    this.selected = None;
                }
                this.editor = None;
                this.sync_editor(cx);
            },
            cx,
        );
    }

    fn close_dialog(&mut self, cx: &mut Context<Self>) {
        if !self.busy {
            self.dialog = None;
            cx.notify();
        }
    }

    pub fn render_nav(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(window);
        let search = self.search.read(cx).buffer.text.clone();
        let query = search.trim().to_lowercase();
        let paths = self
            .library
            .paths
            .iter()
            .filter(|path| query.is_empty() || path.to_lowercase().contains(&query))
            .cloned()
            .collect::<Vec<_>>();
        let active = self.active();
        let focused = self.search.focus_handle(cx).is_focused(window);
        div()
            .id("folders-nav")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape"
                    && this.search.focus_handle(cx).is_focused(window)
                {
                    this.search
                        .update(cx, |input, cx| input.set_text(String::new(), cx));
                    cx.stop_propagation();
                }
            }))
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .pb_2()
            .child(list::sidebar_header(
                "folders-back",
                colors,
                cx.listener(|_, _, _, cx| cx.emit(FoldersEvent::Back)),
                [ghost_icon_button("folders-new", "PlusIcon", colors)
                    .on_click(cx.listener(|this, _, window, cx| this.open_create(window, cx)))
                    .into_any_element()],
            ))
            .child(list::search_field(
                &self.search,
                focused,
                !search.is_empty(),
                colors,
                cx.listener(|this, _, _, cx| {
                    this.search
                        .update(cx, |input, cx| input.set_text(String::new(), cx));
                }),
            ))
            .child(
                div()
                    .id("folders-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .pt_1()
                    .map(|view| {
                        if paths.is_empty() {
                            view.child(list::empty_list(
                                "Folder01Icon",
                                if search.is_empty() {
                                    "No folders yet"
                                } else {
                                    "No folders found"
                                },
                                colors,
                            ))
                        } else {
                            view.children(paths.into_iter().map(|path| {
                                let selected = active.as_ref() == Some(&path);
                                let glyph = self.icon(&path);
                                list::list_row(
                                    SharedString::from(format!("folder-{path}")),
                                    selected,
                                    true,
                                    colors,
                                )
                                .when(selected, |row| row.font_weight(crate::ui::theme::MEDIUM))
                                .on_click(cx.listener({
                                    let path = path.clone();
                                    move |this, _, window, cx| {
                                        this.focus.focus(window, cx);
                                        this.select(path.clone(), cx)
                                    }
                                }))
                                .child(
                                    div()
                                        .flex()
                                        .min_w_0()
                                        .items_center()
                                        .gap_2()
                                        .child(glyph.render(16.))
                                        .child(
                                            div()
                                                .min_w_0()
                                                .truncate()
                                                .child(SharedString::from(path)),
                                        ),
                                )
                            }))
                        }
                    })
                    .child(self.shared_library.clone()),
            )
            .into_any_element()
    }

    fn render_editor(
        &self,
        editor: &Editor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = theme(window);
        let path = editor.path.clone();
        let glyph = self.icon(&path);
        let name_focused = editor.name.focus_handle(cx).is_focused(window);
        let instructions_focused = editor.instructions.focus_handle(cx).is_focused(window);
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(48.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .pr_1()
                    .pl_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("folder-icon")
                                    .size(px(28.))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|style| style.bg(colors.accent))
                                    .on_click(cx.listener(|this, _, _, cx| this.open_picker(cx)))
                                    .child(glyph.render(16.))
                                    .when_some(self.picker.as_ref(), |trigger, (picker, _)| {
                                        trigger.child(list::popover(
                                            picker.clone().into_any_element(),
                                            cx,
                                        ))
                                    }),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .when(self.busy, |view| view.opacity(0.5))
                                    .on_key_down(cx.listener(
                                        |this, event: &KeyDownEvent, window, cx| {
                                            if event.keystroke.key == "escape" {
                                                if let Some(editor) = &this.editor {
                                                    let name = folders::display_name(&editor.path)
                                                        .to_owned();
                                                    editor.name.update(cx, |input, cx| {
                                                        input.set_text(name, cx)
                                                    });
                                                }
                                                this.focus.focus(window, cx);
                                                cx.stop_propagation();
                                            }
                                        },
                                    ))
                                    .when(name_focused, |view| {
                                        view.on_mouse_down_out(
                                            cx.listener(|this, _, _, cx| this.commit_name(cx)),
                                        )
                                    })
                                    .child(editor.name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(2.))
                            .child(list::share_button("folder-share", colors).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.share(cx);
                                }),
                            ))
                            .child(
                                ghost_icon_button("folder-actions", "MoreHorizontalIcon", colors)
                                    .when(self.actions_open, |button| button.bg(colors.muted))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if !this.busy {
                                            this.actions_open = !this.actions_open;
                                            cx.notify();
                                        }
                                    }))
                                    .when(self.actions_open, |button| {
                                        button.child(dropdown(
                                            Anchor::TopRight,
                                            floating_menu(128., colors).child(
                                                floating_menu_body(colors).child(
                                                    menu_item(
                                                        "delete-folder",
                                                        "Delete",
                                                        true,
                                                        false,
                                                        window,
                                                        colors,
                                                    )
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.actions_open = false;
                                                        this.dialog =
                                                            Some(Dialog::Delete(path.clone()));
                                                        cx.notify();
                                                    })),
                                                ),
                                            ),
                                            cx,
                                        ))
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .id("folder-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_3()
                    .pt_3()
                    .pb_6()
                    .child(
                        div()
                            .max_w(px(672.))
                            .flex()
                            .flex_col()
                            .gap_6()
                            .when(crate::product::account::current(cx).signed_in(), |view| {
                                view.child(editor.access.clone())
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .child(heading("Context"))
                                    .child(
                                        div()
                                            .text_xs()
                                            .line_height(px(16.))
                                            .text_color(colors.muted_foreground)
                                            .child("What these notes are usually about"),
                                    )
                                    .child(
                                        // `rows={4}` with `px-3 py-2.5 leading-5`.
                                        div()
                                            .w_full()
                                            .px_3()
                                            .py(px(10.))
                                            .line_height(px(20.))
                                            .border_1()
                                            .border_color(if instructions_focused {
                                                colors.ring
                                            } else {
                                                colors.border.opacity(0.6)
                                            })
                                            .text_sm()
                                            .when(instructions_focused, |view| {
                                                view.on_mouse_down_out(cx.listener(
                                                    |this, _, _, cx| this.commit_instructions(cx),
                                                ))
                                            })
                                            .child(editor.instructions.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(heading("Materials"))
                                    .child(self.render_materials(editor, colors, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_materials(
        &self,
        editor: &Editor,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tile = || {
            div()
                .w_full()
                .aspect_ratio(4. / 3.)
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .border_1()
                .border_color(colors.border)
        };
        div()
            .grid()
            .grid_cols(3)
            .gap_2()
            .child(
                tile()
                    .id("add-material")
                    .border_dashed()
                    .text_color(colors.muted_foreground)
                    .cursor_pointer()
                    .when(self.busy, |tile| tile.opacity(0.5))
                    .hover(|style| style.bg(colors.accent).text_color(colors.foreground))
                    .on_click(cx.listener(|this, _, _, cx| this.add_material(cx)))
                    .child(icon("PlusIcon", 24., colors.muted_foreground))
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(16.))
                            .font_weight(crate::ui::theme::MEDIUM)
                            .child("Add file"),
                    ),
            )
            .children(
                editor
                    .materials
                    .iter()
                    .enumerate()
                    .map(|(index, material)| {
                        let id = material.id.clone();
                        tile()
                            .relative()
                            .px_3()
                            .py_2()
                            .child(icon("File01Icon", 24., colors.muted_foreground))
                            .child(
                                div()
                                    .w_full()
                                    .truncate()
                                    .text_center()
                                    .text_xs()
                                    .line_height(px(16.))
                                    .child(SharedString::from(material.name.clone())),
                            )
                            .child(
                                div()
                                    .id(("remove-material", index))
                                    .absolute()
                                    .top(px(6.))
                                    .right(px(6.))
                                    .size(px(24.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .hover(|style| style.bg(colors.accent))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.remove_material(id.clone(), cx)
                                    }))
                                    .child(icon("XIcon", 12., colors.muted_foreground)),
                            )
                    }),
            )
            .into_any_element()
    }

    fn render_dialog(
        &self,
        dialog: &Dialog,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = theme(window);
        let (title, body, confirm): (&str, AnyElement, &str) = match dialog {
            Dialog::Create { input, error } => (
                "New folder",
                div()
                    .child(
                        div()
                            .h(px(36.))
                            .px_3()
                            .flex()
                            .items_center()
                            .border_1()
                            .border_color(if input.focus_handle(cx).is_focused(window) {
                                colors.ring
                            } else {
                                colors.input
                            })
                            .shadow_xs()
                            .text_sm()
                            .child(input.clone()),
                    )
                    .when_some(*error, |view, error| {
                        view.child(
                            div()
                                .pt_2()
                                .text_center()
                                .text_xs()
                                .text_color(colors.destructive)
                                .child(error),
                        )
                    })
                    .into_any_element(),
                "Create",
            ),
            Dialog::Delete(_) => (
                "Delete folder",
                div()
                    .w_full()
                    .text_center()
                    .text_size(px(13.))
                    .line_height(px(17.7))
                    .child("Notes stay in All notes. This folder, its nested folders, and all their materials will be deleted.")
                    .into_any_element(),
                "Delete folder",
            ),
        };
        let destructive = matches!(dialog, Dialog::Delete(_));
        let mut body = Some(body);
        list::glass_dialog(colors)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    this.close_dialog(cx);
                    cx.stop_propagation();
                }
            }))
            .on_mouse_down_out(cx.listener(|this, _, _, cx| this.close_dialog(cx)))
            // The delete description sits in the dialog header; the name field follows it.
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_center()
                            .text_size(px(13.))
                            .line_height(px(20.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .when(destructive, |header| header.children(body.take())),
            )
            .children(body)
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap_2()
                    .child(
                        list::dialog_button("dialog-cancel", "Cancel", false, colors)
                            .on_click(cx.listener(|this, _, _, cx| this.close_dialog(cx))),
                    )
                    .child(
                        list::dialog_button("dialog-confirm", confirm, destructive, colors)
                            .when(self.busy, |button| button.opacity(0.5))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if matches!(this.dialog, Some(Dialog::Create { .. })) {
                                    this.submit_create(cx)
                                } else {
                                    this.confirm_delete(cx)
                                }
                            })),
                    ),
            )
            .into_any_element()
    }
}

fn heading(text: &'static str) -> gpui::Div {
    div()
        .text_sm()
        .line_height(px(20.))
        .font_weight(crate::ui::theme::MEDIUM)
        .child(text)
}

impl MenuOwner for FoldersView {
    fn close_menu(&mut self) {
        self.actions_open = false;
        self.picker = None;
    }
}

impl Drop for FoldersView {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl Render for FoldersView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let content = match &self.editor {
            Some(editor) => self.render_editor(editor, window, cx),
            None => div()
                .size_full()
                .px_6()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(colors.muted_foreground)
                .child("No folders yet. Create one to group notes and materials.")
                .into_any_element(),
        };
        div()
            .relative()
            .size_full()
            .child(content)
            .when(!self.message.is_empty(), |view| {
                view.child(
                    div()
                        .absolute()
                        .bottom_2()
                        .left_3()
                        .text_xs()
                        .text_color(colors.destructive)
                        .child(self.message.clone()),
                )
            })
            .when_some(self.dialog.as_ref(), |view, dialog| {
                view.child(list::modal(window, self.render_dialog(dialog, window, cx)))
            })
            .when_some(self.sharing.as_ref(), |view, (dialog, _)| {
                view.child(list::modal(window, dialog.clone().into_any_element()))
            })
    }
}
