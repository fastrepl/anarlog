//! The Developers settings page: `settings/developers/{index,cli,skills,
//! cloud-api,webhooks}.tsx`.

use gpui::{
    AnyElement, ClickEvent, Context, Div, Entity, Focusable as _, MouseButton, SharedString,
    Window, div, prelude::*, px,
};

use super::Workspace;
use super::toast::FlashVariant;
use crate::developers::{CliState, CliStatus, SkillAgent, SkillAgentStatus, Webhook};
use crate::text_input::{TextInput, TextInputEvent, TextInputStyle};
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

const DEVELOPERS_GUIDE_URL: &str = "https://docs.anarlog.so/agents/overview";

/// A row-button action.
type RowAction = Box<dyn Fn(&mut Workspace, &mut Context<Workspace>)>;
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) struct DevelopersState {
    cli: Option<Result<CliStatus, String>>,
    cli_installing: bool,
    skills: Vec<SkillAgentStatus>,
    skills_installing: bool,
    skills_menu_open: bool,
    webhooks: Vec<Webhook>,
    webhook_input: Entity<TextInput>,
    webhook_creating: bool,
    /// `createdWebhook.secret`: shown once until copied.
    created_secret: Option<String>,
    testing: Option<String>,
}

impl Workspace {
    /// Builds the page state and runs the CLI / skills / webhooks queries.
    pub(crate) fn ensure_developers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.developers.is_some() {
            self.reload_developers(cx);
            return;
        }
        let style = TextInputStyle {
            text: self.theme.foreground,
            placeholder: self.theme.muted_foreground,
            selection: self.theme.selection,
            underline_when_focused: false,
            masked: false,
        };
        let input =
            cx.new(|cx| TextInput::new("https://example.com/webhooks/anarlog", style, window, cx));
        cx.subscribe_in(&input, window, |this, _, event: &TextInputEvent, _, cx| {
            if *event == TextInputEvent::Enter {
                this.add_webhook(cx);
            }
        })
        .detach();
        self.developers = Some(DevelopersState {
            cli: None,
            cli_installing: false,
            skills: Vec::new(),
            skills_installing: false,
            skills_menu_open: false,
            webhooks: Vec::new(),
            webhook_input: input,
            webhook_creating: false,
            created_secret: None,
            testing: None,
        });
        self.reload_developers(cx);
    }

    fn reload_developers(&mut self, cx: &mut Context<Self>) {
        let identifier = self.store.identifier().to_string();
        let cli = self.store.runtime().spawn_blocking(move || {
            (
                crate::developers::check_cli(&identifier, APP_VERSION),
                crate::developers::list_skill_agents(),
            )
        });
        let webhooks = self.store.list_webhooks();
        cx.spawn(async move |this, cx| {
            let probes = cli.await.ok();
            let webhooks = webhooks.await.ok().and_then(Result::ok);
            this.update(cx, |this, cx| {
                if let Some(state) = this.developers.as_mut() {
                    if let Some((status, skills)) = probes {
                        state.cli = Some(Ok(status));
                        state.skills = skills.unwrap_or_default();
                    }
                    if let Some(webhooks) = webhooks {
                        state.webhooks = webhooks;
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn install_cli(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.developers.as_mut() else {
            return;
        };
        state.cli_installing = true;
        cx.notify();
        let identifier = self.store.identifier().to_string();
        let task = self
            .store
            .runtime()
            .spawn_blocking(move || crate::developers::install_cli(&identifier, APP_VERSION));
        cx.spawn(async move |this, cx| {
            let result = task
                .await
                .map_err(|error| error.to_string())
                .and_then(|r| r);
            this.update(cx, |this, cx| {
                match result {
                    Ok(status) => {
                        // `getCliInstallNotification`
                        if status.state == CliState::Installed {
                            this.flash(
                                FlashVariant::Success,
                                format!("{} is ready to use", status.command_name),
                                cx,
                            );
                        } else {
                            this.flash(
                                FlashVariant::Error,
                                status.details.clone().unwrap_or_else(|| {
                                    format!("{} could not be installed", status.command_name)
                                }),
                                cx,
                            );
                        }
                        if let Some(state) = this.developers.as_mut() {
                            state.cli = Some(Ok(status));
                        }
                    }
                    Err(error) => this.flash(FlashVariant::Error, error, cx),
                }
                if let Some(state) = this.developers.as_mut() {
                    state.cli_installing = false;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn install_skills(&mut self, agents: Vec<SkillAgent>, cx: &mut Context<Self>) {
        let Some(state) = self.developers.as_mut() else {
            return;
        };
        state.skills_installing = true;
        state.skills_menu_open = false;
        cx.notify();
        let task = self.store.runtime().spawn_blocking(move || {
            let mut statuses = Vec::new();
            for agent in agents {
                statuses.push(crate::developers::install_skill(agent)?);
            }
            Ok::<_, String>(statuses)
        });
        cx.spawn(async move |this, cx| {
            let result = task
                .await
                .map_err(|error| error.to_string())
                .and_then(|r| r);
            this.update(cx, |this, cx| {
                match result {
                    Ok(statuses) => {
                        let message = if statuses.len() == 1 {
                            format!(
                                "Anarlog skill added to {}",
                                statuses[0].agent.display_name()
                            )
                        } else {
                            format!("Anarlog skill added to {} agents", statuses.len())
                        };
                        this.flash(FlashVariant::Success, message, cx);
                    }
                    Err(error) => this.flash(FlashVariant::Error, error, cx),
                }
                if let Some(state) = this.developers.as_mut() {
                    state.skills_installing = false;
                }
                this.reload_developers(cx);
            })
            .ok();
        })
        .detach();
    }

    fn add_webhook(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.developers.as_mut() else {
            return;
        };
        let url = state.webhook_input.read(cx).text().trim().to_string();
        if url.is_empty() || state.webhook_creating {
            return;
        }
        state.webhook_creating = true;
        cx.notify();
        let task = self.store.create_webhook(url);
        cx.spawn(async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update(cx, |this, cx| {
                match result {
                    Ok((_, secret)) => {
                        if let Some(state) = this.developers.as_mut() {
                            state.created_secret = Some(secret);
                            state
                                .webhook_input
                                .update(cx, |input, cx| input.set_text("", cx));
                        }
                        this.flash(FlashVariant::Success, "Webhook added", cx);
                    }
                    Err(error) => this.flash(FlashVariant::Error, error.to_string(), cx),
                }
                if let Some(state) = this.developers.as_mut() {
                    state.webhook_creating = false;
                }
                this.reload_developers(cx);
            })
            .ok();
        })
        .detach();
    }

    fn toggle_webhook(&mut self, id: String, active: bool, cx: &mut Context<Self>) {
        let task = self.store.set_webhook_active(id, active);
        cx.spawn(async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update(cx, |this, cx| {
                if let Err(error) = result {
                    this.flash(FlashVariant::Error, error.to_string(), cx);
                }
                this.reload_developers(cx);
            })
            .ok();
        })
        .detach();
    }

    fn delete_webhook(&mut self, id: String, cx: &mut Context<Self>) {
        let task = self.store.delete_webhook(id);
        cx.spawn(async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update(cx, |this, cx| {
                match result {
                    Ok(_) => this.flash(FlashVariant::Success, "Webhook removed", cx),
                    Err(error) => this.flash(FlashVariant::Error, error.to_string(), cx),
                }
                this.reload_developers(cx);
            })
            .ok();
        })
        .detach();
    }

    fn test_webhook(&mut self, id: String, cx: &mut Context<Self>) {
        if let Some(state) = self.developers.as_mut() {
            state.testing = Some(id.clone());
        }
        cx.notify();
        let task = self.store.test_webhook(id);
        cx.spawn(async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update(cx, |this, cx| {
                match result {
                    Ok((true, _)) => this.flash(FlashVariant::Success, "Test delivery sent", cx),
                    Ok((false, status)) => this.flash(
                        FlashVariant::Error,
                        format!("Test delivery failed: {status}"),
                        cx,
                    ),
                    Err(error) => this.flash(FlashVariant::Error, error.to_string(), cx),
                }
                if let Some(state) = this.developers.as_mut() {
                    state.testing = None;
                }
                this.reload_developers(cx);
            })
            .ok();
        })
        .detach();
    }

    fn copy_text(&mut self, text: String, success: &'static str, cx: &mut Context<Self>) {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        self.flash(FlashVariant::Success, success, cx);
    }

    /// `SettingsDevelopers`: the title row with the Guide button, then the
    /// CLI & MCP, Cloud API & Connectors, and Webhooks sections (`gap-8`).
    pub(super) fn render_developers_settings(&self, title: Div, cx: &Context<Self>) -> Div {
        let theme = self.theme;
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
                    .child(title)
                    .child(
                        self.outline_button(
                            "developers-guide",
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child("Guide")
                                .child(icon("external-link", px(14.0), theme.foreground))
                                .into_any_element(),
                            false,
                            Some(Box::new(|_, _, cx| cx.open_url(DEVELOPERS_GUIDE_URL))),
                            cx,
                        ),
                    ),
            )
            .child(self.render_cli_section(cx))
            .child(self.render_cloud_api_section())
            .child(self.render_webhooks_section(cx))
    }

    /// `Button size="sm" variant="outline"` (or `default` when `primary`):
    /// `h-8 px-3 gap-1.5 text-xs font-medium` under the control squircle.
    #[allow(clippy::type_complexity)]
    pub(super) fn outline_button(
        &self,
        id: &'static str,
        content: AnyElement,
        disabled: bool,
        on_click: Option<Box<dyn Fn(&mut Workspace, &mut Window, &mut Context<Workspace>)>>,
        cx: &Context<Self>,
    ) -> gpui::Stateful<Div> {
        let theme = self.theme;
        let hovered = self.hovered == Some(id);
        let on_click = on_click.map(std::rc::Rc::new);
        div()
            .id(id)
            .relative()
            .flex()
            .h(px(32.0))
            .flex_shrink_0()
            .items_center()
            .gap(px(6.0))
            .px_3()
            .child(crate::squircle::squircle(
                crate::squircle::CONTROL_RADIUS,
                Some(if hovered && !disabled {
                    theme.accent
                } else {
                    theme.background
                }),
                Some((1.0, theme.border)),
            ))
            .tw_text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(theme.foreground)
            .when(disabled, |button| button.opacity(0.5).cursor_not_allowed())
            .when(!disabled, |button| {
                button
                    .cursor_pointer()
                    .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                        this.set_hovered(id, *hovering, cx);
                    }))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .when_some(on_click, |button, on_click| {
                        button.on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            on_click(this, window, cx);
                        }))
                    })
            })
            .child(content)
    }

    /// `Button size="sm"` (default variant): `bg-primary` squircle.
    fn primary_button(
        &self,
        id: &'static str,
        label: &'static str,
        disabled: bool,
        cx: &Context<Self>,
    ) -> gpui::Stateful<Div> {
        let theme = self.theme;
        let hovered = self.hovered == Some(id);
        div()
            .id(id)
            .relative()
            .flex()
            .h(px(32.0))
            .flex_shrink_0()
            .items_center()
            .px_3()
            .child(crate::squircle::squircle(
                crate::squircle::CONTROL_RADIUS,
                Some(if hovered && !disabled {
                    alpha(theme.primary, 0.9)
                } else {
                    theme.primary
                }),
                None,
            ))
            .tw_text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(theme.primary_foreground)
            .when(disabled, |button| button.opacity(0.5).cursor_not_allowed())
            .when(!disabled, |button| {
                button
                    .cursor_pointer()
                    .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                        this.set_hovered(id, *hovering, cx);
                    }))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.install_cli(cx)))
            })
            .child(label)
    }

    fn section_heading(&self, text: &'static str) -> Div {
        div()
            .tw_text_lg()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(self.theme.foreground)
            .child(text)
    }

    fn row_title(&self, text: &'static str) -> Div {
        div()
            .tw_text_sm()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(self.theme.foreground)
            .child(text)
    }

    /// `CliSection`: Anarlog CLI (status + Install), MCP server (Copy config),
    /// Agent skills (Add skill to…).
    fn render_cli_section(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let state = self.developers.as_ref();
        let status = state.and_then(|state| state.cli.as_ref());
        let installing = state.is_some_and(|state| state.cli_installing);
        let is_installed =
            matches!(status, Some(Ok(status)) if status.state == CliState::Installed);
        let can_install = matches!(
            status,
            Some(Ok(status))
                if status.supported
                    && status.state != CliState::ResourceMissing
                    && status.state != CliState::Conflict
        );

        // `CliStatus`
        let status_line: Option<AnyElement> = match status {
            None => Some(
                div()
                    .mt_1()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .tw_text_xs()
                    .text_color(theme.muted_foreground)
                    .child(icon("arrows-clockwise", px(12.0), theme.muted_foreground))
                    .child("Checking…")
                    .into_any_element(),
            ),
            Some(Err(error)) => Some(
                div()
                    .mt_1()
                    .tw_text_xs()
                    .text_color(theme.destructive)
                    .child(SharedString::from(format!(
                        "Could not check the CLI: {error}"
                    )))
                    .into_any_element(),
            ),
            Some(Ok(status)) if status.state == CliState::Installed => None,
            Some(Ok(status)) => {
                let show_details = matches!(
                    status.state,
                    CliState::Conflict | CliState::Unsupported | CliState::ResourceMissing
                );
                let text = if show_details {
                    status
                        .details
                        .clone()
                        .unwrap_or_else(|| "Unavailable".to_string())
                } else {
                    "Not installed".to_string()
                };
                Some(
                    div()
                        .mt_1()
                        .flex()
                        .items_start()
                        .gap(px(6.0))
                        .tw_text_xs()
                        .child(
                            div()
                                .mt_1()
                                .size(px(8.0))
                                .flex_shrink_0()
                                .rounded_full()
                                .bg(if status.state == CliState::Conflict {
                                    gpui::rgb(0xf59e0b)
                                } else {
                                    alpha(theme.muted_foreground, 0.5)
                                }),
                        )
                        .child(
                            div()
                                .text_color(theme.muted_foreground)
                                .child(SharedString::from(text)),
                        )
                        .into_any_element(),
                )
            }
        };

        let command = match status {
            Some(Ok(status)) if status.state == CliState::Installed => status.install_path.clone(),
            Some(Ok(status)) => status.command_name.to_string(),
            _ => "anarlog".to_string(),
        };
        let configuration = crate::developers::mcp_configuration(&command);

        let skills = state.map(|state| state.skills.clone()).unwrap_or_default();
        let skills_installing = state.is_some_and(|state| state.skills_installing);
        let skills_open = state.is_some_and(|state| state.skills_menu_open);

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(self.section_heading("CLI & MCP"))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.0))
                                            .child(self.row_title("Anarlog CLI"))
                                            .when(is_installed, |row| {
                                                row.child(icon("check-circle", px(14.0), gpui::rgb(0x009966)))
                                            }),
                                    )
                                    .children(status_line),
                            )
                            .child(self.primary_button(
                                "cli-install",
                                if installing {
                                    "Installing…"
                                } else if is_installed {
                                    "Reinstall"
                                } else {
                                    "Install"
                                },
                                !can_install || installing,
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .child(div().min_w_0().flex_1().child(self.row_title("MCP server")))
                            .child(self.outline_button(
                                "mcp-copy-config",
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .child(icon("copy", px(14.0), theme.foreground))
                                    .child("Copy config")
                                    .into_any_element(),
                                !is_installed,
                                Some(Box::new(move |this, _, cx| {
                                    this.copy_text(configuration.clone(), "MCP configuration copied", cx);
                                })),
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .child(self.row_title("Agent skills"))
                                    .child(
                                        div()
                                            .mt_1()
                                            .tw_text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child("Teach coding agents when and how to use the Anarlog CLI and MCP"),
                                    ),
                            )
                            .child(
                                div()
                                    .relative()
                                    .flex_shrink_0()
                                    .child(self.outline_button(
                                        "skills-add",
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(6.0))
                                            .child(if skills_installing { "Installing…" } else { "Add skill to…" })
                                            .when(!skills_installing, |row| {
                                                row.child(icon("caret-down", px(14.0), theme.foreground))
                                            })
                                            .into_any_element(),
                                        skills.is_empty() || skills_installing,
                                        Some(Box::new(|this, _, cx| {
                                            if let Some(state) = this.developers.as_mut() {
                                                state.skills_menu_open = !state.skills_menu_open;
                                                cx.notify();
                                            }
                                        })),
                                        cx,
                                    ))
                                    .when(skills_open, |anchor| {
                                        anchor.child(self.render_skills_menu(&skills, cx))
                                    }),
                            ),
                    ),
            )
    }

    /// The `variant="app" align="end" w-52` skills dropdown: Install to all
    /// agents, a separator, one row per agent with its check when installed.
    fn render_skills_menu(&self, skills: &[SkillAgentStatus], cx: &Context<Self>) -> AnyElement {
        use super::menu::{Align, Entry, MenuSpec, Trailing};
        let detected: Vec<SkillAgent> = skills
            .iter()
            .filter(|status| status.detected)
            .map(|status| status.agent)
            .collect();
        let mut entries = vec![
            Entry::Item {
                icon: None,
                dim_icon: false,
                label: "Install to all agents".into(),
                trailing: Trailing::None,
                destructive: false,
                on_select: (!detected.is_empty()).then(|| {
                    let detected = detected.clone();
                    Box::new(
                        move |this: &mut Workspace, _: &mut Window, cx: &mut Context<Workspace>| {
                            this.install_skills(detected.clone(), cx)
                        },
                    ) as super::menu::Select
                }),
                submenu: None,
            },
            Entry::Separator,
        ];
        for status in skills {
            let agent = status.agent;
            entries.push(Entry::Item {
                icon: None,
                dim_icon: false,
                label: agent.display_name().into(),
                trailing: Trailing::Check(status.installed),
                destructive: false,
                on_select: status.detected.then(|| {
                    Box::new(
                        move |this: &mut Workspace, _: &mut Window, cx: &mut Context<Workspace>| {
                            this.install_skills(vec![agent], cx)
                        },
                    ) as super::menu::Select
                }),
                submenu: None,
            });
        }
        let spec = MenuSpec {
            id: "skills-menu",
            width: 208.0,
            entries,
            open_sub: None,
            on_hover_sub: |_, _, _| {},
            on_close: |this, cx| {
                if let Some(state) = this.developers.as_mut() {
                    state.skills_menu_open = false;
                    cx.notify();
                }
            },
        };
        // Anchored under the button's right edge (`align="end"`, 4px offset).
        div()
            .absolute()
            .top(px(36.0))
            .right_0()
            .child(self.render_menu_inline(spec, Align::End, cx))
            .into_any_element()
    }

    /// `CloudApiSection` while billing is unresolved: heading + spinner.
    fn render_cloud_api_section(&self) -> Div {
        let theme = self.theme;
        div()
            .flex()
            .items_start()
            .justify_between()
            .gap_4()
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .child(self.section_heading("Cloud API & Connectors"))
                    .child(
                        div()
                            .mt_1()
                            .tw_text_xs()
                            .text_color(theme.muted_foreground)
                            .child("Uploads meeting content for remote access while Anarlog is closed."),
                    ),
            )
            .child(
                div()
                    .mt_1()
                    .flex_shrink_0()
                    .child(icon("arrows-clockwise", px(16.0), theme.muted_foreground)),
            )
    }

    /// `WebhooksSection`: heading, the URL form, the one-time secret card,
    /// and the endpoint rows.
    fn render_webhooks_section(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let Some(state) = self.developers.as_ref() else {
            return div();
        };
        let input = state.webhook_input.clone();
        let creating = state.webhook_creating;
        let mut body = div().child(
            div()
                .flex()
                .gap_2()
                .child(
                    // `Input className="h-8 max-w-md text-sm"`
                    div()
                        .id("webhook-url")
                        .flex()
                        .h(px(32.0))
                        .w_full()
                        .max_w(px(448.0))
                        .items_center()
                        .rounded_md()
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.background)
                        .px_3()
                        .tw_text_sm()
                        .cursor_text()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                            input.read(cx).focus_handle(cx).focus(window);
                        }))
                        .child(div().min_w_0().flex_1().child(state.webhook_input.clone())),
                )
                .child(self.outline_button(
                    "webhook-add",
                    div().child("Add webhook").into_any_element(),
                    creating,
                    Some(Box::new(|this, _, cx| this.add_webhook(cx))),
                    cx,
                )),
        );
        if let Some(secret) = state.created_secret.clone() {
            let secret_for_copy = secret.clone();
            body = body.child(
                div()
                    .mt_3()
                    .rounded_xl()
                    .border_1()
                    .border_color(theme.border)
                    .bg(alpha(theme.muted, 0.3))
                    .p_3()
                    .child(
                        div()
                            .tw_text_xs()
                            .text_color(theme.muted_foreground)
                            .child("Copy this signing secret now — it is only shown once."),
                    )
                    .child(
                        div()
                            .mt_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .rounded_md()
                                    .bg(theme.muted)
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .tw_text_xs()
                                    .font_family(
                                        crate::theme::mono_font_family(cx.text_system())
                                            .unwrap_or_default(),
                                    )
                                    .text_color(theme.foreground)
                                    .child(SharedString::from(secret)),
                            )
                            .child(
                                div()
                                    .id("webhook-secret-copy")
                                    .flex()
                                    .h(px(28.0))
                                    .flex_shrink_0()
                                    .items_center()
                                    .gap(px(6.0))
                                    .px_2()
                                    .tw_text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(theme.foreground)
                                    .cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.copy_text(
                                            secret_for_copy.clone(),
                                            "Signing secret copied",
                                            cx,
                                        );
                                        if let Some(state) = this.developers.as_mut() {
                                            state.created_secret = None;
                                        }
                                    }))
                                    .child(icon("copy", px(14.0), theme.foreground))
                                    .child("Copy"),
                            ),
                    ),
            );
        }
        if !state.webhooks.is_empty() {
            body = body.child(
                div().mt_3().flex().flex_col().gap(px(6.0)).children(
                    state
                        .webhooks
                        .iter()
                        .map(|webhook| self.render_webhook_row(webhook, state, cx)),
                ),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(self.section_heading("Webhooks"))
            .child(body)
    }

    /// `WebhookRow`: url + status line, Pause/Enable · Test · Delete ghost buttons.
    fn render_webhook_row(
        &self,
        webhook: &Webhook,
        state: &DevelopersState,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let mut parts: Vec<String> = Vec::new();
        if !webhook.active {
            parts.push("not receiving events".to_string());
        }
        parts.push(if webhook.events.is_empty() {
            "All events".to_string()
        } else {
            webhook.events.join(", ")
        });
        if webhook.last_delivery_at.is_some() {
            parts.push(format!("Last delivery {}", webhook.last_delivery_status));
        }
        let testing = state.testing.as_deref() == Some(webhook.id.as_str());
        let ghost = |id: SharedString,
                     label: &'static str,
                     destructive: bool,
                     disabled: bool,
                     on_click: RowAction| {
            let on_click = std::rc::Rc::new(on_click);
            div()
                .id(id)
                .flex()
                .h(px(28.0))
                .items_center()
                .px_2()
                .rounded(px(8.0))
                .tw_text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(if destructive {
                    theme.destructive
                } else {
                    theme.foreground
                })
                .when(disabled, |button| button.opacity(0.5))
                .when(!disabled, |button| {
                    button
                        .cursor_pointer()
                        .hover(move |style| style.bg(theme.accent))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(
                            cx.listener(move |this, _: &ClickEvent, _, cx| on_click(this, cx)),
                        )
                })
                .child(label)
        };
        let id = webhook.id.clone();
        let (toggle_id, test_id, delete_id) = (id.clone(), id.clone(), id.clone());
        let active = webhook.active;
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .tw_text_sm()
            .child(
                div()
                    .flex()
                    .min_w_0()
                    .flex_col()
                    .child(
                        div()
                            .truncate()
                            .text_color(if active {
                                theme.foreground
                            } else {
                                theme.muted_foreground
                            })
                            .when(!active, |url| url.line_through())
                            .child(SharedString::from(webhook.url.clone())),
                    )
                    .child(
                        div()
                            .tw_text_xs()
                            .text_color(theme.muted_foreground)
                            .child(SharedString::from(parts.join(" · "))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_1()
                    .child(ghost(
                        format!("webhook-toggle-{id}").into(),
                        if active { "Pause" } else { "Enable" },
                        false,
                        false,
                        Box::new(move |this, cx| {
                            this.toggle_webhook(toggle_id.clone(), !active, cx)
                        }),
                    ))
                    .child(ghost(
                        format!("webhook-test-{id}").into(),
                        "Test",
                        false,
                        testing,
                        Box::new(move |this, cx| this.test_webhook(test_id.clone(), cx)),
                    ))
                    .child(ghost(
                        format!("webhook-delete-{id}").into(),
                        "Delete",
                        true,
                        false,
                        Box::new(move |this, cx| this.delete_webhook(delete_id.clone(), cx)),
                    )),
            )
    }
}
