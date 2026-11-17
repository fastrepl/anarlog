//! Settings › Ticket (`settings/todo/*`): GitHub with its repository filter and private-repo
//! connection, and Apple Reminders access on macOS. Reached after OAuth with `return_to=todo`.

use std::{sync::Arc, time::Duration};

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, Entity, Focusable, Global, Hsla, SharedString,
    Subscription, Transformation, Window, div, img, percentage, prelude::*, px,
};
use serde_json::Value;

use super::{
    PlanRequired, account,
    controls::{self, icon},
    integrations::{self, Connection},
    local::imports_view::spinner,
    preferences_view::PreferencesView,
};
use crate::{
    application::toasts,
    ui::{
        input::{InputEvent, TextInput},
        theme::{MEDIUM, Theme, theme},
    },
};

const REPOSITORY: &str = "todo_github_repository";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Authorized,
    Denied,
    NotDetermined,
}

/// Reminders authorization (`usePermission("reminders")`).
pub trait Reminders: Send + Sync {
    fn status(&self) -> Access;
    fn request(&self) -> Access;
}

/// Replaces the system Reminders permission (tests and screenshots).
#[derive(Clone)]
pub struct RemindersOverride(pub Arc<dyn Reminders>);

impl Global for RemindersOverride {}

struct SystemReminders;

impl Reminders for SystemReminders {
    #[cfg(target_os = "macos")]
    fn status(&self) -> Access {
        match anlg_apple_todo::Handle::authorization_status() {
            anlg_apple_todo::ReminderAuthStatus::Authorized => Access::Authorized,
            anlg_apple_todo::ReminderAuthStatus::NotDetermined => Access::NotDetermined,
            _ => Access::Denied,
        }
    }

    #[cfg(target_os = "macos")]
    fn request(&self) -> Access {
        anlg_apple_todo::Handle::request_full_access();
        self.status()
    }

    #[cfg(not(target_os = "macos"))]
    fn status(&self) -> Access {
        Access::Denied
    }

    #[cfg(not(target_os = "macos"))]
    fn request(&self) -> Access {
        Access::Denied
    }
}

fn reminders(cx: &App) -> Arc<dyn Reminders> {
    cx.try_global::<RemindersOverride>()
        .map(|installed| installed.0.clone())
        .unwrap_or_else(|| Arc::new(SystemReminders))
}

/// `isGitHubRepository`.
pub fn is_repository(value: &str) -> bool {
    let valid = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    };
    matches!(value.split_once('/'), Some((owner, repo)) if valid(owner) && valid(repo))
}

pub struct Ticket {
    open: [bool; 2],
    adding: bool,
    input: Entity<TextInput>,
    suggestions: Vec<String>,
    show_suggestions: bool,
    search: u64,
    connections: Option<Vec<Connection>>,
    opening: Option<&'static str>,
    reminders: Option<Access>,
    requesting: bool,
    trouble: bool,
}

impl Ticket {
    pub fn new(cx: &mut Context<PreferencesView>) -> (Self, Subscription) {
        let input = cx.new(|cx| TextInput::new("Search or type owner/repo", cx).inline());
        let subscription = cx.subscribe(&input, |this, input, event: &InputEvent, cx| {
            match event {
                InputEvent::Submitted => {
                    let value = input.read(cx).buffer.text.trim().to_owned();
                    if is_repository(&value) {
                        this.select_repository(value, cx);
                    }
                }
                _ => this.search_repositories(cx),
            }
            cx.notify();
        });
        let ticket = Self {
            open: [false; 2],
            adding: false,
            input,
            suggestions: Vec::new(),
            show_suggestions: false,
            search: 0,
            connections: None,
            opening: None,
            reminders: None,
            requesting: false,
            trouble: false,
        };
        (ticket, subscription)
    }
}

impl PreferencesView {
    /// Refreshes the GitHub connection and Reminders access when the page opens.
    pub(super) fn load_ticket(&mut self, cx: &mut Context<Self>) {
        let reminders = reminders(cx);
        let status = cx
            .background_executor()
            .spawn(async move { reminders.status() });
        cx.spawn(async move |this, cx| {
            let status = status.await;
            let _ = this.update(cx, |this, cx| {
                this.ticket.reminders = Some(status);
                cx.notify();
            });
        })
        .detach();
        let service = integrations::installed(cx).filter(|_| account::is_paid(cx));
        let Some(service) = service else {
            return;
        };
        let request = service.connections();
        cx.spawn(async move |this, cx| {
            let connections = request.await.ok();
            let _ = this.update(cx, |this, cx| {
                this.ticket.connections = connections;
                cx.notify();
            });
        })
        .detach();
    }

    fn select_repository(&mut self, repository: String, cx: &mut Context<Self>) {
        self.set(REPOSITORY, Value::String(repository), cx);
        self.close_repository_input(cx);
    }

    fn close_repository_input(&mut self, cx: &mut Context<Self>) {
        self.ticket.adding = false;
        self.ticket.show_suggestions = false;
        self.ticket.suggestions.clear();
        self.ticket.search += 1;
        self.ticket
            .input
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        cx.notify();
    }

    /// Debounced by 300 ms like shipping's repository search.
    fn search_repositories(&mut self, cx: &mut Context<Self>) {
        self.ticket.search += 1;
        self.ticket.show_suggestions = true;
        let generation = self.ticket.search;
        let query = self.ticket.input.read(cx).buffer.text.trim().to_owned();
        if query.chars().count() < 2 {
            self.ticket.suggestions.clear();
            return;
        }
        let github = crate::workspace::task::github(&self.runtime, cx);
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            if this
                .update(cx, |this, _| this.ticket.search != generation)
                .unwrap_or(true)
            {
                return;
            }
            let found = crate::workspace::task::search_repositories(github, query).await;
            let _ = this.update(cx, |this, cx| {
                if this.ticket.search == generation {
                    this.ticket.suggestions = found;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// `openIntegration` for GitHub, returning to this page.
    fn open_github(
        &mut self,
        action: &'static str,
        connection: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = integrations::installed(cx) else {
            return;
        };
        if self.ticket.opening.is_some() {
            return;
        }
        self.ticket.opening = Some(action);
        cx.spawn(async move |this, cx| {
            let result = service.open("github", action, connection, "todo").await;
            let _ = this.update(cx, |this, cx| {
                this.ticket.opening = None;
                if result.is_err() {
                    toasts::error(integrations::SETUP_ERROR, cx);
                }
                this.load_ticket(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn request_reminders(&mut self, cx: &mut Context<Self>) {
        if self.ticket.requesting {
            return;
        }
        self.ticket.requesting = true;
        let reminders = reminders(cx);
        let status = cx
            .background_executor()
            .spawn(async move { reminders.request() });
        cx.spawn(async move |this, cx| {
            let status = status.await;
            let _ = this.update(cx, |this, cx| {
                this.ticket.requesting = false;
                this.ticket.reminders = Some(status);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn ticket(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(window);
        let mut providers = vec![(
            "GitHub",
            img("product/brands/github-icon.svg")
                .size(px(20.))
                .into_any_element(),
            self.github(&colors, cx).into_any_element(),
        )];
        if cfg!(target_os = "macos") {
            providers.push((
                "Apple Reminders",
                img("product/brands/apple-reminders.png")
                    .size(px(20.))
                    .rounded(px(4.))
                    .into_any_element(),
                self.apple_reminders(&colors, cx).into_any_element(),
            ));
        }
        let count = providers.len();
        let accent = colors.accent;
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(controls::page_title("Ticket"))
            .child(div().children(providers.into_iter().enumerate().map(
                |(index, (name, logo, content))| {
                    let open = self.ticket.open[index];
                    let group = SharedString::from(format!("ticket-provider-{index}"));
                    div()
                        .when(index + 1 < count, |item| {
                            item.border_b_1().border_color(colors.border)
                        })
                        .child(
                            div()
                                .id(group.clone())
                                .group(group.clone())
                                .flex()
                                .items_center()
                                .gap_1()
                                .rounded(px(8.))
                                .cursor_pointer()
                                .hover(move |style| style.bg(accent))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.ticket.open[index] = !this.ticket.open[index];
                                    cx.notify();
                                }))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .py_3()
                                        .text_sm()
                                        .line_height(px(20.))
                                        .font_weight(MEDIUM)
                                        .child(logo)
                                        .child(name),
                                )
                                .child(
                                    icon(
                                        "product/ArrowDown01Icon.svg",
                                        16.,
                                        colors.muted_foreground,
                                    )
                                    .opacity(0.)
                                    .group_hover(group, |style| style.opacity(1.))
                                    .when(open, |caret| {
                                        caret.with_transformation(Transformation::rotate(
                                            percentage(0.5),
                                        ))
                                    }),
                                ),
                        )
                        .when(open, |item| {
                            item.child(
                                div()
                                    .pb_3()
                                    .text_sm()
                                    .flex()
                                    .flex_col()
                                    .gap_3()
                                    .child(content),
                            )
                        })
                },
            )))
            .into_any_element()
    }

    fn github(&self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let account = account::current(cx);
        let paid = account.is_paid(account::now());
        let muted = colors.muted_foreground;
        let foreground = colors.foreground;
        let link = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap_1()
                .underline()
                .cursor_pointer()
                .hover(move |style| style.text_color(foreground))
                .child(label)
        };
        let busy = |action: &'static str| {
            (self.ticket.opening == Some(action))
                .then(|| spinner(format!("ticket-{action}"), 12., muted))
        };
        let connection = self
            .ticket
            .connections
            .as_ref()
            .and_then(|connections| integrations::find(connections, "github"))
            .map(|connection| connection.connection_id.clone());
        let access: AnyElement =
            if !account.signed_in() {
                div()
                    .child("Sign in for private repo access.")
                    .into_any_element()
            } else if !paid {
                link("ticket-github-plan", "Connect GitHub for private repos.")
                    .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(PlanRequired)))
                    .into_any_element()
            } else if let Some(connection) = connection {
                link(
                    "ticket-github-disconnect",
                    "Disconnect private repo access.",
                )
                .children(busy("disconnect"))
                .when(self.ticket.opening.is_some(), |link| link.opacity(0.5))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.open_github("disconnect", Some(connection.clone()), cx)
                }))
                .into_any_element()
            } else {
                link("ticket-github-connect", "Connect GitHub for private repos.")
                    .children(busy("connect"))
                    .when(self.ticket.opening.is_some(), |link| link.opacity(0.5))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.open_github("connect", None, cx)
                    }))
                    .into_any_element()
            };
        let repository = self.text(REPOSITORY).trim().to_owned();
        let has_repository = !repository.is_empty();
        let small_button = |id: &'static str, path: &'static str| {
            div().id(id).cursor_pointer().child(icon(path, 14., muted))
        };
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_x_1()
                    .text_xs()
                    .line_height(px(16.))
                    .text_color(muted)
                    .child("Only public repositories are supported.")
                    .child(access),
            )
            .when(has_repository && !self.ticket.adding, |view| {
                let url = format!("https://github.com/{repository}");
                view.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().text_sm().text_color(muted).child(repository.clone()))
                        .child(
                            small_button("ticket-repository-open", "product/ExternalLinkIcon.svg")
                                .on_click(move |_: &ClickEvent, _, cx| cx.open_url(&url)),
                        )
                        .child(
                            small_button("ticket-repository-remove", "product/XIcon.svg").on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.set(REPOSITORY, Value::String(String::new()), cx)
                                }),
                            ),
                        ),
                )
            })
            .child(if self.ticket.adding {
                self.repository_input(colors, cx).into_any_element()
            } else {
                div()
                    .id("ticket-repository-add")
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_xs()
                    .line_height(px(16.))
                    .text_color(muted)
                    .cursor_pointer()
                    .hover(move |style| style.text_color(foreground))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.ticket.adding = true;
                        this.ticket
                            .input
                            .update(cx, |input, cx| input.focus_handle(cx).focus(window, cx));
                        cx.notify();
                    }))
                    .child(icon("product/PlusIcon.svg", 12., muted))
                    .child(if has_repository {
                        "Replace repository"
                    } else {
                        "Add repository"
                    })
                    .into_any_element()
            })
    }

    fn repository_input(&self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let value = self.ticket.input.read(cx).buffer.text.trim().to_owned();
        let valid = is_repository(&value);
        let muted = colors.muted_foreground;
        let foreground = colors.foreground;
        let accent = colors.accent;
        let suggestions = (self.ticket.show_suggestions && !self.ticket.suggestions.is_empty())
            .then(|| {
                controls::dropdown(
                    div()
                        .mt_1()
                        .w(px(288.))
                        .p_1()
                        .rounded(px(8.))
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.popover)
                        .shadow_md()
                        .occlude()
                        .children(self.ticket.suggestions.iter().enumerate().map(
                            |(index, repo)| {
                                let repo = repo.clone();
                                div()
                                    .id(SharedString::from(format!("ticket-suggestion-{index}")))
                                    .px_3()
                                    .py(px(6.))
                                    .text_sm()
                                    .text_color(muted)
                                    .cursor_pointer()
                                    .hover(move |style| style.bg(accent))
                                    .child(repo.clone())
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.select_repository(repo.clone(), cx)
                                    }))
                            },
                        )),
                    false,
                )
            });
        let text_button = |id: &'static str, label: &'static str, enabled: bool| {
            div()
                .id(id)
                .text_xs()
                .underline()
                .text_color(muted)
                .when(!enabled, |button| button.opacity(0.4))
                .when(enabled, |button| {
                    button
                        .cursor_pointer()
                        .hover(move |style| style.text_color(foreground))
                })
                .child(label)
        };
        div()
            .relative()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .relative()
                    .flex_1()
                    .h(px(36.))
                    .px_3()
                    .flex()
                    .items_center()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(colors.input)
                    .shadow_xs()
                    .text_sm()
                    .child(div().flex_1().h(px(20.)).child(self.ticket.input.clone()))
                    .children(suggestions),
            )
            .child(
                text_button("ticket-repository-submit", "Add", valid).when(valid, |button| {
                    button.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.select_repository(value.clone(), cx)
                    }))
                }),
            )
            .child(
                text_button("ticket-repository-cancel", "Cancel", true).on_click(
                    cx.listener(|this, _: &ClickEvent, _, cx| this.close_repository_input(cx)),
                ),
            )
    }

    /// `AccessPermissionRow` until Reminders is authorized, then `TroubleShootingLink`.
    fn apple_reminders(&self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let authorized = self.ticket.reminders == Some(Access::Authorized);
        let trouble = self.troubleshooting(colors, cx);
        if authorized {
            return div().child(trouble);
        }
        let red: Hsla = gpui::rgb(0xfb2c36).into();
        let status = self.ticket.reminders;
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .py_2()
            .child(
                div()
                    .flex_1()
                    .child(
                        div()
                            .mb_1()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_color(red)
                            .child(icon("product/AlertCircleIcon.svg", 16., red))
                            .child(div().text_sm().font_weight(MEDIUM).child("Reminders")),
                    )
                    .child(trouble),
            )
            .child(
                div()
                    .id("ticket-reminders-action")
                    .size(px(32.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.))
                    .bg(colors.primary)
                    .when(self.ticket.requesting, |button| button.opacity(0.5))
                    .when(!self.ticket.requesting, |button| {
                        button.cursor_pointer().on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| match status {
                                Some(Access::Denied) => open_reminders_settings(cx),
                                _ => this.request_reminders(cx),
                            },
                        ))
                    })
                    .child(icon(
                        "product/ArrowRight02Icon.svg",
                        20.,
                        colors.primary_foreground,
                    )),
            )
    }

    /// `TroubleShootingLink`; the native permission service has no reset.
    fn troubleshooting(&self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let foreground = colors.foreground;
        let link = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .underline()
                .cursor_pointer()
                .hover(move |style| style.text_color(foreground))
                .child(label)
        };
        let row = div()
            .text_xs()
            .line_height(px(16.))
            .text_color(colors.muted_foreground);
        if !self.ticket.trouble {
            return row.flex().child(
                link("ticket-reminders-trouble", "Having trouble?").on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.ticket.trouble = true;
                        cx.notify();
                    },
                )),
            );
        }
        row.flex()
            .flex_wrap()
            .gap_1()
            .child("You can")
            .child(
                link("ticket-reminders-request", "Request")
                    .when(self.ticket.requesting, |link| link.opacity(0.5))
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| this.request_reminders(cx)),
                    ),
            )
            .child("or")
            .child(
                link("ticket-reminders-open", "Open")
                    .on_click(|_: &ClickEvent, _, cx| open_reminders_settings(cx)),
            )
            .child("permission panel.")
            .child(link("ticket-reminders-back", "Back").on_click(cx.listener(
                |this, _: &ClickEvent, _, cx| {
                    this.ticket.trouble = false;
                    cx.notify();
                },
            )))
    }
}

fn open_reminders_settings(cx: &mut App) {
    cx.open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Reminders");
}

#[cfg(test)]
mod tests {
    use super::is_repository;

    #[test]
    fn repository_filter_accepts_only_owner_and_repo() {
        assert!(is_repository("zed-industries/zed.dev"));
        for invalid in ["zed", "a/b/c", "/zed", "owner/", "own er/repo"] {
            assert!(!is_repository(invalid), "{invalid}");
        }
    }
}
