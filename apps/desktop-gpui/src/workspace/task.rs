//! Shipping's Task tab (`task/*`): the GitHub issues and pull requests linked from a note,
//! opened by clicking their chips (`editor-bridge/app-link-view.tsx`).

use std::{collections::HashMap, rc::Rc, sync::Arc};

use desktop_runtime::{Result, RuntimeHandle, ServiceError};
use futures::future::BoxFuture;
use gpui::{
    App, ClickEvent, Context, Div, Global, Hsla, Render, ScrollHandle, SharedString, WeakEntity,
    Window, div, prelude::*, px, rgb, svg,
};
use serde_json::Value;

use super::navigation::Route;
use crate::{
    chat::markdown,
    editor::document::Document,
    ui::theme::{MEDIUM, Theme, monospace_font, theme},
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Resource {
    pub pull: bool,
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

impl Resource {
    fn key(&self) -> String {
        format!(
            "{}:{}/{}#{}",
            if self.pull {
                "github_pr"
            } else {
                "github_issue"
            },
            self.owner,
            self.repo,
            self.number
        )
    }

    fn parse(key: &str) -> Option<Self> {
        let (kind, rest) = key.split_once(':')?;
        let (path, number) = rest.rsplit_once('#')?;
        let (owner, repo) = path.split_once('/')?;
        Some(Self {
            pull: match kind {
                "github_pr" => true,
                "github_issue" => false,
                _ => return None,
            },
            owner: owner.into(),
            repo: repo.into(),
            number: number.parse().ok()?,
        })
    }

    fn from_app_link(attrs: &Value) -> Option<Self> {
        if attrs["provider"] != "github" {
            return None;
        }
        let pull = match attrs["kind"].as_str()? {
            "issue" => false,
            "pull_request" => true,
            _ => return None,
        };
        let owner = attrs["owner"].as_str().filter(|value| !value.is_empty())?;
        let repo = attrs["repo"].as_str().filter(|value| !value.is_empty())?;
        Some(Self {
            pull,
            owner: owner.into(),
            repo: repo.into(),
            number: attrs["number"].as_u64().filter(|number| *number > 0)?,
        })
    }
}

/// The tab for these resources; equal lists are the same tab, like `openTaskTab`.
pub fn route(resources: &[Resource]) -> Route {
    Route::Task(
        resources
            .iter()
            .map(Resource::key)
            .collect::<Vec<_>>()
            .join(" ")
            .into(),
    )
}

pub fn resources(route: &str) -> Vec<Resource> {
    route.split(' ').filter_map(Resource::parse).collect()
}

/// `collectSiblingResources`: the GitHub issues and PRs in the clicked chip's list item, or
/// in the whole note outside lists. Empty unless the clicked chip is one of them.
pub fn sibling_resources(document: &Document, position: usize, clicked: &Value) -> Vec<Resource> {
    if Resource::from_app_link(&clicked["attrs"]).is_none() {
        return Vec::new();
    }
    let path = document
        .resolve(position)
        .map(|resolved| resolved.path)
        .unwrap_or_default();
    let target = (0..=path.len())
        .rev()
        .filter_map(|depth| document.node(&path[..depth]))
        .find(|node| matches!(node.kind(), "listItem" | "taskItem"))
        .unwrap_or_else(|| document.root.clone());
    fn visit(node: &Value, found: &mut Vec<Resource>) {
        if node["type"] == "appLink" {
            found.extend(Resource::from_app_link(&node["attrs"]));
            return;
        }
        for child in node["content"].as_array().into_iter().flatten() {
            visit(child, found);
        }
    }
    let mut found = Vec::new();
    visit(&target.value(), &mut found);
    found
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Label {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Issue {
    pub title: String,
    pub number: u64,
    pub closed: bool,
    pub pull: bool,
    pub merged: bool,
    pub author: Option<String>,
    pub created_at: String,
    pub comments: u64,
    pub labels: Vec<Label>,
    pub assignees: Vec<String>,
    pub body: Option<String>,
}

impl Issue {
    pub fn from_value(value: &Value) -> Self {
        let text = |value: &Value| value.as_str().map(str::to_owned);
        Self {
            title: text(&value["title"]).unwrap_or_default(),
            number: value["number"].as_u64().unwrap_or_default(),
            closed: value["state"] == "closed",
            pull: !value["pull_request"].is_null(),
            merged: !value["pull_request"]["merged_at"].is_null(),
            author: text(&value["user"]["login"]),
            created_at: text(&value["created_at"]).unwrap_or_default(),
            comments: value["comments"].as_u64().unwrap_or_default(),
            labels: value["labels"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|label| Label {
                    name: text(&label["name"]).unwrap_or_default(),
                    color: text(&label["color"]).filter(|color| !color.is_empty()),
                })
                .collect(),
            assignees: value["assignees"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|assignee| text(&assignee["login"]))
                .collect(),
            body: text(&value["body"]).filter(|body| !body.is_empty()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Comment {
    pub author: Option<String>,
    pub created_at: String,
    pub body: Option<String>,
}

impl Comment {
    pub fn from_value(value: &Value) -> Self {
        Self {
            author: value["user"]["login"].as_str().map(str::to_owned),
            created_at: value["created_at"].as_str().unwrap_or_default().to_owned(),
            body: value["body"]
                .as_str()
                .filter(|body| !body.is_empty())
                .map(str::to_owned),
        }
    }
}

/// The public GitHub REST API (`plugin-todo`'s unauthenticated client).
pub trait Github: Send + Sync {
    fn get(&self, path: String) -> BoxFuture<'static, Result<Value>>;
}

/// Replaces the public GitHub API (tests and screenshots).
#[derive(Clone)]
pub struct GithubOverride(pub Arc<dyn Github>);

impl Global for GithubOverride {}

pub fn github(runtime: &RuntimeHandle, cx: &App) -> Arc<dyn Github> {
    cx.try_global::<GithubOverride>()
        .map(|installed| installed.0.clone())
        .unwrap_or_else(|| Arc::new(PublicGithub(runtime.clone())))
}

struct PublicGithub(RuntimeHandle);

impl Github for PublicGithub {
    fn get(&self, path: String) -> BoxFuture<'static, Result<Value>> {
        let reply = self.0.service(move |_| async move {
            let failed = |error: reqwest::Error| ServiceError::Failed(error.to_string().into());
            let response = reqwest::Client::new()
                .get(format!("https://api.github.com{path}"))
                .header("accept", "application/vnd.github+json")
                .header("user-agent", "anarlog-desktop")
                .send()
                .await
                .map_err(failed)?;
            let status = response.status();
            if !status.is_success() {
                return Err(ServiceError::Failed(
                    format!("GitHub API returned {status}").into(),
                ));
            }
            response.json::<Value>().await.map_err(failed)
        });
        Box::pin(async move { reply?.receive().await })
    }
}

/// `searchGitHubRepos`: up to six `owner/repo` names; failures read as no results.
pub async fn search_repositories(github: Arc<dyn Github>, query: String) -> Vec<String> {
    let query: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    github
        .get(format!("/search/repositories?q={query}&per_page=6"))
        .await
        .ok()
        .and_then(|value| value["items"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|item| item["full_name"].as_str().map(str::to_owned))
        .collect()
}

enum Load {
    Loading,
    Failed,
    Ready(Issue, Option<Vec<Comment>>),
}

/// `TabContentTask`.
pub struct TaskView {
    resources: Vec<Resource>,
    details: HashMap<Resource, Load>,
    scroll: ScrollHandle,
    active: Option<usize>,
}

impl TaskView {
    pub fn new(runtime: &RuntimeHandle, route: &str, cx: &mut Context<Self>) -> Self {
        let resources = resources(route);
        let github = github(runtime, cx);
        for resource in resources.clone() {
            let github = github.clone();
            cx.spawn(async move |this, cx| {
                let base = format!(
                    "/repos/{}/{}/issues/{}",
                    resource.owner, resource.repo, resource.number
                );
                let issue = github
                    .get(base.clone())
                    .await
                    .map(|value| Issue::from_value(&value));
                let comments = match &issue {
                    Ok(_) => github
                        .get(format!("{base}/comments"))
                        .await
                        .ok()
                        .map(|value| {
                            value
                                .as_array()
                                .into_iter()
                                .flatten()
                                .map(Comment::from_value)
                                .collect()
                        }),
                    Err(_) => None,
                };
                let _ = this.update(cx, |this, cx| {
                    this.details.insert(
                        resource,
                        match issue {
                            Ok(issue) => Load::Ready(issue, comments),
                            Err(_) => Load::Failed,
                        },
                    );
                    cx.notify();
                });
            })
            .detach();
        }
        Self {
            details: resources
                .iter()
                .map(|r| (r.clone(), Load::Loading))
                .collect(),
            resources,
            scroll: ScrollHandle::new(),
            active: None,
        }
    }
}

fn date(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| {
            date.with_timezone(&chrono::Local)
                .format("%b %-d, %Y")
                .to_string()
        })
        .unwrap_or_default()
}

fn icon(path: &'static str, size: f32, color: Hsla) -> gpui::Svg {
    svg()
        .path(path)
        .size(px(size))
        .flex_shrink_0()
        .text_color(color)
}

/// Parses `#rrggbb` label colors with an alpha byte, as shipping's inline styles do.
fn label_color(hex: &str, alpha: u8) -> Hsla {
    let value = u32::from_str_radix(hex, 16).unwrap_or(0x525252);
    gpui::rgba((value << 8) | alpha as u32).into()
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "comment" } else { "comments" }
}

impl TaskView {
    fn markdown(&self, text: &str, id: &str, colors: &Theme, cx: &App) -> gpui::AnyElement {
        let style = markdown::Style {
            colors: Theme {
                foreground: colors.muted_foreground,
                ..*colors
            },
            mono: monospace_font(cx),
            chat: WeakEntity::new_invalid(),
            menu: None,
            copied: None,
            selection: None,
            texts: Rc::default(),
            streaming: false,
        };
        div()
            .mt_1()
            .text_color(colors.muted_foreground)
            .child(markdown::render(&markdown::parse(text), &style, id))
            .into_any_element()
    }

    fn badge(issue: &Issue) -> Div {
        // Tailwind's purple/red/green 100 and 700.
        let (label, background, foreground, path) = if issue.pull && issue.merged {
            ("Merged", 0xf3e8ff, 0x8200db, "product/GitMergeIcon.svg")
        } else if issue.closed {
            if issue.pull {
                ("Closed", 0xffe2e2, 0xc10007, "product/CancelCircleIcon.svg")
            } else {
                ("Closed", 0xf3e8ff, 0x8200db, "product/CancelCircleIcon.svg")
            }
        } else if issue.pull {
            ("Open", 0xdcfce7, 0x008236, "product/GitPullRequestIcon.svg")
        } else {
            ("Open", 0xdcfce7, 0x008236, "product/RadioButtonIcon.svg")
        };
        div()
            .flex()
            .items_center()
            .gap_1()
            .rounded(px(8.))
            .px(px(10.))
            .py(px(2.))
            .text_xs()
            .line_height(px(16.))
            .font_weight(MEDIUM)
            .bg(rgb(background))
            .text_color(rgb(foreground))
            .child(icon(path, 14., rgb(foreground).into()))
            .child(label)
    }

    fn resource(
        &self,
        index: usize,
        resource: &Resource,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let view = div().w_full().max_w(px(768.)).px_6().py_6();
        let centered = |text: &'static str| {
            div()
                .py_12()
                .flex()
                .items_center()
                .justify_center()
                .text_color(colors.muted_foreground)
                .child(text)
        };
        let (issue, comments) = match self.details.get(resource) {
            Some(Load::Ready(issue, comments)) => (issue, comments),
            Some(Load::Failed) => {
                return view.child(centered(if resource.pull {
                    "Failed to load pull request"
                } else {
                    "Failed to load issue"
                }));
            }
            _ => return view.child(centered("Loading...")),
        };
        let pull = resource.pull || issue.pull;
        let url = format!(
            "https://github.com/{}/{}/{}/{}",
            resource.owner,
            resource.repo,
            if pull { "pull" } else { "issues" },
            resource.number
        );
        let muted = colors.muted_foreground;
        let accent = colors.accent;
        let header = div()
            .mb_4()
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .text_xl()
                            .line_height(px(27.5))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(issue.title.clone())
                            .child(
                                div()
                                    .ml_2()
                                    .font_weight(gpui::FontWeight::NORMAL)
                                    .text_color(muted)
                                    .child(format!("#{}", issue.number)),
                            )
                            .flex()
                            .flex_wrap(),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("task-open-{index}")))
                            .flex_none()
                            .p(px(6.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .hover(move |style| style.bg(accent))
                            .on_click(move |_: &ClickEvent, _, cx| cx.open_url(&url))
                            .child(icon("product/ExternalLinkIcon.svg", 16., muted)),
                    ),
            )
            .child(
                div()
                    .mt_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .line_height(px(20.))
                    .text_color(muted)
                    .child(Self::badge(issue))
                    .child(format!(
                        "{} opened on {}",
                        issue.author.clone().unwrap_or_default(),
                        date(&issue.created_at)
                    ))
                    .when(issue.comments > 0, |view| {
                        view.child(format!(
                            "· {} {}",
                            issue.comments,
                            plural(issue.comments as usize)
                        ))
                    }),
            );
        let labels = (!issue.labels.is_empty()).then(|| {
            div()
                .mb_4()
                .flex()
                .flex_wrap()
                .gap(px(6.))
                .children(issue.labels.iter().map(|label| {
                    let (background, foreground, border) = match &label.color {
                        Some(color) => (
                            label_color(color, 0x20),
                            label_color(color, 0xff),
                            label_color(color, 0x40),
                        ),
                        None => (
                            rgb(0xe5e5e5).into(),
                            rgb(0x525252).into(),
                            rgb(0xd4d4d4).into(),
                        ),
                    };
                    div()
                        .rounded(px(8.))
                        .px(px(10.))
                        .py(px(2.))
                        .border_1()
                        .border_color(border)
                        .bg(background)
                        .text_color(foreground)
                        .text_xs()
                        .line_height(px(16.))
                        .font_weight(MEDIUM)
                        .child(label.name.clone())
                }))
        });
        let assignees = (!issue.assignees.is_empty()).then(|| {
            div()
                .mb_4()
                .flex()
                .items_center()
                .gap_2()
                .text_sm()
                .text_color(muted)
                .child(div().font_weight(MEDIUM).child("Assignees:"))
                .children(
                    issue
                        .assignees
                        .iter()
                        .map(|login| div().child(login.clone())),
                )
        });
        let body = div()
            .border_t_1()
            .border_color(colors.border)
            .pt_4()
            .map(|view| match &issue.body {
                Some(body) => {
                    view.child(self.markdown(body, &format!("task-body-{index}"), colors, cx))
                }
                None => view
                    .text_sm()
                    .italic()
                    .text_color(muted)
                    .child("No description provided."),
            });
        let comments = comments
            .as_ref()
            .filter(|comments| !comments.is_empty())
            .map(|comments| {
                div()
                    .mt_6()
                    .border_t_1()
                    .border_color(colors.border)
                    .pt_4()
                    .child(
                        div()
                            .mb_4()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_sm()
                            .font_weight(MEDIUM)
                            .text_color(muted)
                            .child(icon("product/Message01Icon.svg", 16., muted))
                            .child(format!("{} {}", comments.len(), plural(comments.len()))),
                    )
                    .child(div().flex().flex_col().gap_4().children(
                        comments.iter().enumerate().map(|(position, comment)| {
                            div()
                                .rounded(px(8.))
                                .border_1()
                                .border_color(colors.border)
                                .bg(colors.muted.opacity(0.5))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .px_4()
                                        .py(px(10.))
                                        .border_b_1()
                                        .border_color(colors.border)
                                        .text_sm()
                                        .text_color(muted)
                                        .child(
                                            div()
                                                .font_weight(MEDIUM)
                                                .child(comment.author.clone().unwrap_or_default()),
                                        )
                                        .child(format!(
                                            "commented on {}",
                                            date(&comment.created_at)
                                        )),
                                )
                                .child(div().px_4().py_3().map(|view| {
                                    match &comment.body {
                                        Some(body) => view.child(self.markdown(
                                            body,
                                            &format!("task-comment-{index}-{position}"),
                                            colors,
                                            cx,
                                        )),
                                        None => view
                                            .text_sm()
                                            .italic()
                                            .text_color(muted)
                                            .child("No content."),
                                    }
                                }))
                        }),
                    ))
            });
        view.child(header)
            .children(labels)
            .children(assignees)
            .child(body)
            .children(comments)
    }
}

impl TaskView {
    /// The section nearest the top of the scroll area, as shipping's scroll listener picks it.
    fn track_active(&mut self, cx: &mut Context<Self>) {
        let top = self.scroll.bounds().top();
        let active = (0..self.resources.len())
            .filter_map(|index| {
                let bounds = self.scroll.bounds_for_item(index)?;
                Some((index, f32::from((bounds.top() - top).abs())))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index);
        if active != self.active {
            self.active = active;
            cx.notify();
        }
    }
}

impl Render for TaskView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let resources = self.resources.clone();
        let nav = (resources.len() > 1).then(|| {
            div()
                .w(px(160.))
                .flex_none()
                .px_2()
                .py_6()
                .flex()
                .flex_col()
                .gap(px(2.))
                .children(resources.iter().enumerate().map(|(index, resource)| {
                    let active = self.active == Some(index);
                    div()
                        .id(SharedString::from(format!("task-nav-{index}")))
                        .w_full()
                        .rounded(px(6.))
                        .px_2()
                        .py(px(6.))
                        .text_xs()
                        .line_height(px(16.))
                        .cursor_pointer()
                        .text_color(if active {
                            colors.foreground
                        } else {
                            colors.muted_foreground
                        })
                        .when(active, |item| item.font_weight(MEDIUM))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.scroll.scroll_to_item(index);
                            this.active = Some(index);
                            cx.notify();
                        }))
                        .child(format!(
                            "{}/{} #{}",
                            resource.owner, resource.repo, resource.number
                        ))
                }))
        });
        // The nav sits beside the scroll area, where shipping keeps it with `sticky top-0`.
        div()
            .size_full()
            .flex()
            .child(
                div()
                    .id("task-scroll")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .on_scroll(cx.listener(|this, _, _, cx| this.track_active(cx)))
                    .children(resources.iter().enumerate().map(|(index, resource)| {
                        div()
                            .when(index > 0, |view| {
                                view.child(
                                    div()
                                        .max_w(px(768.))
                                        .px_6()
                                        .child(div().border_t_2().border_color(colors.border)),
                                )
                            })
                            .child(self.resource(index, resource, &colors, cx))
                    }))
                    .child(div().h(px(80.))),
            )
            .children(nav)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn chip_click_collects_github_chips_from_its_list_item() {
        let chip = |kind: &str, number: u64| json!({"type": "appLink", "attrs": {"provider": "github", "kind": kind, "owner": "acme", "repo": "app", "number": number, "url": "https://github.com/acme/app"}});
        let paragraph = |content: Vec<Value>| json!({"type": "paragraph", "content": content});
        let item =
            |content: Vec<Value>| json!({"type": "listItem", "content": [paragraph(content)]});
        let body = json!({"type": "doc", "content": [
            paragraph(vec![chip("issue", 1)]),
            {"type": "bulletList", "content": [
                item(vec![json!({"type": "text", "text": "Fix "}), chip("issue", 2), chip("pull_request", 3), chip("commit", 4)]),
                item(vec![chip("issue", 5)]),
            ]},
        ]});
        let document = Document::parse(body.to_string().into()).unwrap();
        // Inside the first list item: doc > bulletList > listItem > paragraph, after "Fix ".
        let position = 3 + 3 + 4;
        let found = sibling_resources(&document, position, &chip("issue", 2));
        assert_eq!(
            found.iter().map(|r| (r.pull, r.number)).collect::<Vec<_>>(),
            [(false, 2), (true, 3)]
        );
        let Route::Task(key) = route(&found) else {
            unreachable!()
        };
        assert_eq!(resources(&key), found);
        // Outside a list, every issue and PR chip in the note.
        assert_eq!(sibling_resources(&document, 1, &chip("issue", 1)).len(), 4);
        assert!(sibling_resources(&document, position, &chip("commit", 4)).is_empty());
    }
}
