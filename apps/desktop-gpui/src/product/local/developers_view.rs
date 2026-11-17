//! Settings › Developers (shipping's `settings/developers`): CLI & MCP, agent skills, Cloud API &
//! Connectors and webhooks.

use std::{sync::Arc, time::Duration};

use desktop_runtime::{Result, RuntimeHandle};
use futures::future::BoxFuture;
use gpui::{
    Animation, AnimationExt, AnyElement, ClipboardItem, Context, Div, ElementId, Entity,
    EventEmitter, Global, Render, SharedString, Stateful, Subscription, Transformation, Window,
    div, img, percentage, prelude::*, px, rgb, svg,
};
use sqlx::SqlitePool;

use super::{
    developers::{CliState, CliStatus, DeveloperTools, SkillAgent, SkillAgentStatus},
    failure,
    webhooks::{self, WebhookInfo},
};
use crate::{
    application::toasts,
    product::{
        PlanRequired, account,
        account_view::plan_gate,
        cloud::cloud_api::{CloudApi, CloudApiKey},
        controls,
    },
    ui::{
        input::{InputEvent, TextInput},
        theme::{MEDIUM, Theme, monospace_font, theme},
    },
};

const GUIDE_URL: &str = "https://docs.anarlog.so/agents/overview";

/// The page's local services, installed once the native services start.
#[derive(Clone)]
pub struct Developers {
    pub runtime: RuntimeHandle,
    pub tools: DeveloperTools,
}

impl Global for Developers {}

impl Developers {
    fn blocking<T: Send + 'static>(
        &self,
        work: impl FnOnce(DeveloperTools) -> std::result::Result<T, String> + Send + 'static,
    ) -> BoxFuture<'static, Result<T>> {
        let (runtime, tools) = (self.runtime.clone(), self.tools.clone());
        Box::pin(async move {
            runtime
                .service(move |_| async move {
                    tokio::task::spawn_blocking(move || work(tools))
                        .await
                        .map_err(failure)?
                        .map_err(failure)
                })?
                .receive()
                .await
        })
    }

    fn db<T: Send + 'static, F>(
        &self,
        work: impl FnOnce(SqlitePool) -> F + Send + 'static,
    ) -> BoxFuture<'static, Result<T>>
    where
        F: Future<Output = Result<T>> + Send + 'static,
    {
        let runtime = self.runtime.clone();
        Box::pin(async move {
            runtime
                .service(move |services| work(services.db.pool().clone()))?
                .receive()
                .await
        })
    }
}

enum Load<T> {
    Pending,
    Ready(T),
    Failed(String),
}

#[derive(Clone, Copy, PartialEq)]
enum Variant {
    Primary,
    Outline,
    Ghost,
}

#[derive(Clone, Copy, PartialEq)]
enum Tone {
    Success,
    Error,
}

pub struct DevelopersView {
    cli: Load<CliStatus>,
    installing: bool,
    agents: Vec<SkillAgentStatus>,
    skill_menu: bool,
    adding_skill: bool,
    /// The account the Cloud API state below belongs to.
    cloud_user: Option<Arc<str>>,
    settings: Load<bool>,
    settings_requested: bool,
    toggling: bool,
    keys: Vec<CloudApiKey>,
    created_key: Option<String>,
    creating_key: bool,
    key_name: Entity<TextInput>,
    webhooks: Vec<WebhookInfo>,
    created_secret: Option<String>,
    creating_webhook: bool,
    testing: bool,
    webhook_url: Entity<TextInput>,
    loaded_local: bool,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<PlanRequired> for DevelopersView {}

impl DevelopersView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let key_name = cx.new(|cx| TextInput::new("Key name (e.g. Claude Code)", cx).inline());
        let webhook_url =
            cx.new(|cx| TextInput::new("https://example.com/webhooks/anarlog", cx).inline());
        let subscriptions = vec![
            cx.subscribe(&key_name, |this, _, event, cx| match event {
                InputEvent::Submitted => this.create_key(cx),
                _ => cx.notify(),
            }),
            cx.subscribe(&webhook_url, |this, _, event, cx| match event {
                InputEvent::Submitted => this.create_webhook(cx),
                _ => cx.notify(),
            }),
            cx.observe_global::<Developers>(|this, cx| this.load_local(cx)),
            cx.observe_global::<account::Account>(|this, cx| this.sync_cloud(cx)),
            cx.observe_global::<CloudApi>(|this, cx| this.sync_cloud(cx)),
        ];
        let mut view = Self {
            cli: Load::Pending,
            installing: false,
            agents: Vec::new(),
            skill_menu: false,
            adding_skill: false,
            cloud_user: None,
            settings: Load::Pending,
            settings_requested: false,
            toggling: false,
            keys: Vec::new(),
            created_key: None,
            creating_key: false,
            key_name,
            webhooks: Vec::new(),
            created_secret: None,
            creating_webhook: false,
            testing: false,
            webhook_url,
            loaded_local: false,
            _subscriptions: subscriptions,
        };
        view.load_local(cx);
        view.sync_cloud(cx);
        view
    }

    fn run<T: 'static>(
        &mut self,
        work: BoxFuture<'static, Result<T>>,
        done: impl FnOnce(&mut Self, Result<T>, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                done(this, result, cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Shipping's `toast.success` / `toast.error`.
    fn toast(&mut self, tone: Tone, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        match tone {
            Tone::Success => toasts::success(message, cx),
            Tone::Error => toasts::error(message, cx),
        }
    }

    fn copy(&mut self, text: String, message: &'static str, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.toast(Tone::Success, message, cx);
    }

    fn load_local(&mut self, cx: &mut Context<Self>) {
        let Some(developers) = cx.try_global::<Developers>().cloned() else {
            return;
        };
        if std::mem::replace(&mut self.loaded_local, true) {
            return;
        }
        self.run(
            developers.blocking(|tools| Ok(tools.cli_status())),
            |this, result, _| {
                this.cli = match result {
                    Ok(status) => Load::Ready(status),
                    Err(error) => Load::Failed(error.to_string()),
                }
            },
            cx,
        );
        self.load_agents(cx);
        self.load_webhooks(cx);
    }

    fn load_agents(&mut self, cx: &mut Context<Self>) {
        let Some(developers) = cx.try_global::<Developers>().cloned() else {
            return;
        };
        self.run(
            developers.blocking(|tools| Ok(tools.skill_agents())),
            |this, result, _| {
                if let Ok(agents) = result {
                    this.agents = agents;
                }
            },
            cx,
        );
    }

    fn install_cli(&mut self, cx: &mut Context<Self>) {
        let Some(developers) = cx.try_global::<Developers>().cloned() else {
            return;
        };
        self.installing = true;
        self.run(
            developers.blocking(|tools| tools.install_cli()),
            |this, result, cx| {
                this.installing = false;
                match result {
                    Ok(status) => {
                        if status.state == CliState::Installed {
                            let message = format!("{} is ready to use", status.command_name);
                            this.toast(Tone::Success, message, cx);
                        } else {
                            let message = status.details.clone().unwrap_or_else(|| {
                                format!("{} could not be installed", status.command_name)
                            });
                            this.toast(Tone::Error, message, cx);
                        }
                        this.cli = Load::Ready(status);
                    }
                    Err(error) => this.toast(Tone::Error, error.to_string(), cx),
                }
            },
            cx,
        );
    }

    fn add_skill(&mut self, agents: Vec<SkillAgent>, cx: &mut Context<Self>) {
        let Some(developers) = cx.try_global::<Developers>().cloned() else {
            return;
        };
        self.skill_menu = false;
        self.adding_skill = true;
        self.run(
            developers.blocking(move |tools| {
                agents
                    .into_iter()
                    .map(|agent| tools.install_skill(agent))
                    .collect::<std::result::Result<Vec<_>, _>>()
            }),
            |this, result, cx| {
                this.adding_skill = false;
                match result {
                    Ok(added) => {
                        let message = match added.as_slice() {
                            [one] => format!("Anarlog skill added to {}", one.agent.display_name()),
                            many => format!("Anarlog skill added to {} agents", many.len()),
                        };
                        this.toast(Tone::Success, message, cx);
                    }
                    Err(error) => this.toast(Tone::Error, error.to_string(), cx),
                }
                this.load_agents(cx);
            },
            cx,
        );
    }

    /// Resets the Cloud API state for a new account and loads it once the account is Pro.
    fn sync_cloud(&mut self, cx: &mut Context<Self>) {
        let current = account::current(cx);
        let user = current.user.as_ref().map(|user| user.id.clone());
        if user != self.cloud_user {
            self.cloud_user = user;
            self.settings = Load::Pending;
            self.settings_requested = false;
            self.keys.clear();
            self.created_key = None;
        }
        let ready = current.claims.is_some();
        if !ready || !account::is_pro(cx) || self.settings_requested {
            cx.notify();
            return;
        }
        let Some(api) = cx.try_global::<CloudApi>().cloned() else {
            return;
        };
        self.settings_requested = true;
        self.run(
            api.settings(),
            |this, result, cx| match result {
                Ok(enabled) => {
                    this.settings = Load::Ready(enabled);
                    if enabled {
                        this.load_keys(cx);
                    }
                }
                Err(error) => this.settings = Load::Failed(error.to_string()),
            },
            cx,
        );
    }

    fn toggle_cloud(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let Some(api) = cx.try_global::<CloudApi>().cloned() else {
            return;
        };
        self.toggling = true;
        self.run(
            api.set_enabled(enabled),
            |this, result, cx| {
                this.toggling = false;
                match result {
                    Ok(toggled) => {
                        this.settings = Load::Ready(toggled.enabled);
                        if toggled.enabled {
                            this.load_keys(cx);
                        }
                        let (tone, message) = if toggled.backfill_failed {
                            (Tone::Error, "Cloud API enabled, but existing meetings could not be uploaded. Anarlog will retry.".to_string())
                        } else if toggled.enabled && toggled.uploaded == 1 {
                            (
                                Tone::Success,
                                "Cloud API enabled — 1 meeting uploaded".into(),
                            )
                        } else if toggled.enabled {
                            (
                                Tone::Success,
                                format!(
                                    "Cloud API enabled — {} meetings uploaded",
                                    toggled.uploaded
                                ),
                            )
                        } else {
                            (
                                Tone::Success,
                                "Cloud API disabled and readable copies deleted".into(),
                            )
                        };
                        this.toast(tone, message, cx);
                    }
                    Err(error) => this.toast(Tone::Error, error.to_string(), cx),
                }
            },
            cx,
        );
    }

    fn load_keys(&mut self, cx: &mut Context<Self>) {
        let Some(api) = cx.try_global::<CloudApi>().cloned() else {
            return;
        };
        self.run(
            api.keys(),
            |this, result, _| {
                if let Ok(keys) = result {
                    this.keys = keys;
                }
            },
            cx,
        );
    }

    fn create_key(&mut self, cx: &mut Context<Self>) {
        let Some(api) = cx.try_global::<CloudApi>().cloned() else {
            return;
        };
        if self.creating_key || !matches!(self.settings, Load::Ready(true)) {
            return;
        }
        let name = self.key_name.read(cx).buffer.text.trim().to_owned();
        let name = if name.is_empty() {
            "Connector".into()
        } else {
            name
        };
        self.key_name
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        self.creating_key = true;
        self.run(
            api.create_key(name),
            |this, result, cx| {
                this.creating_key = false;
                match result {
                    Ok(created) => {
                        this.created_key = Some(created.key);
                        this.load_keys(cx);
                    }
                    Err(error) => this.toast(Tone::Error, error.to_string(), cx),
                }
            },
            cx,
        );
    }

    fn revoke_key(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(api) = cx.try_global::<CloudApi>().cloned() else {
            return;
        };
        self.run(
            api.revoke_key(id),
            |this, result, cx| match result {
                Ok(()) => this.load_keys(cx),
                Err(error) => this.toast(Tone::Error, error.to_string(), cx),
            },
            cx,
        );
    }

    fn load_webhooks(&mut self, cx: &mut Context<Self>) {
        let Some(developers) = cx.try_global::<Developers>().cloned() else {
            return;
        };
        self.run(
            developers.db(|pool| async move { webhooks::list(&pool).await }),
            |this, result, _| {
                if let Ok(list) = result {
                    this.webhooks = list;
                }
            },
            cx,
        );
    }

    fn create_webhook(&mut self, cx: &mut Context<Self>) {
        let Some(developers) = cx.try_global::<Developers>().cloned() else {
            return;
        };
        let url = self.webhook_url.read(cx).buffer.text.trim().to_owned();
        if url.is_empty() || self.creating_webhook {
            return;
        }
        self.webhook_url
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        self.creating_webhook = true;
        self.run(
            developers.db(move |pool| async move { webhooks::create(&pool, &url, &[]).await }),
            |this, result, cx| {
                this.creating_webhook = false;
                match result {
                    Ok(created) => {
                        this.created_secret = Some(created.secret);
                        this.load_webhooks(cx);
                    }
                    Err(error) => this.toast(Tone::Error, error.to_string(), cx),
                }
            },
            cx,
        );
    }

    fn webhook_action(&mut self, id: String, action: WebhookAction, cx: &mut Context<Self>) {
        let Some(developers) = cx.try_global::<Developers>().cloned() else {
            return;
        };
        if action == WebhookAction::Test {
            self.testing = true;
        }
        self.run(
            developers.db(move |pool| async move {
                match action {
                    WebhookAction::Delete => webhooks::delete(&pool, &id).await.map(|()| None),
                    WebhookAction::SetActive(active) => {
                        webhooks::set_active(&pool, &id, active).await.map(|_| None)
                    }
                    WebhookAction::Test => webhooks::test(&pool, &id).await.map(Some),
                }
            }),
            move |this, result, cx| {
                if action == WebhookAction::Test {
                    this.testing = false;
                }
                match result {
                    Ok(Some(delivery)) if delivery.delivered => {
                        this.toast(
                            Tone::Success,
                            format!("Test delivered ({})", delivery.status),
                            cx,
                        );
                        this.load_webhooks(cx);
                    }
                    Ok(Some(delivery)) => {
                        this.toast(
                            Tone::Error,
                            format!("Test failed ({})", delivery.status),
                            cx,
                        );
                        this.load_webhooks(cx);
                    }
                    Ok(None) => this.load_webhooks(cx),
                    Err(error) => this.toast(Tone::Error, error.to_string(), cx),
                }
            },
            cx,
        );
    }
}

#[derive(Clone, Copy, PartialEq)]
enum WebhookAction {
    Delete,
    SetActive(bool),
    Test,
}

fn button(
    id: impl Into<ElementId>,
    variant: Variant,
    enabled: bool,
    colors: &Theme,
) -> Stateful<Div> {
    let (primary, accent) = (colors.primary, colors.accent);
    div()
        .id(id)
        .h(px(28.))
        .px_2()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .gap_2()
        .text_xs()
        .line_height(px(16.))
        .font_weight(MEDIUM)
        .whitespace_nowrap()
        .map(|button| match variant {
            Variant::Primary => button
                .bg(primary)
                .text_color(colors.primary_foreground)
                .shadow_xs()
                .when(enabled, |button| {
                    button.hover(move |style| style.bg(primary.opacity(0.9)))
                }),
            Variant::Outline => button
                .border_1()
                .border_color(colors.input)
                .bg(colors.background)
                .shadow_xs()
                .when(enabled, |button| {
                    button.hover(move |style| style.bg(accent))
                }),
            Variant::Ghost => button.when(enabled, |button| {
                button.hover(move |style| style.bg(accent))
            }),
        })
        .when(!enabled, |button| button.opacity(0.5))
        .when(enabled, |button| button.cursor_pointer())
}

fn spinner(id: &'static str, size: f32, color: gpui::Hsla) -> AnyElement {
    controls::icon("product/LoaderCircleIcon.svg", size, color)
        .with_animation(
            id,
            Animation::new(Duration::from_secs(1)).repeat(),
            |icon, delta| icon.with_transformation(Transformation::rotate(percentage(delta))),
        )
        .into_any_element()
}

fn small(colors: &Theme) -> Div {
    div()
        .text_xs()
        .line_height(px(16.))
        .text_color(colors.muted_foreground)
}

fn label(text: &'static str) -> Div {
    div()
        .text_sm()
        .line_height(px(20.))
        .font_weight(MEDIUM)
        .child(text)
}

fn code(text: impl Into<SharedString>, colors: &Theme, cx: &gpui::App) -> Div {
    div()
        .min_w_0()
        .overflow_hidden()
        .px(px(6.))
        .py(px(2.))
        .bg(colors.muted)
        .font_family(monospace_font(cx))
        .text_xs()
        .line_height(px(16.))
        .whitespace_nowrap()
        .child(text.into())
}

/// A row with a title (and detail) on the left and its control on the right.
fn row(left: impl IntoElement, right: impl IntoElement) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(div().min_w_0().child(left))
        .child(div().flex().flex_shrink_0().gap_2().child(right))
}

fn input_box(input: Entity<TextInput>, max_width: f32, colors: &Theme) -> Div {
    div()
        .flex_1()
        .max_w(px(max_width))
        .h(px(32.))
        .px_3()
        .flex()
        .items_center()
        .border_1()
        .border_color(colors.input)
        .text_sm()
        .overflow_hidden()
        .child(div().w_full().child(input))
}

fn agent_icon(agent: SkillAgent, dark: bool, colors: &Theme) -> AnyElement {
    let image = |path: &'static str| img(path).size(px(16.)).flex_shrink_0().into_any_element();
    let tinted = |path: &'static str| {
        svg()
            .path(path)
            .size(px(16.))
            .flex_shrink_0()
            .text_color(colors.popover_foreground)
            .into_any_element()
    };
    match agent {
        SkillAgent::ClaudeCode => image("product/providers/claude.svg"),
        SkillAgent::Codex => tinted("product/providers/openai.svg"),
        SkillAgent::Cursor => tinted("product/providers/cursor.svg"),
        SkillAgent::Opencode if dark => image("product/OpenCodeDark.svg"),
        SkillAgent::Opencode => image("product/OpenCodeLight.svg"),
    }
}

impl DevelopersView {
    fn cli_section(&mut self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let status = match &self.cli {
            Load::Ready(status) => Some(status.clone()),
            _ => None,
        };
        let installed = status
            .as_ref()
            .is_some_and(|status| status.state == CliState::Installed);
        let can_install = status.as_ref().is_some_and(|status| {
            status.supported
                && !matches!(status.state, CliState::ResourceMissing | CliState::Conflict)
        });
        let detail = match &self.cli {
            Load::Pending => Some(
                div()
                    .mt_1()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_xs()
                    .line_height(px(16.))
                    .text_color(colors.muted_foreground)
                    .child(spinner("cli-checking", 12., colors.muted_foreground))
                    .child("Checking…"),
            ),
            Load::Failed(error) => Some(
                div()
                    .mt_1()
                    .text_xs()
                    .line_height(px(16.))
                    .text_color(colors.destructive)
                    .child(format!("Could not check the CLI: {error}")),
            ),
            Load::Ready(status) if status.state == CliState::Installed => None,
            Load::Ready(status) => {
                let text = if matches!(
                    status.state,
                    CliState::Conflict | CliState::Unsupported | CliState::ResourceMissing
                ) {
                    status
                        .details
                        .clone()
                        .unwrap_or_else(|| "Unavailable".into())
                } else {
                    "Not installed".into()
                };
                let dot = if status.state == CliState::Conflict {
                    rgb(0xf59e0b).into()
                } else {
                    colors.muted_foreground.opacity(0.5)
                };
                Some(
                    div()
                        .mt_1()
                        .flex()
                        .items_start()
                        .gap(px(6.))
                        .text_xs()
                        .line_height(px(16.))
                        .child(div().mt_1().size(px(8.)).flex_shrink_0().bg(dot))
                        .child(div().text_color(colors.muted_foreground).child(text)),
                )
            }
        };
        let check: gpui::Hsla = if colors.background.l < 0.5 {
            rgb(0x34d399).into()
        } else {
            rgb(0x059669).into()
        };
        let cli = row(
            div()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(label("Anarlog CLI"))
                        .when(installed, |view| {
                            view.child(controls::icon(
                                "product/CheckmarkCircle02Icon.svg",
                                14.,
                                check,
                            ))
                        }),
                )
                .children(detail),
            button(
                "install-cli",
                Variant::Primary,
                can_install && !self.installing,
                colors,
            )
            .map(|button| {
                if self.installing {
                    button.child(spinner("cli-installing", 14., colors.primary_foreground))
                } else {
                    button.child(if installed { "Reinstall" } else { "Install" })
                }
            })
            .when(can_install && !self.installing, |button| {
                button.on_click(cx.listener(|this, _, _, cx| this.install_cli(cx)))
            }),
        );
        let configuration = super::developers::mcp_configuration(
            status
                .as_ref()
                .map(|status| {
                    if installed {
                        status.install_path.as_str()
                    } else {
                        status.command_name.as_str()
                    }
                })
                .unwrap_or("anarlog"),
        );
        let mcp = row(
            label("MCP server"),
            button("copy-mcp", Variant::Outline, installed, colors)
                .child(controls::icon(
                    "product/CopyIcon.svg",
                    14.,
                    colors.foreground,
                ))
                .child("Copy config")
                .when(installed, |button| {
                    button.on_click(cx.listener(move |this, _, _, cx| {
                        this.copy(configuration.clone(), "MCP configuration copied", cx)
                    }))
                }),
        );
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(controls::section_title("CLI & MCP"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(cli)
                    .child(mcp)
                    .child(self.skills_row(colors, cx)),
            )
    }

    fn skills_row(&mut self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let enabled = !self.agents.is_empty() && !self.adding_skill;
        let trigger = button("add-skill", Variant::Outline, enabled, colors)
            .map(|button| {
                if self.adding_skill {
                    button.child(spinner("skill-installing", 14., colors.foreground))
                } else {
                    button.child("Add skill to…").child(controls::icon(
                        "product/ArrowDown01Icon.svg",
                        14.,
                        colors.foreground,
                    ))
                }
            })
            .when(enabled, |button| {
                button.on_click(cx.listener(|this, _, _, cx| {
                    this.skill_menu = !this.skill_menu;
                    cx.notify();
                }))
            });
        row(
            div().child(label("Agent skills")).child(
                small(colors)
                    .mt_1()
                    .child("Teach coding agents when and how to use the Anarlog CLI and MCP"),
            ),
            div()
                .relative()
                .child(trigger)
                .when(self.skill_menu, |view| {
                    view.child(controls::dropdown(self.skill_menu(colors, cx), true))
                }),
        )
    }

    fn skill_menu(&self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let dark = colors.background.l < 0.5;
        let detected = self
            .agents
            .iter()
            .filter(|agent| agent.detected)
            .map(|agent| agent.agent)
            .collect::<Vec<_>>();
        let accent = colors.accent;
        let item = |id: ElementId, enabled: bool| {
            div()
                .id(id)
                .px_2()
                .py(px(6.))
                .flex()
                .items_center()
                .gap_2()
                .text_sm()
                .line_height(px(20.))
                .when(!enabled, |item| item.opacity(0.5))
                .when(enabled, |item| item.hover(move |style| style.bg(accent)))
        };
        let all_enabled = !detected.is_empty();
        div()
            .mt_1()
            .w(px(208.))
            .p(px(2.))
            .border_1()
            .border_color(colors.app_floating_border)
            .bg(colors.app_floating_chrome)
            .text_color(colors.popover_foreground)
            .shadow_lg()
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.skill_menu = false;
                cx.notify();
            }))
            .child(
                div()
                    .p(px(6.))
                    .border_1()
                    .border_color(colors.app_floating_border)
                    .bg(colors.app_floating_panel)
                    .flex()
                    .flex_col()
                    .child(
                        item("skill-all".into(), all_enabled)
                            .child("Install to all agents")
                            .when(all_enabled, |item| {
                                item.on_click(cx.listener(move |this, _, _, cx| {
                                    this.add_skill(detected.clone(), cx)
                                }))
                            }),
                    )
                    .child(div().mx(px(-4.)).my_1().h(px(1.)).bg(colors.muted))
                    .children(self.agents.iter().map(|status| {
                        let agent = status.agent;
                        item(
                            SharedString::from(format!("skill-{agent:?}")).into(),
                            status.detected,
                        )
                        .child(agent_icon(agent, dark, colors))
                        .child(div().flex_1().child(agent.display_name()))
                        .when(status.installed, |item| {
                            item.child(controls::icon(
                                "product/CheckIcon.svg",
                                14.,
                                colors.popover_foreground,
                            ))
                        })
                        .when(status.detected, |item| {
                            item.on_click(
                                cx.listener(move |this, _, _, cx| this.add_skill(vec![agent], cx)),
                            )
                        })
                    })),
            )
    }

    fn cloud_heading(error: Option<String>, colors: &Theme) -> Div {
        div()
            .min_w_0()
            .child(controls::section_title("Cloud API & Connectors"))
            .child(small(colors).mt_1().child(
                "Uploads meeting content for remote access while Anarlog is closed.",
            ))
            .child(small(colors).mt_1().child("Enabling it uploads a separate server-readable copy of your meeting titles, notes, summaries, participants, action items, and transcripts. Your normal cloud sync stays end-to-end encrypted. Turning it off deletes the server-readable copies."))
            .when_some(error, |view, error| {
                view.child(
                    div()
                        .mt_2()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(colors.destructive)
                        .child(error),
                )
            })
    }

    fn endpoints(&self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let (rest, mcp) = cx
            .try_global::<CloudApi>()
            .map(CloudApi::endpoints)
            .unwrap_or_default();
        let endpoint =
            |id: &'static str, title: &'static str, value: String, copied: &'static str| {
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(16.))
                            .font_weight(MEDIUM)
                            .text_color(colors.muted_foreground)
                            .child(title),
                    )
                    .child(
                        div()
                            .mt_1()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(code(value.clone(), colors, cx))
                            .child(
                                button(id, Variant::Ghost, true, colors)
                                    .size(px(28.))
                                    .p_0()
                                    .child(controls::icon(
                                        "product/CopyIcon.svg",
                                        14.,
                                        colors.foreground,
                                    ))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.copy(value.clone(), copied, cx)
                                    })),
                            ),
                    )
            };
        div()
            .flex()
            .gap_3()
            .child(endpoint(
                "copy-rest",
                "REST API",
                rest,
                "Cloud API URL copied",
            ))
            .child(endpoint(
                "copy-remote-mcp",
                "Remote MCP",
                mcp,
                "Remote MCP URL copied",
            ))
    }

    fn cloud_section(&mut self, colors: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let current = account::current(cx);
        if current.claims.is_none() {
            return div()
                .flex()
                .items_start()
                .justify_between()
                .gap_4()
                .child(Self::cloud_heading(None, colors))
                .child(div().mt_1().child(spinner(
                    "cloud-api-loading",
                    16.,
                    colors.muted_foreground,
                )))
                .into_any_element();
        }
        if !account::is_pro(cx) {
            let section = div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .items_start()
                        .justify_between()
                        .gap_4()
                        .child(Self::cloud_heading(None, colors))
                        .child(controls::switch(
                            "cloud-api-switch",
                            false,
                            true,
                            colors,
                            |_, _, _| {},
                        )),
                )
                .child(self.endpoints(colors, cx));
            return plan_gate(
                false,
                section,
                cx.listener(|_, _, _, cx| cx.emit(PlanRequired)),
            )
            .into_any_element();
        }
        let (enabled, error) = match &self.settings {
            Load::Ready(enabled) => (*enabled, None),
            Load::Failed(error) => (false, Some(error.clone())),
            Load::Pending => (false, None),
        };
        let interactive = matches!(self.settings, Load::Ready(_)) && !self.toggling;
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap_4()
                    .child(Self::cloud_heading(error, colors))
                    .child(controls::switch(
                        "cloud-api-switch",
                        enabled,
                        interactive,
                        colors,
                        cx.listener(move |this, _, _, cx| this.toggle_cloud(!enabled, cx)),
                    )),
            )
            .when(enabled, |view| {
                view.child(self.endpoints(colors, cx))
                    .child(self.keys_block(colors, cx))
            })
            .into_any_element()
    }

    fn created(
        &self,
        id: &'static str,
        notice: &'static str,
        value: String,
        copied: &'static str,
        colors: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .mt_3()
            .p_3()
            .border_1()
            .border_color(colors.border)
            .bg(colors.muted.opacity(0.3))
            .child(small(colors).child(notice))
            .child(
                div()
                    .mt_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(code(value.clone(), colors, cx))
                    .child(
                        button(id, Variant::Ghost, true, colors)
                            .child(controls::icon(
                                "product/CopyIcon.svg",
                                14.,
                                colors.foreground,
                            ))
                            .child("Copy")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.copy(value.clone(), copied, cx);
                                if copied == "Cloud API key copied" {
                                    this.created_key = None;
                                } else {
                                    this.created_secret = None;
                                }
                            })),
                    ),
            )
    }

    fn keys_block(&self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        div()
            .child(
                div()
                    .mb_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        svg()
                            .path(crate::workspace::catalog_icons::path("Key01Icon"))
                            .size(px(16.))
                            .flex_shrink_0()
                            .text_color(colors.muted_foreground),
                    )
                    .child(label("Cloud API keys")),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(input_box(self.key_name.clone(), 256., colors))
                    .child(
                        button("create-key", Variant::Outline, !self.creating_key, colors)
                            .child("Create key")
                            .when(!self.creating_key, |button| {
                                button.on_click(cx.listener(|this, _, _, cx| this.create_key(cx)))
                            }),
                    ),
            )
            .when_some(self.created_key.clone(), |view, key| {
                view.child(self.created(
                    "copy-created-key",
                    "Copy this key now — it is only shown once.",
                    key,
                    "Cloud API key copied",
                    colors,
                    cx,
                ))
            })
            .when(!self.keys.is_empty(), |view| {
                view.child(div().mt_3().flex().flex_col().gap(px(6.)).children(
                    self.keys.iter().map(|key| {
                        let id = key.id.clone();
                        let when = match &key.last_used_at {
                            Some(used) => format!("Last used {}", date(used)),
                            None => format!("Created {}", date(&key.created_at)),
                        };
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .text_sm()
                            .line_height(px(20.))
                            .child(
                                div()
                                    .flex()
                                    .min_w_0()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        code(format!("{}…", key.key_prefix), colors, cx)
                                            .flex_shrink_0(),
                                    )
                                    .child(div().min_w_0().truncate().child(key.name.clone())),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .gap_2()
                                    .child(small(colors).child(when))
                                    .child(
                                        button(
                                            SharedString::from(format!("revoke-{}", key.id)),
                                            Variant::Ghost,
                                            true,
                                            colors,
                                        )
                                        .text_color(colors.destructive)
                                        .child("Revoke")
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                this.revoke_key(id.clone(), cx)
                                            }),
                                        ),
                                    ),
                            )
                    }),
                ))
            })
    }

    fn webhooks_section(&self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(controls::section_title("Webhooks"))
            .child(
                div()
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(input_box(self.webhook_url.clone(), 448., colors))
                            .child(
                                button(
                                    "add-webhook",
                                    Variant::Outline,
                                    !self.creating_webhook,
                                    colors,
                                )
                                .child("Add webhook")
                                .when(
                                    !self.creating_webhook,
                                    |button| {
                                        button.on_click(
                                            cx.listener(|this, _, _, cx| this.create_webhook(cx)),
                                        )
                                    },
                                ),
                            ),
                    )
                    .when_some(self.created_secret.clone(), |view, secret| {
                        view.child(self.created(
                            "copy-signing-secret",
                            "Copy this signing secret now — it is only shown once.",
                            secret,
                            "Signing secret copied",
                            colors,
                            cx,
                        ))
                    })
                    .when(!self.webhooks.is_empty(), |view| {
                        view.child(
                            div().mt_3().flex().flex_col().gap(px(6.)).children(
                                self.webhooks
                                    .iter()
                                    .map(|webhook| self.webhook_row(webhook, colors, cx)),
                            ),
                        )
                    }),
            )
    }

    fn webhook_row(&self, webhook: &WebhookInfo, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let mut status = Vec::new();
        if !webhook.active {
            status.extend(["Paused".to_string(), "not receiving events".into()]);
        }
        status.push(if webhook.events.is_empty() {
            "All events".into()
        } else {
            webhook.events.join(", ")
        });
        if webhook.last_delivery_at.is_some() {
            status.push(format!("Last delivery {}", webhook.last_delivery_status));
        }
        let (id, active) = (webhook.id.clone(), webhook.active);
        let key = |name: &str| SharedString::from(format!("webhook-{name}-{}", webhook.id));
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .text_sm()
            .line_height(px(20.))
            .child(
                div()
                    .flex()
                    .min_w_0()
                    .flex_col()
                    .child(
                        div()
                            .truncate()
                            .when(!active, |view| {
                                view.text_color(colors.muted_foreground).line_through()
                            })
                            .child(webhook.url.clone()),
                    )
                    .child(small(colors).child(status.join(" · "))),
            )
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_1()
                    .child(
                        button(key("toggle"), Variant::Ghost, true, colors)
                            .child(if active { "Pause" } else { "Enable" })
                            .on_click(cx.listener({
                                let id = id.clone();
                                move |this, _, _, cx| {
                                    this.webhook_action(
                                        id.clone(),
                                        WebhookAction::SetActive(!active),
                                        cx,
                                    )
                                }
                            })),
                    )
                    .child(
                        button(key("test"), Variant::Ghost, !self.testing, colors)
                            .child("Test")
                            .when(!self.testing, |button| {
                                let id = id.clone();
                                button.on_click(cx.listener(move |this, _, _, cx| {
                                    this.webhook_action(id.clone(), WebhookAction::Test, cx)
                                }))
                            }),
                    )
                    .child(
                        button(key("delete"), Variant::Ghost, true, colors)
                            .child(controls::icon(
                                "sharing/TrashIcon.svg",
                                14.,
                                colors.destructive,
                            ))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.webhook_action(id.clone(), WebhookAction::Delete, cx)
                            })),
                    ),
            )
    }
}

/// The `YYYY-MM-DD` prefix shipping shows for key timestamps.
fn date(timestamp: &str) -> &str {
    timestamp.get(..10).unwrap_or(timestamp)
}

impl Render for DevelopersView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        div()
            .flex()
            .flex_col()
            .gap_8()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(controls::page_title("Developers"))
                    .child(
                        button("developers-guide", Variant::Outline, true, &colors)
                            .child("Guide")
                            .child(controls::icon(
                                "product/ExternalLinkIcon.svg",
                                14.,
                                colors.foreground,
                            ))
                            .on_click(|_, _, cx| cx.open_url(GUIDE_URL)),
                    ),
            )
            .child(self.cli_section(&colors, cx))
            .child(self.cloud_section(&colors, cx))
            .child(self.webhooks_section(&colors, cx))
    }
}
