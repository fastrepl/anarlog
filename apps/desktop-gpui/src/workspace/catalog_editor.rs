use std::sync::Arc;

use desktop_runtime::{CancellationToken, Generation, RuntimeHandle};
use gpui::{
    Context, Entity, EventEmitter, Focusable, KeyDownEvent, Render, SharedString, Subscription,
    Window, div, prelude::*, px,
};
use serde_json::json;

use super::{
    mutations::{self, Command, Draft},
    ports::{Catalog, CatalogRow},
};
use crate::ui::{
    input::{InputEvent, TextInput},
    theme::theme,
};

pub struct EditorSaved(pub Option<CatalogRow>);

pub struct CatalogEditor {
    runtime: RuntimeHandle,
    catalog: Catalog,
    draft: Option<Draft>,
    inputs: Vec<Entity<TextInput>>,
    group_inputs: Vec<Vec<Entity<TextInput>>>,
    merge_target: Entity<TextInput>,
    dirty: bool,
    busy: bool,
    confirm_delete: bool,
    message: String,
    viewer: Option<Arc<str>>,
    generation: Generation,
    cancel: CancellationToken,
    subscriptions: Vec<Subscription>,
    session_target: Entity<TextInput>,
    automation_client: Option<super::automation_runner::AutomationClient>,
    edit_revision: u64,
    targets: Vec<(Arc<str>, Arc<str>)>,
    targets_are_contacts: bool,
    confirm_retry: bool,
    pending_action: Option<EditorAction>,
}

impl EventEmitter<EditorSaved> for CatalogEditor {}
impl EventEmitter<super::library::OpenNote> for CatalogEditor {}

impl CatalogEditor {
    pub fn new(runtime: RuntimeHandle, catalog: Catalog, cx: &mut Context<Self>) -> Self {
        Self {
            runtime,
            catalog,
            draft: None,
            inputs: Vec::new(),
            group_inputs: Vec::new(),
            merge_target: cx.new(|cx| TextInput::new("Duplicate contact ID", cx)),
            dirty: false,
            busy: false,
            confirm_delete: false,
            message: String::new(),
            viewer: None,
            generation: Generation::default(),
            cancel: CancellationToken::new(),
            subscriptions: Vec::new(),
            session_target: cx.new(|cx| TextInput::new("Note ID", cx)),
            automation_client: None,
            edit_revision: 0,
            targets: Vec::new(),
            targets_are_contacts: false,
            confirm_retry: false,
            pending_action: None,
        }
    }

    pub fn blocked(&self) -> bool {
        self.dirty || self.busy
    }

    fn choose_target(&mut self, contacts: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let input = if contacts {
            &self.merge_target
        } else {
            &self.session_target
        };
        let query = input.read(cx).buffer.text.trim().to_owned();
        let selected = self.draft.as_ref().map(|draft| draft.row.id.clone());
        let query = format!("%{}%", super::ports::escape_like(&query));
        let reply=self.runtime.read(CancellationToken::new(),move |services| async move {
            let sql=if contacts {"SELECT id,name AS title FROM humans WHERE deleted_at IS NULL AND id IS NOT ?2 AND (name LIKE ?1 ESCAPE '\\' OR email LIKE ?1 ESCAPE '\\') ORDER BY name,id LIMIT 30"} else {"SELECT id,title FROM sessions WHERE deleted_at IS NULL AND title LIKE ?1 ESCAPE '\\' AND id IS NOT ?2 ORDER BY created_at DESC,id LIMIT 30"};
            services.executor.execute(sql.into(),vec![json!(query),json!(selected)]).await.map_err(mutations::failure)
        });
        self.busy = true;
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(rows) => {
                        this.targets = rows
                            .iter()
                            .filter_map(|row| {
                                Some((
                                    row["id"].as_str()?.into(),
                                    row["title"].as_str().unwrap_or("Untitled").into(),
                                ))
                            })
                            .collect();
                        this.targets_are_contacts = contacts;
                        this.message = if this.targets.is_empty() {
                            "No matching records. Clear the search and choose again.".into()
                        } else {
                            "Choose a record below.".into()
                        };
                    }
                    Err(error) => this.message = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn retry(&mut self, cx: &mut Context<Self>) {
        if self.blocked() {
            return;
        }
        if !self.confirm_retry {
            self.confirm_retry = true;
            self.message =
                "Verify every destination first. Allowing a retry can duplicate external delivery."
                    .into();
            cx.notify();
            return;
        }
        let Some(draft) = &self.draft else {
            return;
        };
        let session = self.session_target.read(cx).buffer.text.trim().to_owned();
        if session.is_empty() {
            self.message = "Choose a note first.".into();
            cx.notify();
            return;
        }
        let reply = super::automation_runner::allow_retry(
            &self.runtime,
            draft.row.id.clone(),
            desktop_runtime::SessionId(session.into()),
        );
        self.busy = true;
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.confirm_retry = false;
                this.message = match result {
                    Ok(()) => "Retry allowed. Run for note to deliver again.".into(),
                    Err(error) => error.to_string(),
                };
                cx.notify();
            });
        })
        .detach();
    }
    pub fn set_viewer(&mut self, viewer: Option<Arc<str>>) {
        self.viewer = viewer;
    }

    pub fn set_automation_client(
        &mut self,
        client: Option<super::automation_runner::AutomationClient>,
    ) {
        self.automation_client = client;
    }

    fn use_for_note(&mut self, cx: &mut Context<Self>) {
        if self.blocked() {
            self.message = "Save or discard edits first.".into();
            cx.notify();
            return;
        }
        let Some(draft) = &self.draft else {
            return;
        };
        let session = desktop_runtime::SessionId(
            self.session_target
                .read(cx)
                .buffer
                .text
                .trim()
                .to_owned()
                .into(),
        );
        if session.0.is_empty() {
            self.message = "Choose a note ID.".into();
            cx.notify();
            return;
        }
        let reply = super::automation_runner::run(
            &self.runtime,
            draft.row.id.clone(),
            session,
            self.automation_client.clone(),
        );
        self.busy = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.message = match result {
                    Ok(message) => message,
                    Err(error) => format!("Action failed: {error}"),
                };
                cx.notify();
            });
        })
        .detach();
    }

    pub fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.blocked() {
            self.message = "Save or discard the current draft first.".into();
            cx.notify();
            return;
        }
        self.cancel.cancel();
        self.generation.advance();
        self.install(mutations::blank(self.catalog), cx);
        self.dirty = true;
        if let Some(input) = self.inputs.first() {
            input.focus_handle(cx).focus(window, cx);
        }
    }

    fn install(&mut self, draft: Draft, cx: &mut Context<Self>) {
        self.inputs.clear();
        self.subscriptions.clear();
        self.group_inputs.clear();
        for group in draft.groups.iter().flatten() {
            let mut inputs = Vec::new();
            for field in &group.fields {
                let input = cx.new(|cx| {
                    let mut input = TextInput::new(field.label, cx);
                    if matches!(
                        field.key,
                        "prompt"
                            | "content"
                            | "description"
                            | "system_prompt"
                            | "body"
                            | "body_text"
                            | "memo"
                            | "instructions"
                            | "sections_json"
                            | "targets_json"
                            | "steps"
                    ) {
                        input = input.multiline();
                    }
                    input.set_text(field.value.clone(), cx);
                    input
                });
                self.subscriptions
                    .push(cx.subscribe(&input, |this, _, event, cx| {
                        if matches!(event, InputEvent::Changed) {
                            this.dirty = true;
                            this.edit_revision = this.edit_revision.wrapping_add(1);
                            if let Some(draft) = &mut this.draft {
                                draft.groups_dirty = true;
                            }
                        } else if matches!(event, InputEvent::Rejected) {
                            this.message = "Input rejected; original draft retained.".into();
                        }
                        cx.notify();
                    }));
                inputs.push(input);
            }
            self.group_inputs.push(inputs);
        }
        for field in &draft.fields {
            let input = cx.new(|cx| {
                let mut input = TextInput::new(field.label, cx);
                if matches!(
                    field.key,
                    "prompt"
                        | "content"
                        | "description"
                        | "system_prompt"
                        | "body"
                        | "body_text"
                        | "memo"
                        | "instructions"
                        | "sections_json"
                        | "targets_json"
                        | "steps"
                ) {
                    input = input.multiline();
                }
                input.set_text(field.value.clone(), cx);
                input
            });
            self.subscriptions
                .push(cx.subscribe(&input, |this, _, event, cx| {
                    match event {
                        InputEvent::Changed => {
                            this.edit_revision = this.edit_revision.wrapping_add(1);
                            this.dirty = true;
                            this.message.clear();
                        }
                        InputEvent::Rejected => {
                            this.message = "Input rejected; original draft retained.".into()
                        }
                        InputEvent::Submitted => {
                            this.act(EditorAction::Save, cx);
                            return;
                        }
                    }
                    cx.notify();
                }));
            self.inputs.push(input);
        }
        self.draft = Some(draft);
        self.dirty = false;
        self.message.clear();
        self.confirm_delete = false;
        cx.notify();
    }

    pub fn load(&mut self, row: CatalogRow, cx: &mut Context<Self>) {
        if self.blocked() {
            return;
        }
        if self
            .draft
            .as_ref()
            .is_some_and(|draft| draft.row.id == row.id && draft.row.kind == row.kind)
        {
            return;
        }
        self.reload(row, cx);
    }

    pub(super) fn open_action(
        &mut self,
        row: CatalogRow,
        action: EditorAction,
        cx: &mut Context<Self>,
    ) {
        if self.blocked() {
            return;
        }
        if self
            .draft
            .as_ref()
            .is_some_and(|draft| draft.row.id == row.id && draft.row.kind == row.kind)
        {
            self.act(action, cx);
        } else {
            self.pending_action = Some(action);
            self.reload(row, cx);
        }
    }

    fn reload(&mut self, row: CatalogRow, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
        let generation = self.generation.advance();
        let reply = mutations::load(&self.runtime, row, self.cancel.clone());
        self.message = "Loading…".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation || this.blocked() {
                    return;
                }
                match result {
                    Ok(draft) => {
                        this.install(draft, cx);
                        if let Some(action) = this.pending_action.take() {
                            this.act(action, cx);
                        }
                    }
                    Err(error) => {
                        this.message = format!("Could not load: {error}");
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn snapshot(&self, cx: &Context<Self>) -> Option<Draft> {
        let mut draft = self.draft.clone()?;
        for (field, input) in draft.fields.iter_mut().zip(&self.inputs) {
            field.value = input.read(cx).buffer.text.clone();
        }
        for (group, inputs) in draft.groups.iter_mut().flatten().zip(&self.group_inputs) {
            for (field, input) in group.fields.iter_mut().zip(inputs) {
                field.value = input.read(cx).buffer.text.clone();
            }
        }
        Some(draft)
    }

    fn change_group(&mut self, add: Option<&str>, remove: Option<usize>, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(mut draft) = self.snapshot(cx) else {
            return;
        };
        let Some(groups) = &mut draft.groups else {
            return;
        };
        if let Some(kind) = add {
            if groups.len() >= 64 {
                self.message = "Maximum 64 sections or steps.".into();
                cx.notify();
                return;
            }
            groups.push(mutations::new_group(kind));
        }
        if let Some(index) = remove {
            groups.remove(index);
        }
        draft.groups_dirty = true;
        self.install(draft, cx);
        self.dirty = true;
        self.edit_revision = self.edit_revision.wrapping_add(1);
    }

    fn set_field(&mut self, key: &str, value: &str, cx: &mut Context<Self>) {
        if let Some(index) = self
            .draft
            .as_ref()
            .and_then(|draft| draft.fields.iter().position(|field| field.key == key))
        {
            self.inputs[index].update(cx, |input, cx| input.set_text(value.into(), cx));
            self.dirty = true;
            self.edit_revision = self.edit_revision.wrapping_add(1);
            cx.notify();
        }
    }

    fn toggle_export(&mut self, index: usize, key: &'static str, cx: &mut Context<Self>) {
        let Some(draft) = &mut self.draft else {
            return;
        };
        let Some(group) = draft
            .groups
            .as_mut()
            .and_then(|groups| groups.get_mut(index))
        else {
            return;
        };
        let enabled = group.base["options"][key].as_bool().unwrap_or(true);
        if !group.base["options"].is_object() {
            group.base["options"] = json!({});
        }
        group.base["options"][key] = json!(!enabled);
        draft.groups_dirty = true;
        self.dirty = true;
        self.edit_revision = self.edit_revision.wrapping_add(1);
        cx.notify();
    }

    fn act(&mut self, action: EditorAction, cx: &mut Context<Self>) {
        if self
            .inputs
            .iter()
            .chain(self.group_inputs.iter().flatten())
            .any(|input| input.read(cx).buffer.marked.is_some())
        {
            self.message = "Finish composing text before saving or discarding.".into();
            cx.notify();
            return;
        }
        if self.busy {
            return;
        }
        let Some(draft) = self.snapshot(cx) else {
            return;
        };
        if matches!(action, EditorAction::Discard) {
            self.dirty = false;
            if draft.base.is_none() {
                self.draft = None;
                self.inputs.clear();
                self.subscriptions.clear();
            } else {
                self.reload(draft.row, cx);
            }
            self.confirm_delete = false;
            cx.notify();
            return;
        }
        if self.dirty && !matches!(action, EditorAction::Save) {
            self.message = "Save or discard edits first.".into();
            cx.notify();
            return;
        }
        if matches!(action, EditorAction::Delete) && !self.confirm_delete {
            self.confirm_delete = true;
            cx.notify();
            return;
        }
        let deleting = matches!(action, EditorAction::Delete);
        let command = match action {
            EditorAction::Save => Command::Save(draft),
            EditorAction::Duplicate => Command::Duplicate(draft),
            EditorAction::Pin => Command::Pin(draft),
            EditorAction::Delete => Command::Delete(draft),
            EditorAction::Merge => Command::Merge {
                primary: draft,
                duplicate: self.merge_target.read(cx).buffer.text.trim().into(),
            },
            EditorAction::Discard => return,
        };
        let reply = mutations::dispatch(&self.runtime, command, self.viewer.clone());
        let revision = self.edit_revision;
        let runtime = self.runtime.clone();
        self.busy = true;
        self.message = "Saving…".into();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let refreshed = match &result {
                Ok(row) if !deleting => match mutations::load(&runtime, row.clone(), CancellationToken::new()) {
                    Ok(reply) => reply.receive().await.ok(),
                    Err(_) => None,
                },
                _ => None,
            };
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                match result {
                    Ok(row) => {
                        this.dirty = revision != this.edit_revision;
                        this.confirm_delete = false;
                        cx.emit(EditorSaved((!deleting).then_some(row.clone())));
                        if deleting {
                            this.draft = None;
                            this.inputs.clear();
                            this.subscriptions.clear();
                            this.message = "Deleted.".into();
                        } else if let Some(refreshed) = refreshed {
                            if this.dirty {
                                if let Some(draft) = &mut this.draft {
                                    draft.row = refreshed.row;
                                    draft.base = refreshed.base;
                                }
                                this.message = "Saved. Newer edits remain unsaved.".into();
                            } else { this.install(refreshed, cx); }
                        } else {
                            this.message = "Saved, but refreshing the revision failed. Reload before saving again.".into();
                        }
                    }
                    Err(error) => {
                        this.message = format!("Not saved: {error}. Your draft is retained.")
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

#[derive(Clone, Copy)]
pub(super) enum EditorAction {
    Save,
    Discard,
    Duplicate,
    Delete,
    Pin,
    Merge,
}

impl Drop for CatalogEditor {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl Render for CatalogEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let existing = self
            .draft
            .as_ref()
            .is_some_and(|draft| draft.base.is_some());
        let kind = self
            .draft
            .as_ref()
            .map(|draft| draft.row.kind.clone())
            .unwrap_or_default();
        div()
            .id("catalog-editor")
            .flex()
            .flex_col()
            .gap_4()
            .w_full()
            .max_w(px(720.))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.modifiers.secondary(cx) && event.keystroke.key == "s" {
                    this.act(EditorAction::Save, cx);
                    cx.stop_propagation();
                }
                if event.keystroke.key == "escape" && this.confirm_delete {
                    this.confirm_delete = false;
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_xl().child(self.catalog.label()))
                    .child(
                        div()
                            .id("catalog-create")
                            .cursor_pointer()
                            .rounded(px(6.))
                            .px_3()
                            .py_1()
                            .bg(colors.accent)
                            .child("+ New")
                            .on_click(cx.listener(|this, _, window, cx| this.create(window, cx))),
                    ),
            )
            .when_some(self.draft.clone(), |view, draft| {
                view.children(
                    draft
                        .fields
                        .iter()
                        .zip(&self.inputs)
                        .filter(|(field, _)| {
                            draft.groups.is_none()
                                || !matches!(field.key, "sections_json" | "steps")
                        })
                        .map(|(field, input)| {
                            let current = input.read(cx).buffer.text.clone();
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(colors.muted_foreground)
                                        .child(field.label),
                                )
                                .when(!matches!(field.key, "enabled" | "trigger"), |view| {
                                    view.child(input.clone())
                                })
                                .when(field.key == "enabled", |view| {
                                    let enabled = current == "true";
                                    view.child(
                                        div()
                                            .id("workflow-enabled")
                                            .cursor_pointer()
                                            .child(if enabled {
                                                "[x] Enabled"
                                            } else {
                                                "[ ] Disabled"
                                            })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.set_field(
                                                    "enabled",
                                                    if enabled { "false" } else { "true" },
                                                    cx,
                                                )
                                            })),
                                    )
                                })
                                .when(field.key == "trigger", |view| {
                                    view.child(
                                        div().flex().gap_2().children(
                                            [
                                                ("note_enhanced", "Note enhanced"),
                                                ("meeting_completed", "Meeting completed"),
                                            ]
                                            .into_iter()
                                            .map(
                                                |(value, label)| {
                                                    div()
                                                        .id(value)
                                                        .px_2()
                                                        .py_1()
                                                        .rounded(px(6.))
                                                        .cursor_pointer()
                                                        .bg(if current == value {
                                                            colors.accent
                                                        } else {
                                                            colors.card
                                                        })
                                                        .child(label)
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.set_field("trigger", value, cx)
                                                            },
                                                        ))
                                                },
                                            ),
                                        ),
                                    )
                                })
                        }),
                )
            })
            .when_some(
                self.draft.clone().filter(|draft| draft.groups.is_some()),
                |view, draft| {
                    view.children(
                        draft
                            .groups
                            .iter()
                            .flatten()
                            .zip(&self.group_inputs)
                            .enumerate()
                            .map(|(index, (group, inputs))| {
                                div()
                                    .p_3()
                                    .border_1()
                                    .border_color(colors.border)
                                    .rounded(px(8.))
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(format!(
                                                "{} {}",
                                                index + 1,
                                                group.kind.replace('_', " ")
                                            ))
                                            .child(
                                                div()
                                                    .id(("remove-group", index))
                                                    .cursor_pointer()
                                                    .child("Remove")
                                                    .on_click(cx.listener(
                                                        move |this, _, _, cx| {
                                                            this.change_group(None, Some(index), cx)
                                                        },
                                                    )),
                                            ),
                                    )
                                    .children(group.fields.iter().zip(inputs).map(
                                        |(field, input)| {
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(colors.muted_foreground)
                                                        .child(field.label),
                                                )
                                                .child(input.clone())
                                        },
                                    ))
                                    .when(group.kind == "markdown_export", |view| {
                                        view.children(
                                            [
                                                ("include_memo", "Note"),
                                                ("include_summary", "Summary"),
                                                ("include_transcript", "Transcript"),
                                                ("include_action_items", "Action items"),
                                                ("include_id_suffix", "ID suffix"),
                                            ]
                                            .into_iter()
                                            .map(
                                                |(key, label)| {
                                                    let enabled = group.base["options"][key]
                                                        .as_bool()
                                                        .unwrap_or(true);
                                                    div()
                                                        .id(SharedString::from(format!(
                                                            "option-{index}-{key}"
                                                        )))
                                                        .text_xs()
                                                        .cursor_pointer()
                                                        .child(format!(
                                                            "[{}] {label}",
                                                            if enabled { "x" } else { " " }
                                                        ))
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.toggle_export(index, key, cx)
                                                            },
                                                        ))
                                                },
                                            ),
                                        )
                                    })
                            }),
                    )
                    .child(
                        div().flex().gap_2().children(
                            [
                                ("markdown_export", "+ Markdown"),
                                ("slack_recap", "+ Slack"),
                                ("notion_update", "+ Notion"),
                                ("linear_issues", "+ Linear"),
                            ]
                            .into_iter()
                            .map(|(kind, label)| {
                                div()
                                    .id(SharedString::from(label))
                                    .cursor_pointer()
                                    .rounded(px(6.))
                                    .px_2()
                                    .py_1()
                                    .bg(colors.accent)
                                    .child(label)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.change_group(Some(kind), None, cx)
                                    }))
                            }),
                        ),
                    )
                },
            )
            .when(self.draft.is_some(), |view| {
                view.child(
                    div().flex().gap_2().children(
                        [
                            (EditorAction::Save, "Save"),
                            (EditorAction::Discard, "Discard"),
                            (EditorAction::Duplicate, "Duplicate"),
                            (EditorAction::Pin, "Pin / unpin"),
                            (EditorAction::Delete, "Delete"),
                        ]
                        .into_iter()
                        .filter(|(action, _)| match action {
                            EditorAction::Save | EditorAction::Discard => true,
                            EditorAction::Duplicate => existing && kind.as_ref() == "workflow",
                            EditorAction::Pin => {
                                existing && matches!(kind.as_ref(), "human" | "organization")
                            }
                            EditorAction::Delete => existing,
                            _ => false,
                        })
                        .map(|(action, label)| {
                            div()
                                .id(SharedString::from(format!("edit-{label}")))
                                .px_3()
                                .py_1()
                                .rounded(px(6.))
                                .bg(colors.accent)
                                .cursor_pointer()
                                .opacity(if self.busy { 0.5 } else { 1. })
                                .child(
                                    if matches!(action, EditorAction::Delete) && self.confirm_delete
                                    {
                                        "Confirm delete"
                                    } else {
                                        label
                                    },
                                )
                                .on_click(cx.listener(move |this, _, _, cx| this.act(action, cx)))
                        }),
                    ),
                )
            })
            .when(existing && kind.as_ref() == "human", |view| {
                view.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().child(self.merge_target.clone()))
                        .child(
                            div()
                                .id("choose-contact")
                                .cursor_pointer()
                                .child("Choose contact")
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.choose_target(true, cx)),
                                ),
                        )
                        .child(
                            div()
                                .id("contact-merge")
                                .px_3()
                                .py_1()
                                .cursor_pointer()
                                .child("Merge into this contact")
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.act(EditorAction::Merge, cx)),
                                ),
                        ),
                )
            })
            .when(existing && kind.as_ref() == "workflow", |view| {
                view.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().child(self.session_target.clone()))
                        .child(
                            div()
                                .id("choose-note")
                                .cursor_pointer()
                                .child("Choose note")
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.choose_target(false, cx)),
                                ),
                        )
                        .child(
                            div()
                                .id("use-for-note")
                                .cursor_pointer()
                                .px_3()
                                .py_2()
                                .rounded(px(6.))
                                .bg(colors.accent)
                                .child("Run for note")
                                .on_click(cx.listener(|this, _, _, cx| this.use_for_note(cx))),
                        ),
                )
            })
            .when(self.confirm_delete, |view| {
                view.child(div().text_sm().text_color(colors.destructive).child(
                    "Delete this item? This action changes stored data. Press Escape to cancel.",
                ))
            })
            .when(existing && kind.as_ref() == "workflow", |view| {
                view.child(
                    div()
                        .id("retry-automation")
                        .text_xs()
                        .cursor_pointer()
                        .child(if self.confirm_retry {
                            "Confirm destinations checked"
                        } else {
                            "Allow retry for selected note"
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.retry(cx))),
                )
            })
            .children(self.targets.iter().enumerate().map(|(index, (id, title))| {
                let id = id.clone();
                div()
                    .id(("target", index))
                    .px_2()
                    .py_1()
                    .rounded(px(6.))
                    .hover(|style| style.bg(colors.accent))
                    .cursor_pointer()
                    .child(SharedString::from(title.clone()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let input = if this.targets_are_contacts {
                            &this.merge_target
                        } else {
                            &this.session_target
                        };
                        input.update(cx, |input, cx| input.set_text(id.to_string(), cx));
                        this.targets.clear();
                        this.message.clear();
                        cx.notify();
                    }))
            }))
            .when(!self.message.is_empty(), |view| {
                view.child(div().text_sm().child(self.message.clone()))
            })
            .when(self.dirty, |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(colors.muted_foreground)
                        .child("Unsaved changes"),
                )
            })
    }
}
