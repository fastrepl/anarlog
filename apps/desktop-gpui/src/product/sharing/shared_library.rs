//! Shipping's `SharedResourceLibrarySection` (`resource-sharing/library.tsx`): folders and
//! templates others shared with the account, under the sidebar list, and the dialog that adds
//! an editable copy.

use std::sync::Arc;

use desktop_runtime::{RuntimeHandle, SaveDocument, ServiceError};
use gpui::{Context, Entity, FontWeight, Render, SharedString, Window, div, prelude::*, px};
use serde_json::{Value, json};

use super::{SharingCloud, popover::icon};
use crate::{
    product::{
        account,
        cloud::resources::{ResourceKind, SharedResource},
    },
    ui::{input::TextInput, theme::theme},
    workspace::{
        folders,
        resource_list::{dialog_button, glass_dialog, modal},
    },
};

pub struct SharedLibrary {
    runtime: RuntimeHandle,
    kind: ResourceKind,
    search: Entity<TextInput>,
    resources: Vec<SharedResource>,
    selected: Option<SharedResource>,
    importing: bool,
    error: Option<SharedString>,
    _subscriptions: Vec<gpui::Subscription>,
}

impl SharedLibrary {
    pub fn new(
        runtime: RuntimeHandle,
        kind: ResourceKind,
        search: Entity<TextInput>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![
            cx.observe(&search, |_, _, cx| cx.notify()),
            cx.observe_global::<account::Account>(|this, cx| this.load(cx)),
        ];
        let mut this = Self {
            runtime,
            kind,
            search,
            resources: Vec::new(),
            selected: None,
            importing: false,
            error: None,
            _subscriptions: subscriptions,
        };
        this.load(cx);
        this
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let cloud = cx
            .try_global::<SharingCloud>()
            .map(|cloud| cloud.0.clone())
            .filter(|_| account::current(cx).signed_in());
        let Some(cloud) = cloud else {
            self.resources.clear();
            return;
        };
        let kind = self.kind;
        cx.spawn(async move |this, cx| {
            let resources = cloud.shared_resources(kind).await.unwrap_or_default();
            let _ = this.update(cx, |this, cx| {
                this.set_resources(resources);
                cx.notify();
            });
        })
        .detach();
    }

    fn set_resources(&mut self, resources: Vec<SharedResource>) {
        self.resources = resources.into_iter().filter(|r| !r.owned).collect();
    }

    fn import(&mut self, cx: &mut Context<Self>) {
        let Some(resource) = self.selected.clone() else {
            return;
        };
        self.importing = true;
        self.error = None;
        let runtime = self.runtime.clone();
        let kind = self.kind;
        cx.spawn(async move |this, cx| {
            let result = match kind {
                ResourceKind::Folder => import_folder(&runtime, &resource.payload).await,
                ResourceKind::Template => import_template(&runtime, &resource.payload).await,
            };
            let _ = this.update(cx, |this, cx| {
                this.importing = false;
                match result {
                    Ok(()) => this.selected = None,
                    Err(error) => this.error = Some(error.to_string().into()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

/// `resourceDescription`: what the copy brings along.
pub fn describe(kind: ResourceKind, resource: &SharedResource) -> String {
    let count = |items: &Value, one: &str, many: &str| {
        let count = items.as_array().map_or(0, Vec::len);
        format!(
            "{count} {} shared by {}",
            if count == 1 { one } else { many },
            resource.owner_email
        )
    };
    match kind {
        ResourceKind::Folder => count(&resource.payload["notes"], "note", "notes"),
        ResourceKind::Template => resource.payload["template"]["description"]
            .as_str()
            .filter(|description| !description.is_empty())
            .map(Into::into)
            .unwrap_or_else(|| format!("Shared by {}", resource.owner_email)),
    }
}

/// `availableFolderPath`: the shared folder's name, numbered when it is taken.
pub fn available_path(name: &str, taken: &[String]) -> String {
    let name = name.trim().trim_matches('/');
    let name = if name.is_empty() {
        "Shared folder"
    } else {
        name
    };
    let free = |candidate: &str| !taken.iter().any(|path| path == candidate);
    if free(name) {
        return name.into();
    }
    (2..)
        .map(|index| format!("{name} {index}"))
        .find(|candidate| free(candidate))
        .unwrap_or_default()
}

fn failure(message: &str) -> ServiceError {
    ServiceError::Failed(message.to_owned().into())
}

/// `importSharedFolder`: a new folder with the shared context and a note for each shared one.
async fn import_folder(runtime: &RuntimeHandle, payload: &Value) -> desktop_runtime::Result<()> {
    let taken = runtime
        .read(
            desktop_runtime::CancellationToken::new(),
            |services| async move {
                let rows = services
                    .executor
                    .execute(
                        "SELECT path FROM folders WHERE deleted_at IS NULL".into(),
                        Vec::new(),
                    )
                    .await
                    .map_err(|error| failure(&error.to_string()))?;
                Ok(rows
                    .iter()
                    .filter_map(|row| row["path"].as_str().map(str::to_owned))
                    .collect::<Vec<_>>())
            },
        )?
        .receive()
        .await?;
    let root: Arc<str> =
        available_path(payload["path"].as_str().unwrap_or_default(), &taken).into();
    let root = folders::create(runtime, root)?.receive().await?;
    if let Some(instructions) = payload["instructions"]
        .as_str()
        .filter(|text| !text.is_empty())
    {
        folders::update(
            runtime,
            root.clone(),
            folders::Update::Instructions(instructions.into()),
        )?
        .receive()
        .await?;
    }
    for note in payload["notes"].as_array().into_iter().flatten() {
        let folder = match note["relativeFolderPath"]
            .as_str()
            .filter(|path| !path.is_empty())
        {
            Some(relative) => format!("{root}/{relative}"),
            None => root.to_string(),
        };
        if folder != *root {
            folders::create(runtime, folder.clone().into())?
                .receive()
                .await?;
        }
        let title = note["title"].as_str().unwrap_or_default();
        let created = runtime.create_note(title.into())?.receive().await?;
        let id = created.summary.id.0.clone();
        runtime
            .submit(move |services| async move {
                services
                    .executor
                    .execute(
                        "UPDATE sessions SET folder_path = ? WHERE id = ?".into(),
                        vec![json!(folder), json!(id)],
                    )
                    .await
                    .map_err(|error| failure(&error.to_string()))
            })?
            .receive()
            .await?;
        if let Some(base) = created.note {
            runtime
                .save_document(SaveDocument {
                    base,
                    body: note["body"].to_string().into(),
                })?
                .receive()
                .await?;
        }
    }
    Ok(())
}

/// `createTemplate(parseSharedTemplatePayload(...))`.
async fn import_template(runtime: &RuntimeHandle, payload: &Value) -> desktop_runtime::Result<()> {
    let template = payload["template"].clone();
    if !template.is_object() {
        return Err(failure("This shared template is invalid"));
    }
    runtime
        .submit(move |services| async move {
            let encode = |value: &Value| (!value.is_null()).then(|| value.to_string());
            services
                .executor
                .execute(
                    "INSERT INTO templates (id, title, description, pinned, category, icon_json,
                       targets_json, sections_json, created_at, updated_at)
                     VALUES (?, ?, ?, 0, ?, COALESCE(?, '{\"type\":\"icon\",\"value\":\"notebook-tabs\",\"color\":\"#9ca3af\"}'),
                       ?, ?, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))"
                        .into(),
                    vec![
                        json!(uuid::Uuid::new_v4().to_string()),
                        json!(template["title"].as_str().unwrap_or_default()),
                        json!(template["description"].as_str().unwrap_or_default()),
                        json!(template["category"].as_str()),
                        json!(encode(&template["icon"])),
                        json!(encode(&template["targets"])),
                        json!(encode(&template["sections"]).unwrap_or_else(|| "[]".into())),
                    ],
                )
                .await
                .map(|_| ())
                .map_err(|error| failure(&error.to_string()))
        })?
        .receive()
        .await
}

impl Render for SharedLibrary {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let query = self.search.read(cx).buffer.text.trim().to_lowercase();
        let available: Vec<SharedResource> = self
            .resources
            .iter()
            .filter(|resource| query.is_empty() || resource.title.to_lowercase().contains(&query))
            .cloned()
            .collect();
        if available.is_empty() {
            return div().into_any_element();
        }
        let glyph = match self.kind {
            ResourceKind::Folder => "sharing/Folder01Icon.svg",
            ResourceKind::Template => "sharing/FileTextIcon.svg",
        };
        div()
            .mt_3()
            .pt_3()
            .border_t_1()
            .border_color(colors.border.opacity(0.6))
            .child(
                div()
                    .mb_1()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(11.))
                    .line_height(px(16.5))
                    .font_weight(crate::ui::theme::MEDIUM)
                    .text_color(colors.muted_foreground)
                    .child(icon("sharing/AddTeamIcon.svg", 14., colors.muted_foreground))
                    .child("SHARED WITH ME"),
            )
            .children(available.into_iter().map(|resource| {
                let detail = resource
                    .workspace_name
                    .clone()
                    .unwrap_or_else(|| resource.owner_email.clone());
                let pick = resource.clone();
                div()
                    .id(SharedString::from(format!("shared-resource-{}", resource.share_id)))
                    .w_full()
                    .px_3()
                    .py_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .hover(|s| s.bg(colors.accent.opacity(0.5)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected = Some(pick.clone());
                        this.error = None;
                        cx.notify();
                    }))
                    .child(icon(glyph, 16., colors.muted_foreground))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .text_sm()
                                    .line_height(px(20.))
                                    .child(resource.title.to_string()),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(11.))
                                    .line_height(px(16.5))
                                    .text_color(colors.muted_foreground)
                                    .child(detail.to_string()),
                            ),
                    )
            }))
            .when_some(self.selected.clone(), |view, resource| {
                let importing = self.importing;
                view.child(modal(
                    window,
                    glass_dialog(colors)
                        .text_color(colors.foreground)
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                            if !this.importing {
                                this.selected = None;
                                cx.notify();
                            }
                        }))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .text_center()
                                .child(
                                    div()
                                        .text_lg()
                                        .line_height(px(18.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(resource.title.to_string()),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .line_height(px(20.))
                                        .text_color(colors.muted_foreground)
                                        .child(describe(self.kind, &resource)),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .line_height(px(20.))
                                .text_color(colors.muted_foreground)
                                .child("This adds a copy you can edit without changing the shared item."),
                        )
                        .when_some(self.error.clone(), |view, error| {
                            view.child(
                                div()
                                    .text_xs()
                                    .text_color(colors.destructive)
                                    .child(error),
                            )
                        })
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    dialog_button("dialog-cancel", "Cancel", false, colors)
                                        .flex_1()
                                        .when(!importing, |b| {
                                            b.on_click(cx.listener(|this, _, _, cx| {
                                                this.selected = None;
                                                cx.notify();
                                            }))
                                        }),
                                )
                                .child(
                                    dialog_button("shared-resource-import", "Add a copy", false, colors)
                                        .flex_1()
                                        .h(px(36.))
                                        .text_sm()
                                        .when(importing, |b| b.opacity(0.5))
                                        .when(!importing, |b| {
                                            b.on_click(cx.listener(|this, _, _, cx| this.import(cx)))
                                        }),
                                ),
                        )
                        .into_any_element(),
                ))
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn resource(payload: Value) -> SharedResource {
        SharedResource {
            share_id: "s".into(),
            source_id: "Research".into(),
            title: "Research".into(),
            owner_email: "maya@northwind.example".into(),
            general_workspace_id: None,
            workspace_name: None,
            owned: false,
            payload,
        }
    }

    #[test]
    fn descriptions_and_copy_names_follow_shipping() {
        assert_eq!(
            describe(ResourceKind::Folder, &resource(json!({"notes":[{}, {}]}))),
            "2 notes shared by maya@northwind.example"
        );
        assert_eq!(
            describe(ResourceKind::Folder, &resource(json!({"notes":[{}]}))),
            "1 note shared by maya@northwind.example"
        );
        assert_eq!(
            describe(
                ResourceKind::Template,
                &resource(json!({"template":{"description":"Hiring loop"}}))
            ),
            "Hiring loop"
        );
        assert_eq!(
            describe(ResourceKind::Template, &resource(json!({"template":{}}))),
            "Shared by maya@northwind.example"
        );
        let taken = vec!["Research".to_owned(), "Research 2".to_owned()];
        assert_eq!(available_path("Research", &taken), "Research 3");
        assert_eq!(available_path("Clients", &taken), "Clients");
        assert_eq!(available_path(" / ", &taken), "Shared folder");
    }
}
