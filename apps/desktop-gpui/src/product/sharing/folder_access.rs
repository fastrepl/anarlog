//! The folder editor's "Default access" row (`folders/folder-editor.tsx`): which people new
//! notes in a folder are shared with once their summary is ready. Rules live in the
//! `folder_default_share_access` setting shipping reads (`session-sharing/folder-default-access.ts`).

use std::sync::Arc;

use desktop_runtime::{CancellationToken, RuntimeHandle, ServiceError};
use gpui::{
    Context, FontWeight, Render, SharedString, Window, anchored, deferred, div, prelude::*, px,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{
    model::ShareWorkspace,
    popover::{icon, select_item, select_list},
};
use crate::{product::account, ui::theme::theme};

const RULES_ID: &str = "folder_default_share_access";
const WORKSPACES_SQL: &str = "SELECT w.id, w.name FROM workspaces w
    JOIN workspace_memberships m ON m.workspace_id = w.id AND m.user_id = ? AND m.deleted_at IS NULL
    WHERE w.kind = 'shared' AND w.deleted_at IS NULL
    ORDER BY w.name COLLATE NOCASE, w.id";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub folder_path: String,
    pub access: String,
    pub workspace_id: String,
    pub since: String,
}

/// The select's value: nobody, the meeting's people, or everyone in a Team.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefaultAccess {
    OnlyMe,
    Participants,
    Workspace(Arc<str>),
}

impl DefaultAccess {
    pub fn label(&self, workspaces: &[ShareWorkspace]) -> String {
        match self {
            Self::OnlyMe => "Only me".into(),
            Self::Participants => "People in the meeting".into(),
            Self::Workspace(id) => workspaces
                .iter()
                .find(|workspace| &workspace.id == id)
                .map(|workspace| format!("Everyone in {}", workspace.name))
                .unwrap_or_else(|| "Only me".into()),
        }
    }
}

fn normalize(path: &str) -> String {
    path.trim().trim_matches('/').to_owned()
}

/// `parseFolderDefaultAccessRules`: drops malformed entries instead of failing.
pub fn parse_rules(value: Option<&str>) -> Vec<Rule> {
    let entries: Vec<serde_json::Value> = value
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_default();
    entries
        .iter()
        .filter_map(|entry| {
            let folder_path = normalize(entry["folder_path"].as_str()?);
            let since = entry["since"].as_str().filter(|since| !since.is_empty())?;
            let access = entry["access"].as_str()?;
            let workspace_id = match access {
                "participants" => "",
                "workspace" => entry["workspace_id"].as_str().filter(|id| !id.is_empty())?,
                _ => return None,
            };
            (!folder_path.is_empty()).then(|| Rule {
                folder_path,
                access: access.into(),
                workspace_id: workspace_id.into(),
                since: since.into(),
            })
        })
        .collect()
}

pub fn rule_access(rules: &[Rule], path: &str) -> DefaultAccess {
    let path = normalize(path);
    match rules.iter().find(|rule| rule.folder_path == path) {
        Some(rule) if rule.access == "participants" => DefaultAccess::Participants,
        Some(rule) => DefaultAccess::Workspace(rule.workspace_id.as_str().into()),
        None => DefaultAccess::OnlyMe,
    }
}

/// `setFolderDefaultAccess`: replaces the folder's rule, keeping `since` when it is unchanged
/// so notes created before an unrelated save are not shared retroactively.
pub fn set_rule(rules: &[Rule], path: &str, access: &DefaultAccess, now: &str) -> Vec<Rule> {
    let path = normalize(path);
    let existing = rules.iter().find(|rule| rule.folder_path == path);
    let mut next: Vec<Rule> = rules
        .iter()
        .filter(|rule| rule.folder_path != path)
        .cloned()
        .collect();
    let (kind, workspace) = match access {
        DefaultAccess::OnlyMe => return next,
        DefaultAccess::Participants => ("participants", ""),
        DefaultAccess::Workspace(id) => ("workspace", id.as_ref()),
    };
    let since = existing
        .filter(|rule| rule.access == kind && rule.workspace_id == workspace)
        .map(|rule| rule.since.clone())
        .unwrap_or_else(|| now.into());
    next.push(Rule {
        folder_path: path,
        access: kind.into(),
        workspace_id: workspace.into(),
        since,
    });
    next
}

pub struct FolderAccess {
    runtime: RuntimeHandle,
    path: Arc<str>,
    rules: Vec<Rule>,
    workspaces: Arc<[ShareWorkspace]>,
    open: bool,
    confirm: Option<DefaultAccess>,
    saving: bool,
    cancel: CancellationToken,
    _account: gpui::Subscription,
}

impl FolderAccess {
    pub fn new(runtime: RuntimeHandle, path: Arc<str>, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            runtime,
            path,
            rules: Vec::new(),
            workspaces: Arc::from([]),
            open: false,
            confirm: None,
            saving: false,
            cancel: CancellationToken::new(),
            _account: cx.observe_global::<account::Account>(|this, cx| this.load(cx)),
        };
        this.load(cx);
        this
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
        let user = account::current(cx).user.map(|user| user.id);
        let reply = self
            .runtime
            .read(self.cancel.clone(), move |services| async move {
                let failure = |error: String| ServiceError::Failed(error.into());
                let rules = services
                    .executor
                    .execute(
                        "SELECT value_json FROM app_settings WHERE id = ?".into(),
                        vec![json!(RULES_ID)],
                    )
                    .await
                    .map_err(|error| failure(error.to_string()))?;
                let workspaces = match user {
                    Some(user) => services
                        .executor
                        .execute(WORKSPACES_SQL.into(), vec![json!(user)])
                        .await
                        .map_err(|error| failure(error.to_string()))?,
                    None => Vec::new(),
                };
                Ok((
                    parse_rules(rules.first().and_then(|row| row["value_json"].as_str())),
                    workspaces
                        .iter()
                        .map(|row| ShareWorkspace {
                            id: row["id"].as_str().unwrap_or_default().into(),
                            name: row["name"].as_str().unwrap_or_default().into(),
                        })
                        .collect::<Vec<_>>(),
                ))
            });
        cx.spawn(async move |this, cx| {
            let Ok(reply) = reply else {
                return;
            };
            if let Ok((rules, workspaces)) = reply.receive().await {
                let _ = this.update(cx, |this, cx| {
                    this.rules = rules;
                    this.workspaces = workspaces.into();
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn save(&mut self, access: DefaultAccess, cx: &mut Context<Self>) {
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let rules = set_rule(&self.rules, &self.path, &access, &now);
        let Ok(value) = serde_json::to_string(&rules) else {
            return;
        };
        self.saving = true;
        self.open = false;
        let reply = self.runtime.submit(move |services| async move {
            services
                .executor
                .execute(
                    "INSERT INTO app_settings (id, value_json, updated_at) VALUES (?, ?, ?)
                     ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json,
                     updated_at = excluded.updated_at"
                        .into(),
                    vec![json!(RULES_ID), json!(value), json!(now)],
                )
                .await
                .map_err(|error| ServiceError::Failed(error.to_string().into()))
        });
        cx.spawn(async move |this, cx| {
            let saved = match reply {
                Ok(reply) => reply.receive().await.is_ok(),
                Err(_) => false,
            };
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                if saved {
                    this.rules = rules;
                    this.confirm = None;
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn choose(&mut self, access: DefaultAccess, cx: &mut Context<Self>) {
        self.open = false;
        if access == DefaultAccess::OnlyMe {
            self.save(access, cx);
        } else {
            self.confirm = Some(access);
            cx.notify();
        }
    }
}

impl Render for FolderAccess {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        if !account::current(cx).signed_in() {
            return div().into_any_element();
        }
        let value = rule_access(&self.rules, &self.path);
        let mut options = vec![DefaultAccess::OnlyMe, DefaultAccess::Participants];
        options.extend(
            self.workspaces
                .iter()
                .map(|workspace| DefaultAccess::Workspace(workspace.id.clone())),
        );
        let workspaces = self.workspaces.clone();
        let enabled = !self.saving;
        div()
            .flex()
            .items_start()
            .justify_between()
            .gap_4()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_sm()
                            .line_height(px(20.))
                            .font_weight(crate::ui::theme::MEDIUM)
                            .child("Default access"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(16.))
                            .text_color(colors.muted_foreground)
                            .child(
                                "Share new notes in this folder when their summary is ready. You can still change access on each note.",
                            ),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_shrink_0()
                    .child(
                        div()
                            .id("folder-default-access")
                            .h(px(32.))
                            .w(px(176.))
                            .px_3()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(colors.input)
                            .bg(colors.card)
                            .shadow_xs()
                            .text_xs()
                            .line_height(px(16.))
                            .when_else(
                                enabled,
                                |b| {
                                    b.cursor_pointer().on_click(cx.listener(|this, _, _, cx| {
                                        this.open = !this.open;
                                        cx.notify();
                                    }))
                                },
                                |b| b.opacity(0.5),
                            )
                            .child(div().min_w_0().truncate().child(value.label(&workspaces)))
                            .child(
                                icon(
                                    "sharing/ArrowDown01Icon.svg",
                                    16.,
                                    colors.foreground.opacity(0.5),
                                )
                                .mr(px(-4.)),
                            ),
                    )
                    .when(self.open, |view| {
                        view.child(
                            div().absolute().top_full().left_0().child(
                                deferred(
                                    anchored().snap_to_window_with_margin(px(8.)).child(
                                        div()
                                            .id("folder-default-access-menu")
                                            .mt_1()
                                            .occlude()
                                            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                                this.open = false;
                                                cx.notify();
                                            }))
                                            .child(select_list(176., colors).children(
                                                options.into_iter().enumerate().map(
                                                    |(index, option)| {
                                                        let label = option.label(&workspaces);
                                                        let selected = option == value;
                                                        select_item(
                                                            ("folder-access-option", index),
                                                            label,
                                                            selected,
                                                            true,
                                                            None,
                                                            colors,
                                                        )
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.choose(option.clone(), cx)
                                                            },
                                                        ))
                                                    },
                                                ),
                                            )),
                                    ),
                                )
                                .with_priority(2),
                            ),
                        )
                    }),
            )
            .when_some(self.confirm.clone(), |view, access| {
                let label: SharedString = access.label(&workspaces).into();
                view.child(self.confirmation(label, access, window, cx))
            })
            .into_any_element()
    }
}

impl FolderAccess {
    /// Shipping's `DialogContent` asking before new notes start being shared.
    fn confirmation(
        &self,
        label: SharedString,
        access: DefaultAccess,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let colors = theme(window);
        let size = window.viewport_size();
        let enabled = !self.saving;
        let button = |id: &'static str, text: &'static str, primary: bool| {
            div()
                .id(id)
                .h(px(36.))
                .px_4()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.))
                .text_sm()
                .line_height(px(20.))
                .font_weight(crate::ui::theme::MEDIUM)
                .shadow_xs()
                .map(|b| {
                    if primary {
                        b.bg(colors.primary).text_color(colors.primary_foreground)
                    } else {
                        b.border_1()
                            .border_color(colors.input)
                            .bg(colors.background)
                    }
                })
                .when_else(enabled, |b| b.cursor_pointer(), |b| b.opacity(0.5))
                .child(text)
        };
        deferred(
            anchored().position(gpui::point(px(0.), px(0.))).child(
                div()
                    .id("folder-access-confirm")
                    .w(size.width)
                    .h(size.height)
                    .occlude()
                    .bg(gpui::rgba(0x000000cc))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .id("folder-access-dialog")
                            .relative()
                            .w(px(512.))
                            .p_6()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(colors.border)
                            .bg(colors.background)
                            .text_color(colors.foreground)
                            .shadow_lg()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .text_lg()
                                            .line_height(px(18.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("Share new notes in this folder?"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .line_height(px(20.))
                                            .text_color(colors.muted_foreground)
                                            .child(format!(
                                                "New notes in this folder will be shared with {label} once their summary is ready. Notes already in the folder are not changed."
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .justify_end()
                                    .gap_2()
                                    .child(button("folder-access-cancel", "Cancel", false).when(
                                        enabled,
                                        |b| {
                                            b.hover(|s| s.bg(colors.accent)).on_click(cx.listener(
                                                |this, _, _, cx| {
                                                    this.confirm = None;
                                                    cx.notify();
                                                },
                                            ))
                                        },
                                    ))
                                    .child(button("folder-access-confirm-button", "Confirm", true).when(
                                        enabled,
                                        |b| {
                                            b.hover(|s| s.bg(colors.primary.opacity(0.9)))
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.save(access.clone(), cx)
                                                }))
                                        },
                                    )),
                            )
                            .child(
                                div()
                                    .id("folder-access-close")
                                    .absolute()
                                    .top_4()
                                    .right_4()
                                    .opacity(0.7)
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(1.))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if !this.saving {
                                            this.confirm = None;
                                            cx.notify();
                                        }
                                    }))
                                    .child(icon("sharing/XIcon.svg", 16., colors.foreground)),
                            ),
                    ),
            ),
        )
        .with_priority(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_round_trip_and_keep_since_only_while_unchanged() {
        let rules = parse_rules(Some(
            r#"[{"folder_path":"/Research/","access":"participants","workspace_id":"","since":"t0"},
                {"folder_path":"Team","access":"workspace","workspace_id":"","since":"t0"},
                {"folder_path":"Bad","access":"everyone","workspace_id":"","since":"t0"},
                {"folder_path":"Old","access":"participants","since":""}]"#,
        ));
        assert_eq!(rules.len(), 1);
        assert_eq!(rule_access(&rules, "Research"), DefaultAccess::Participants);
        assert_eq!(rule_access(&rules, "Other"), DefaultAccess::OnlyMe);
        assert!(parse_rules(Some("not json")).is_empty());

        let same = set_rule(&rules, "Research", &DefaultAccess::Participants, "t1");
        assert_eq!(same[0].since, "t0");
        let team = set_rule(
            &rules,
            "Research",
            &DefaultAccess::Workspace("w".into()),
            "t1",
        );
        assert_eq!(
            (
                team[0].access.as_str(),
                team[0].workspace_id.as_str(),
                team[0].since.as_str()
            ),
            ("workspace", "w", "t1")
        );
        assert!(set_rule(&team, "Research", &DefaultAccess::OnlyMe, "t2").is_empty());
        let saved = serde_json::to_string(&team).unwrap();
        assert_eq!(parse_rules(Some(&saved)), team);
    }
}
