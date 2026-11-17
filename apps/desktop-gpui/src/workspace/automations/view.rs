//! The automations route: `AutomationsNav` in the left panel and `AutomationsContent`
//! (the overview, a starter's details, or a workflow's builder) in the main surface.

use std::{collections::HashMap, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use desktop_runtime::{CancellationToken, Reply, RuntimeHandle};
use gpui::{
    AnyElement, Context, Div, ElementId, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    Hsla, KeyDownEvent, MouseButton, Pixels, Point, Render, SharedString, Stateful, Subscription,
    Transformation, Window, anchored, deferred, div, img, percentage, prelude::*, px, rgb, svg,
};

use super::{
    MarkdownOptions, STARTERS, Settings, StarterCommand, StarterId, Step, StepKind, StepType,
    Target, Trigger, Workflow, assets,
};
use crate::{
    contracts::ProductRoute,
    product::{
        account,
        cloud::resources::ResourceKind,
        sharing::{
            resource_dialog::{ResourceDialogEvent, ResourceShareDialog},
            shared_library::SharedLibrary,
        },
    },
    ui::{
        input::{InputEvent, TextInput},
        theme::{Theme, is_dark, theme},
    },
    workspace::{
        automation_runner::{AutomationClient, Choice, Connection, TargetList},
        catalog_icons,
        navigation::Route,
        resource_list as list,
    },
};

const PRO_REQUIRED: &str = "This requires Anarlog Pro";
const STARTER_MENU: &str = "starter";

pub enum AutomationsEvent {
    Back,
    /// Sign in, upgrade, or manage integrations.
    Product(ProductRoute),
    Navigate(Route),
}

impl EventEmitter<AutomationsEvent> for AutomationsView {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Selection {
    Starter(StarterId),
    Chat(Arc<str>),
    Workflow(Arc<str>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Menu {
    Actions,
    Trigger,
    AddStep,
    StepType(Arc<str>),
    DriveFormat(Arc<str>),
    Targets(Arc<str>),
}

enum Load<T> {
    Loading,
    Ready(T),
    Failed,
}

#[derive(Clone, Copy, PartialEq)]
enum Tone {
    Success,
    Error,
}

/// A success or error toast of the app toaster, with its auto-dismiss countdown.
struct Toast {
    tone: Tone,
    message: SharedString,
    remaining_ms: u64,
    total_ms: u64,
    paused: bool,
}

enum Notice {
    Plan(list::WarningToast),
    Toast(Toast),
}

pub struct AutomationsView {
    runtime: RuntimeHandle,
    settings: Arc<Settings>,
    chats: Vec<super::ChatAutomation>,
    selection: Option<Selection>,
    search: Entity<TextInput>,
    menu: Option<Menu>,
    context: Option<(Point<Pixels>, Selection)>,
    show_preview: bool,
    pending: bool,
    client: Option<AutomationClient>,
    signed_in: bool,
    connections: Option<Load<Vec<Connection>>>,
    /// An integration page is being opened; shipping disables the connect buttons meanwhile.
    opening: bool,
    targets: HashMap<TargetList, Load<Vec<Choice>>>,
    notion_queries: HashMap<Arc<str>, (Entity<TextInput>, Subscription)>,
    filenames: HashMap<Arc<str>, (Entity<TextInput>, Subscription)>,
    sharing: Option<(Entity<ResourceShareDialog>, Subscription)>,
    shared_library: Entity<SharedLibrary>,
    notice: Option<(u64, Notice)>,
    notices: u64,
    cancel: CancellationToken,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl AutomationsView {
    pub fn new(runtime: RuntimeHandle, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextInput::new("Search automations...", cx).inline());
        let subscription = cx.subscribe(&search, |this, _, event, cx| {
            if matches!(event, InputEvent::Changed) {
                this.context = None;
                cx.notify();
            }
        });
        let shared_library = cx.new(|cx| {
            SharedLibrary::new(
                runtime.clone(),
                ResourceKind::Automation,
                search.clone(),
                cx,
            )
        });
        Self {
            runtime,
            settings: Arc::default(),
            chats: Vec::new(),
            selection: None,
            search,
            menu: None,
            context: None,
            show_preview: false,
            pending: false,
            client: None,
            signed_in: false,
            connections: None,
            opening: false,
            targets: HashMap::new(),
            notion_queries: HashMap::new(),
            filenames: HashMap::new(),
            sharing: None,
            shared_library,
            notice: None,
            notices: 0,
            cancel: CancellationToken::new(),
            focus: cx.focus_handle(),
            _subscriptions: vec![subscription],
        }
    }

    /// Shipping's `ResourceShareButton` dialog for the selected workflow.
    fn share(&mut self, id: Arc<str>, title: Arc<str>, cx: &mut Context<Self>) {
        let runtime = self.runtime.clone();
        let dialog =
            cx.new(|cx| ResourceShareDialog::new(runtime, ResourceKind::Automation, id, title, cx));
        let subscription = cx.subscribe(&dialog, |this, _, event, cx| {
            this.sharing = None;
            if let ResourceDialogEvent::Navigate(route) = event {
                cx.emit(AutomationsEvent::Navigate(route.clone()));
            }
            cx.notify();
        });
        self.sharing = Some((dialog, subscription));
        cx.notify();
    }

    pub fn set_client(&mut self, client: Option<AutomationClient>, cx: &mut Context<Self>) {
        self.signed_in = client.is_some();
        self.client = client;
        self.connections = None;
        self.targets.clear();
        cx.notify();
    }

    pub fn activate(&mut self, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
        list::watch(
            &self.runtime,
            super::SETTINGS_SQL,
            self.cancel.clone(),
            cx,
            |this, rows, cx| {
                match rows {
                    Ok(rows) => this.settings = Arc::new(Settings::decode(&rows)),
                    Err(error) => this.toast(Tone::Error, error, cx),
                }
                cx.notify();
            },
        );
        list::watch(
            &self.runtime,
            super::CHATS_SQL,
            self.cancel.clone(),
            cx,
            |this, rows, cx| {
                if let Ok(rows) = rows {
                    this.chats = super::decode_chats(&rows);
                }
                cx.notify();
            },
        );
    }

    pub fn suspend(&mut self, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.menu = None;
        self.context = None;
        cx.notify();
    }

    /// `useEffectiveAutomationSelection`: the stored starter draft when nothing is selected.
    fn current(&self) -> Option<Selection> {
        self.selection
            .clone()
            .or_else(|| self.settings.draft_template().map(Selection::Starter))
    }

    fn select(&mut self, selection: Selection, cx: &mut Context<Self>) {
        if self.current().as_ref() != Some(&selection) {
            self.show_preview = false;
        }
        self.selection = Some(selection.clone());
        self.menu = None;
        if let Selection::Chat(group) = &selection {
            // `useEnsuredWorkflow`: a chat automation gets its workflow on first view.
            let title = self
                .chats
                .iter()
                .find(|chat| &chat.id == group)
                .map(|chat| chat.title.trim().to_owned())
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| "Untitled automation".into());
            if !self
                .settings
                .workflows
                .iter()
                .any(|workflow| workflow.chat_group_id.as_ref() == Some(group))
            {
                let reply = super::ensure(
                    &self.runtime,
                    Workflow::empty(None, Some(group.clone()), &title),
                );
                self.write(reply, |_, _, _| {}, cx);
            }
        }
        cx.notify();
    }

    /// Clears the selection when it points at `removed` (`clearSelection`).
    fn clear(&mut self, removed: &Selection) {
        if self.current().as_ref() == Some(removed) {
            self.selection = None;
            self.show_preview = false;
        }
    }

    fn billing_ready(cx: &gpui::App) -> bool {
        let account = account::current(cx);
        account.signed_in() && account.claims.is_some()
    }

    /// Runs a write; errors become an error toast.
    fn write<T: 'static>(
        &mut self,
        reply: desktop_runtime::Result<Reply<T>>,
        then: impl FnOnce(&mut Self, T, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.menu = None;
        self.context = None;
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(value) => then(this, value, cx),
                    Err(_) => this.toast(Tone::Error, "Could not update the automation", cx),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Opens `menu`, or closes it when it is already open. Presses elsewhere close menus
    /// in the root's mouse-down handler, which this press must not reach.
    fn toggle(&mut self, menu: Menu, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.menu = (self.menu.as_ref() != Some(&menu)).then_some(menu);
        cx.notify();
    }

    fn create(&mut self, cx: &mut Context<Self>) {
        let workflow = Workflow::empty(None, None, "Untitled automation");
        let id = workflow.id.clone();
        let reply = super::save(&self.runtime, workflow);
        self.write(
            reply,
            move |this, _, cx| this.select(Selection::Workflow(id), cx),
            cx,
        );
    }

    /// `useSaveWorkflow`: the change shows at once and persists in the background.
    fn persist(&mut self, workflow: Workflow, cx: &mut Context<Self>) {
        let settings = Arc::make_mut(&mut self.settings);
        match settings
            .workflows
            .iter_mut()
            .find(|item| item.id == workflow.id)
        {
            Some(item) => *item = workflow.clone(),
            None => settings.workflows.insert(0, workflow.clone()),
        }
        self.pending = true;
        let reply = super::save(&self.runtime, workflow);
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| {
                this.pending = false;
                if result.is_err() {
                    this.toast(Tone::Error, "Could not update the automation", cx);
                }
                cx.notify();
            });
        })
        .detach();
        self.menu = None;
        cx.notify();
    }

    fn edit_workflow(
        &mut self,
        id: &Arc<str>,
        change: impl FnOnce(&mut Workflow),
        cx: &mut Context<Self>,
    ) {
        let Some(mut workflow) = self.workflow_for(id) else {
            return;
        };
        change(&mut workflow);
        self.persist(workflow, cx);
    }

    fn edit_step(
        &mut self,
        workflow: &Arc<str>,
        step: &Arc<str>,
        change: impl FnOnce(&mut Step),
        cx: &mut Context<Self>,
    ) {
        let step = step.clone();
        self.edit_workflow(
            workflow,
            move |workflow| {
                if let Some(item) = workflow.steps.iter_mut().find(|item| item.id() == &step) {
                    change(item);
                }
            },
            cx,
        );
    }

    /// The workflow a workflow or chat selection shows (`useEnsuredWorkflow`'s fallback until
    /// the stored one arrives).
    fn workflow_for(&self, id: &Arc<str>) -> Option<Workflow> {
        self.settings.workflow(id).cloned()
    }

    fn selected_workflow(&self, selection: &Selection) -> Option<Workflow> {
        match selection {
            Selection::Workflow(id) => {
                Some(self.settings.workflow(id).cloned().unwrap_or_else(|| {
                    Workflow::empty(Some(id.clone()), None, "Untitled automation")
                }))
            }
            Selection::Chat(group) => Some(
                self.settings
                    .workflows
                    .iter()
                    .find(|workflow| workflow.chat_group_id.as_ref() == Some(group))
                    .cloned()
                    .unwrap_or_else(|| {
                        Workflow::empty(None, Some(group.clone()), "Untitled automation")
                    }),
            ),
            Selection::Starter(_) => None,
        }
    }

    fn set_enabled(&mut self, workflow: Workflow, enabled: bool, cx: &mut Context<Self>) {
        if enabled && !account::is_pro(cx) {
            self.notify_pro(cx);
            return;
        }
        let mut workflow = workflow;
        workflow.enabled = enabled;
        self.persist(workflow, cx);
    }

    fn delete(&mut self, selection: Selection, cx: &mut Context<Self>) {
        let reply = match &selection {
            Selection::Workflow(id) => super::delete(&self.runtime, Some(id.clone()), None),
            Selection::Chat(group) => super::delete(&self.runtime, None, Some(group.clone())),
            Selection::Starter(id) => {
                super::starter_command(&self.runtime, *id, StarterCommand::Remove)
            }
        };
        let message = match selection {
            Selection::Starter(_) => "Automation removed",
            _ => "Automation deleted",
        };
        self.write(
            reply,
            move |this, _, cx| {
                this.clear(&selection);
                this.toast(Tone::Success, message, cx);
            },
            cx,
        );
    }

    fn starter(&mut self, id: StarterId, command: StarterCommand, cx: &mut Context<Self>) {
        let success = match &command {
            StarterCommand::SaveDraft => Some("Automation draft saved"),
            StarterCommand::SetEnabled(true) => Some("Automation enabled"),
            StarterCommand::SetEnabled(false) => Some("Automation disabled"),
            _ => None,
        };
        let failure = match &command {
            StarterCommand::SaveDraft => "Could not save the automation draft",
            StarterCommand::SetTarget(_) => "Could not save the automation setting",
            _ => "Could not update the automation",
        };
        self.pending = true;
        self.menu = None;
        let reply = super::starter_command(&self.runtime, id, command);
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| {
                this.pending = false;
                match (result, success) {
                    (Ok(()), Some(message)) => this.toast(Tone::Success, message, cx),
                    (Ok(()), None) => {}
                    (Err(_), _) => this.toast(Tone::Error, failure, cx),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn choose_folder(&mut self, target: FolderTarget, cx: &mut Context<Self>) {
        let picker = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose export folder".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picker.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let path = path.to_string_lossy().into_owned();
            let _ = this.update(cx, |this, cx| match target {
                FolderTarget::Starter => this.starter(
                    StarterId::MarkdownExport,
                    StarterCommand::SetTarget(path),
                    cx,
                ),
                FolderTarget::Step(workflow, step) => this.edit_step(
                    &workflow,
                    &step,
                    |step| {
                        if let Step::Markdown { directory, .. } = step {
                            *directory = path;
                        }
                    },
                    cx,
                ),
            });
        })
        .detach();
    }

    /// Shipping's `openIntegration`: the integration page opens with the browser hand-off.
    fn connect(
        &mut self,
        integration: &'static str,
        connection: Option<Connection>,
        cx: &mut Context<Self>,
    ) {
        if self.opening {
            return;
        }
        let cloud = cx
            .try_global::<crate::product::sharing::SharingCloud>()
            .map(|cloud| cloud.0.clone())
            .filter(|_| self.client.is_some());
        let Some(cloud) = cloud else {
            // Shipping cannot open the setup without a session and says so.
            self.toast(
                Tone::Error,
                "Could not start the integration setup. Try again.",
                cx,
            );
            return;
        };
        let action = if connection.is_some() {
            "reconnect"
        } else {
            "connect"
        };
        let job = cloud.open_integration(
            integration.into(),
            action,
            connection.map(|connection| connection.id.to_string()),
            None,
        );
        self.opening = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.opening = false;
                if result.is_err() {
                    this.toast(
                        Tone::Error,
                        "Could not start the integration setup. Try again.",
                        cx,
                    );
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The integration callback may have connected or disconnected an account.
    pub fn refresh_connections(&mut self, cx: &mut Context<Self>) {
        self.connections = None;
        cx.notify();
    }

    fn request_connections(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        if self.connections.is_some() {
            return;
        }
        self.connections = Some(Load::Loading);
        let reply = self
            .runtime
            .submit(move |_| async move { client.connections().await });
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| {
                this.connections = Some(match result {
                    Ok(connections) => Load::Ready(connections),
                    Err(_) => Load::Failed,
                });
                cx.notify();
            });
        })
        .detach();
    }

    fn request_targets(&mut self, kind: TargetList, cx: &mut Context<Self>) {
        let Some(client) = self.client.clone() else {
            return;
        };
        if self.targets.contains_key(&kind) {
            return;
        }
        self.targets.insert(kind.clone(), Load::Loading);
        let request = kind.clone();
        let reply = self
            .runtime
            .submit(move |_| async move { client.targets(request).await });
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| {
                this.targets.insert(
                    kind,
                    match result {
                        Ok(choices) => Load::Ready(choices),
                        Err(_) => Load::Failed,
                    },
                );
                cx.notify();
            });
        })
        .detach();
    }

    fn toast(&mut self, tone: Tone, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        let total = if tone == Tone::Success { 3_000 } else { 5_000 };
        self.show_notice(
            Notice::Toast(Toast {
                tone,
                message: message.into(),
                remaining_ms: total,
                total_ms: total,
                paused: false,
            }),
            cx,
        );
    }

    /// `useNotifyPlanRequired`: a six-second warning toast with Upgrade.
    fn notify_pro(&mut self, cx: &mut Context<Self>) {
        self.show_notice(
            Notice::Plan(list::WarningToast {
                message: PRO_REQUIRED,
                action: "Upgrade",
                remaining_ms: 6_000,
                total_ms: 6_000,
                paused: false,
            }),
            cx,
        );
    }

    fn show_notice(&mut self, notice: Notice, cx: &mut Context<Self>) {
        const TICK: u64 = 100;
        self.notices += 1;
        let id = self.notices;
        self.notice = Some((id, notice));
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(TICK))
                    .await;
                let open = this.update(cx, |this, cx| {
                    let Some((_, notice)) =
                        this.notice.as_mut().filter(|(current, _)| *current == id)
                    else {
                        return false;
                    };
                    let (remaining, paused) = match notice {
                        Notice::Plan(toast) => (&mut toast.remaining_ms, toast.paused),
                        Notice::Toast(toast) => (&mut toast.remaining_ms, toast.paused),
                    };
                    if !paused {
                        *remaining = remaining.saturating_sub(TICK);
                        if *remaining == 0 {
                            this.notice = None;
                        }
                        cx.notify();
                    }
                    this.notice.is_some()
                });
                if !matches!(open, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn filename_input(
        &mut self,
        workflow: &Arc<str>,
        step: &Arc<str>,
        value: &str,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        if let Some((input, _)) = self.filenames.get(step) {
            return input.clone();
        }
        let input = cx.new(|cx| {
            let mut input = TextInput::new("{date} {title}.md", cx).inline();
            input.set_text(value.into(), cx);
            input
        });
        let (workflow, id) = (workflow.clone(), step.clone());
        let subscription = cx.subscribe(&input, move |this, input, event, cx| {
            if matches!(event, InputEvent::Changed) {
                let text = input.read(cx).buffer.text.clone();
                this.edit_step(
                    &workflow,
                    &id,
                    |step| {
                        if let Step::Markdown { options, .. } = step {
                            options
                                .get_or_insert_with(MarkdownOptions::default)
                                .filename = text;
                        }
                    },
                    cx,
                );
            }
        });
        self.filenames
            .insert(step.clone(), (input.clone(), subscription));
        input
    }

    fn notion_input(&mut self, key: &Arc<str>, cx: &mut Context<Self>) -> Entity<TextInput> {
        if let Some((input, _)) = self.notion_queries.get(key) {
            return input.clone();
        }
        let input = cx.new(|cx| TextInput::new("Search pages shared with Anarlog…", cx).inline());
        let subscription = cx.subscribe(&input, |this, _, event, cx| {
            if matches!(event, InputEvent::Submitted) {
                cx.notify();
                this.search_notion(cx);
            }
        });
        self.notion_queries
            .insert(key.clone(), (input.clone(), subscription));
        input
    }

    /// Submits every visible Notion search (one per selection at most).
    fn search_notion(&mut self, cx: &mut Context<Self>) {
        let Some(Load::Ready(connections)) = &self.connections else {
            return;
        };
        let Some(connection) = connections
            .iter()
            .find(|connection| connection.integration.as_ref() == "notion")
            .map(|connection| connection.id.clone())
        else {
            return;
        };
        let queries = self
            .notion_queries
            .values()
            .map(|(input, _)| input.read(cx).buffer.text.trim().to_owned())
            .collect::<Vec<_>>();
        for query in queries {
            let kind = TargetList::Notion(
                connection.clone(),
                (!query.is_empty()).then(|| query.into()),
            );
            self.targets.remove(&kind);
            self.request_targets(kind, cx);
        }
    }

    /// The sidebar's rows in order: starters, then workflows, then chat automations.
    fn visible_items(&self, search: &str) -> Vec<Selection> {
        let query = search.trim().to_lowercase();
        let matches = |text: &str| query.is_empty() || text.to_lowercase().contains(&query);
        let with_workflow = self
            .settings
            .workflows
            .iter()
            .filter_map(|workflow| workflow.chat_group_id.clone())
            .collect::<Vec<_>>();
        STARTERS
            .iter()
            .filter(|starter| matches(starter.title) || matches(starter.description))
            .map(|starter| Selection::Starter(starter.id))
            .chain(
                self.settings
                    .workflows
                    .iter()
                    .filter(|workflow| {
                        workflow.id.as_ref() != super::DRIVE_STARTER_ID && matches(&workflow.title)
                    })
                    .map(|workflow| Selection::Workflow(workflow.id.clone())),
            )
            .chain(
                self.chats
                    .iter()
                    .filter(|chat| !with_workflow.contains(&chat.id) && matches(&chat.title))
                    .map(|chat| Selection::Chat(chat.id.clone())),
            )
            .collect()
    }

    pub fn render_nav(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(window);
        let search = self.search.read(cx).buffer.text.clone();
        let searching = !search.is_empty();
        let items = self.visible_items(&search);
        let current = self.current();
        let starters = items
            .iter()
            .filter(|item| matches!(item, Selection::Starter(_)))
            .cloned()
            .collect::<Vec<_>>();
        let mine = items
            .iter()
            .filter(|item| !matches!(item, Selection::Starter(_)))
            .cloned()
            .collect::<Vec<_>>();
        let focused = self.search.focus_handle(cx).is_focused(window);
        let section = |title: &'static str| {
            div()
                .px_3()
                .pt_1()
                .pb_1()
                .text_xs()
                .line_height(px(16.))
                .font_weight(crate::ui::theme::MEDIUM)
                .text_color(colors.muted_foreground)
                .child(title)
        };
        div()
            .id("automations-nav")
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
                "automations-back",
                colors,
                cx.listener(|_, _, _, cx| cx.emit(AutomationsEvent::Back)),
                [
                    list::ghost_icon_button("automations-new", "PlusIcon", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.create(cx)))
                        .into_any_element(),
                ],
                cx,
            ))
            .child(list::search_field(
                &self.search,
                focused,
                searching,
                colors,
                cx.listener(|this, _, _, cx| {
                    this.search
                        .update(cx, |input, cx| input.set_text(String::new(), cx));
                }),
            ))
            .child(
                div()
                    .id("automations-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .pt_1()
                    .map(|view| {
                        if items.is_empty() {
                            view.child(list::empty_list(
                                "ZapIcon",
                                if searching {
                                    "No automations found"
                                } else {
                                    "No automations yet"
                                },
                                colors,
                            ))
                        } else {
                            view.when(!starters.is_empty(), |view| {
                                view.child(div().pb_2().child(section("Get started")).children(
                                    starters.iter().map(|item| {
                                        self.list_item(
                                            item,
                                            current.as_ref() == Some(item),
                                            colors,
                                            cx,
                                        )
                                    }),
                                ))
                            })
                            .when(!mine.is_empty(), |view| {
                                view.child(div().child(section("My automations")).children(
                                    mine.iter().map(|item| {
                                        self.list_item(
                                            item,
                                            current.as_ref() == Some(item),
                                            colors,
                                            cx,
                                        )
                                    }),
                                ))
                            })
                            .child(self.shared_library.clone())
                        }
                    }),
            )
            .when_some(self.context.clone(), |view, (position, item)| {
                view.child(self.context_menu(position, item, colors, cx))
            })
            .into_any_element()
    }

    fn list_item(
        &self,
        item: &Selection,
        selected: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let violet: Hsla = rgb(0x8b5cf6).into();
        let (key, glyph, title, subtitle): (
            SharedString,
            AnyElement,
            SharedString,
            Option<SharedString>,
        ) = match item {
            Selection::Starter(id) => (
                format!("starter-{id:?}").into(),
                starter_icon(*id, 16., false, colors),
                super::starter(*id).title.into(),
                None,
            ),
            Selection::Workflow(id) => {
                let Some(workflow) = self.settings.workflow(id) else {
                    return div().into_any_element();
                };
                (
                    format!("workflow-{id}").into(),
                    list::icon("ZapIcon", 16., violet).into_any_element(),
                    workflow.display_title().to_owned().into(),
                    Some(if workflow.enabled { "Enabled" } else { "Draft" }.into()),
                )
            }
            Selection::Chat(id) => {
                let Some(chat) = self.chats.iter().find(|chat| &chat.id == id) else {
                    return div().into_any_element();
                };
                (
                    format!("chat-{id}").into(),
                    list::icon("ZapIcon", 16., violet).into_any_element(),
                    chat.title.clone().into(),
                    relative(&chat.created_at).map(SharedString::from),
                )
            }
        };
        let (click, menu) = (item.clone(), item.clone());
        list::list_row(key, selected, false, colors)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.focus.focus(window, cx);
                this.select(click.clone(), cx);
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.select(menu.clone(), cx);
                    this.context = Some((event.position, menu.clone()));
                    cx.notify();
                }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .size(px(16.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(glyph),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(crate::ui::theme::MEDIUM)
                                    .child(title),
                            )
                            .when_some(subtitle, |view, subtitle| {
                                view.child(
                                    div()
                                        .mt(px(2.))
                                        .truncate()
                                        .text_xs()
                                        .line_height(px(16.))
                                        .text_color(colors.muted_foreground)
                                        .child(subtitle),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }

    fn context_menu(
        &self,
        position: Point<Pixels>,
        item: Selection,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let edit = item.clone();
        let remove_label = if matches!(item, Selection::Starter(_)) {
            "Remove"
        } else {
            "Delete"
        };
        deferred(
            anchored().position(position).snap_to_window().child(
                list::context_menu(colors)
                    .id("automation-context-menu")
                    .occlude()
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.context = None;
                        cx.notify();
                    }))
                    .child(
                        list::context_item("edit", "Edit", colors).on_click(cx.listener(
                            move |this, _, _, cx| {
                                this.context = None;
                                this.select(edit.clone(), cx);
                            },
                        )),
                    )
                    .child(div().mx(px(-4.)).my_1().h(px(1.)).bg(colors.border))
                    .child(list::context_item("remove", remove_label, colors).on_click(
                        cx.listener(move |this, _, _, cx| this.delete(item.clone(), cx)),
                    )),
            ),
        )
        .with_priority(1)
        .into_any_element()
    }

    fn render_overview(&self, colors: Theme) -> AnyElement {
        let column = column()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(page_title("Automations"))
                    .child(description(
                        "Automate what happens before, during, or after meetings based on the conditions you choose.",
                        colors,
                    )),
            )
            .child(
                div()
                    .min_h(px(224.))
                    .px_6()
                    .py_10()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .border_1()
                    .border_dashed()
                    .border_color(colors.border)
                    .bg(colors.muted.opacity(0.2))
                    .child(
                        div()
                            .size(px(44.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_1()
                            .border_color(colors.border)
                            .bg(colors.background)
                            .child(list::icon("ZapIcon", 20., colors.muted_foreground)),
                    )
                    .child(
                        div()
                            .mt_4()
                            .text_sm()
                            .line_height(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("No automation draft yet"),
                    )
                    .child(
                        div()
                            .mt_1()
                            .max_w(px(384.))
                            .text_center()
                            .text_xs()
                            .line_height(px(19.5))
                            .text_color(colors.muted_foreground)
                            .child("Choose a starter from the sidebar, or create a workflow and add steps like Zapier."),
                    ),
            );
        scroll_area("automations-overview", column).into_any_element()
    }

    /// `AutomationDetailsLayout`: the 48px header, then the scrolling body.
    fn details(
        &self,
        icon: AnyElement,
        title: SharedString,
        description: Option<SharedString>,
        actions: AnyElement,
        body: AnyElement,
        colors: Theme,
    ) -> AnyElement {
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
                                    .size(px(28.))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(icon),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .line_height(px(20.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(title),
                            ),
                    )
                    .child(actions),
            )
            .child(scroll_area(
                "automation-details",
                column()
                    .when_some(description, |view, text| {
                        view.child(description_text(text, colors))
                    })
                    .child(body),
            ))
            .into_any_element()
    }

    fn actions_menu(
        &self,
        label: &'static str,
        selection: Selection,
        window: &Window,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.menu == Some(Menu::Actions);
        div()
            .relative()
            .child(
                list::ghost_icon_button("automation-actions", "MoreHorizontalIcon", colors)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| this.toggle(Menu::Actions, cx)),
                    ),
            )
            .when(open, |view| {
                view.child(
                    div().absolute().top(px(32.)).right_0().child(
                        deferred(
                            anchored()
                                .anchor(gpui::Anchor::TopRight)
                                .snap_to_window()
                                .child(
                                    list::floating_menu(128., colors)
                                        .w_auto()
                                        .min_w(px(128.))
                                        .id("automation-actions-menu")
                                        .occlude()
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation()
                                        })
                                        .child(
                                            list::floating_menu_body(colors).child(
                                                list::menu_item(
                                                    "automation-delete",
                                                    label,
                                                    false,
                                                    false,
                                                    window,
                                                    colors,
                                                )
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.delete(selection.clone(), cx)
                                                })),
                                            ),
                                        ),
                                ),
                        )
                        .with_priority(1),
                    ),
                )
            })
            .into_any_element()
    }

    fn render_workflow(
        &mut self,
        selection: &Selection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = theme(window);
        let Some(workflow) = self.selected_workflow(selection) else {
            return div().into_any_element();
        };
        let (title, description): (SharedString, Option<SharedString>) = match selection {
            Selection::Chat(group) => {
                let chat = self.chats.iter().find(|chat| &chat.id == group);
                let title = chat
                    .map(|chat| chat.title.trim().to_owned())
                    .filter(|title| !title.is_empty())
                    .unwrap_or_else(|| workflow.display_title().to_owned());
                let created = chat
                    .and_then(|chat| relative(&chat.created_at))
                    .map(|created| format!("Created {created}").into());
                (title.into(), created)
            }
            _ => (
                workflow.display_title().to_owned().into(),
                Some(
                    "Add a trigger and actions. Chat on the right can help you refine the workflow."
                        .into(),
                ),
            ),
        };
        let ready_billing = Self::billing_ready(cx);
        let pro = account::is_pro(cx);
        let enabled = workflow.enabled;
        let toggle = workflow.clone();
        let enable_disabled = !ready_billing || self.pending || (pro && !workflow.ready());
        let actions = div()
            .flex()
            .items_center()
            .gap_2()
            .child(list::share_button("automation-share", colors).on_click({
                let (id, title): (Arc<str>, Arc<str>) =
                    (workflow.id.clone(), workflow.display_title().into());
                cx.listener(move |this, _, _, cx| this.share(id.clone(), title.clone(), cx))
            }))
            .child(if enabled {
                button(
                    "automation-disable",
                    None,
                    "Disable",
                    Variant::Outline,
                    ready_billing && !self.pending,
                    colors,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if Self::billing_ready(cx) && !this.pending {
                        this.set_enabled(toggle.clone(), false, cx)
                    }
                }))
            } else {
                button(
                    "automation-enable",
                    Some(catalog_icons::path("ZapIcon")),
                    "Save & enable",
                    Variant::Primary,
                    !enable_disabled,
                    colors,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !enable_disabled {
                        this.set_enabled(toggle.clone(), true, cx)
                    }
                }))
            })
            .child(self.actions_menu("Delete automation", selection.clone(), window, colors, cx))
            .into_any_element();
        let body = self.render_builder(&workflow, window, cx);
        self.details(
            list::icon("ZapIcon", 16., rgb(0x8b5cf6).into()).into_any_element(),
            title,
            description,
            actions,
            body,
            colors,
        )
    }

    /// `WorkflowBuilder`.
    fn render_builder(
        &mut self,
        workflow: &Workflow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = theme(window);
        let id = workflow.id.clone();
        let has_drive = workflow.has_drive();
        let trigger_open = self.menu == Some(Menu::Trigger);
        let trigger_id = id.clone();
        let trigger_select = div()
            .relative()
            .w_full()
            .max_w(px(320.))
            .child(
                select_trigger(
                    "workflow-trigger",
                    workflow.trigger.label().into(),
                    false,
                    true,
                    colors,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.toggle(Menu::Trigger, cx)),
                ),
            )
            .when(trigger_open, |view| {
                let options = [Trigger::NoteEnhanced, Trigger::MeetingCompleted]
                    .into_iter()
                    .map(|trigger| {
                        let id = trigger_id.clone();
                        SelectOption {
                            key: trigger.key().into(),
                            label: trigger.label().into(),
                            selected: workflow.trigger == trigger,
                            enabled: trigger == Trigger::NoteEnhanced || !has_drive,
                            on_select: Box::new(cx.listener(move |this, _, _, cx| {
                                this.edit_workflow(&id, |workflow| workflow.trigger = trigger, cx)
                            })),
                        }
                    })
                    .collect();
                view.child(select_menu(options, Some(320.), colors))
            });
        let mut cards = vec![workflow_card(
            WorkflowCardKind::Trigger,
            "When this happens".into(),
            "Trigger",
            true,
            None,
            trigger_select.into_any_element(),
            colors,
        )];
        for (index, step) in workflow.steps.iter().enumerate() {
            let remove = {
                let (id, step) = (id.clone(), step.id().clone());
                button_ghost_icon(&format!("remove-step-{index}"), colors).on_click(cx.listener(
                    move |this, _, _, cx| {
                        let step = step.clone();
                        this.edit_workflow(
                            &id,
                            |workflow| workflow.steps.retain(|item| item.id() != &step),
                            cx,
                        )
                    },
                ))
            };
            let content = self.render_step(workflow, step, window, cx);
            cards.push(
                div()
                    .child(arrow(colors))
                    .child(workflow_card(
                        WorkflowCardKind::Action,
                        format!("Then {}", index + 1).into(),
                        "Action",
                        step.ready(),
                        Some(remove.into_any_element()),
                        content,
                        colors,
                    ))
                    .into_any_element(),
            );
        }
        cards.push(arrow(colors));
        cards.push(self.render_add_step(workflow, colors, cx));
        div()
            .overflow_hidden()
            .border_1()
            .border_color(colors.border)
            .bg(colors.background)
            .child(
                div()
                    .px_5()
                    .py_4()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .truncate()
                                    .text_sm()
                                    .line_height(px(20.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Workflow"),
                            )
                            .child(badge(
                                if workflow.enabled { "Enabled" } else { "Draft" },
                                false,
                                colors,
                            )),
                    )
                    .child(
                        div()
                            .mt_1()
                            .text_xs()
                            .line_height(px(16.))
                            .text_color(colors.muted_foreground)
                            .child("Add a trigger, then stack actions like Zapier."),
                    ),
            )
            .child(div().p_5().flex().flex_col().gap_2().children(cards))
            .child(
                div()
                    .px_5()
                    .py_4()
                    .border_t_1()
                    .border_color(colors.border)
                    .when(!workflow.ready(), |view| {
                        view.child(
                            div()
                                .text_xs()
                                .line_height(px(16.))
                                .text_color(colors.muted_foreground)
                                .child("Add at least one configured action before enabling."),
                        )
                    })
                    .children(last_run_line(workflow.last_run.as_ref(), colors)),
            )
            .into_any_element()
    }

    fn render_add_step(
        &self,
        workflow: &Workflow,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.menu == Some(Menu::AddStep);
        let id = workflow.id.clone();
        let note_enhanced = workflow.trigger == Trigger::NoteEnhanced;
        div()
            .p_4()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .border_1()
            .border_dashed()
            .border_color(colors.border)
            .bg(colors.muted.opacity(0.2))
            .child(
                div()
                    .child(
                        div()
                            .text_sm()
                            .line_height(px(20.))
                            .font_weight(crate::ui::theme::MEDIUM)
                            .child("Add an action"),
                    )
                    .child(
                        div()
                            .mt_1()
                            .text_xs()
                            .line_height(px(16.))
                            .text_color(colors.muted_foreground)
                            .child("Stack another destination. Steps run top to bottom."),
                    ),
            )
            .child(
                div()
                    .relative()
                    .w(px(176.))
                    .flex_shrink_0()
                    .child(
                        div()
                            .id("add-step")
                            .h(px(32.))
                            .w_full()
                            .px(px(11.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .border_1()
                            .border_color(colors.input)
                            .shadow_xs()
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .cursor_pointer()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(list::icon("PlusIcon", 12., colors.muted_foreground))
                                    .child("Add step"),
                            )
                            .child(caret(colors))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| this.toggle(Menu::AddStep, cx)),
                            ),
                    )
                    .when(open, |view| {
                        let options = StepType::ALL
                            .into_iter()
                            .map(|kind| {
                                let id = id.clone();
                                SelectOption {
                                    key: format!("{kind:?}").into(),
                                    label: kind.short_label().into(),
                                    selected: false,
                                    enabled: kind != StepType::Drive || note_enhanced,
                                    on_select: Box::new(cx.listener(move |this, _, _, cx| {
                                        this.edit_workflow(
                                            &id,
                                            |workflow| workflow.steps.push(Step::new(kind)),
                                            cx,
                                        )
                                    })),
                                }
                            })
                            .collect();
                        view.child(select_menu(options, Some(176.), colors))
                    }),
            )
            .into_any_element()
    }

    fn render_step(
        &mut self,
        workflow: &Workflow,
        step: &Step,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = theme(window);
        let (workflow_id, step_id) = (workflow.id.clone(), step.id().clone());
        let open = self.menu == Some(Menu::StepType(step_id.clone()));
        let note_enhanced = workflow.trigger == Trigger::NoteEnhanced;
        let kind_select = div()
            .relative()
            .w_full()
            .max_w(px(320.))
            .child(
                select_trigger(
                    ElementId::Name(format!("step-type-{step_id}").into()),
                    step.kind().label().into(),
                    false,
                    true,
                    colors,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener({
                        let step_id = step_id.clone();
                        move |this, _, _, cx| this.toggle(Menu::StepType(step_id.clone()), cx)
                    }),
                ),
            )
            .when(open, |view| {
                let options = StepType::ALL
                    .into_iter()
                    .map(|kind| {
                        let (workflow_id, step_id) = (workflow_id.clone(), step_id.clone());
                        SelectOption {
                            key: format!("{kind:?}").into(),
                            label: kind.label().into(),
                            selected: step.kind() == kind,
                            enabled: kind != StepType::Drive || note_enhanced,
                            on_select: Box::new(cx.listener(move |this, _, _, cx| {
                                this.filenames.remove(&step_id);
                                this.edit_step(
                                    &workflow_id,
                                    &step_id,
                                    |step| *step = Step::new(kind),
                                    cx,
                                )
                            })),
                        }
                    })
                    .collect();
                view.child(select_menu(options, Some(320.), colors))
            });
        let config = self.render_step_config(Some(workflow), step, window, cx);
        div()
            .child(kind_select)
            .child(div().mt_3().child(config))
            .into_any_element()
    }

    /// `WorkflowStepConfig`, or a starter's config when `workflow` is `None`.
    fn render_step_config(
        &mut self,
        workflow: Option<&Workflow>,
        step: &Step,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = theme(window);
        let workflow_id = workflow.map(|workflow| workflow.id.clone());
        let step_id = step.id().clone();
        match step {
            Step::Markdown {
                directory, options, ..
            } => {
                let options = options.clone().unwrap_or_default();
                let target = match &workflow_id {
                    Some(workflow) => FolderTarget::Step(workflow.clone(), step_id.clone()),
                    None => FolderTarget::Starter,
                };
                let folder = self.folder_row(directory, target, colors, cx);
                let Some(workflow_id) = workflow_id else {
                    return folder;
                };
                let filename = self.filename_input(&workflow_id, &step_id, &options.filename, cx);
                div()
                    .child(folder)
                    .child(self.markdown_options(
                        &workflow_id,
                        &step_id,
                        &options,
                        filename,
                        colors,
                        cx,
                    ))
                    .into_any_element()
            }
            Step::Target { kind, target, .. } => {
                self.target_config(*kind, target.as_ref(), workflow_id, step_id, window, cx)
            }
            Step::Drive { .. } => match workflow {
                Some(workflow) => {
                    let workflow = workflow.clone();
                    self.drive_config(&workflow, false, step, window, cx)
                }
                None => div().into_any_element(),
            },
        }
    }

    fn folder_row(
        &self,
        directory: &str,
        target: FolderTarget,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let directory = directory.trim();
        config_row(
            "Export folder",
            if directory.is_empty() {
                "No folder selected yet.".into()
            } else {
                directory.to_owned().into()
            },
            Some(
                button(
                    "choose-folder",
                    Some(assets::path("FolderOpenIcon").into()),
                    "Choose folder",
                    Variant::Outline,
                    true,
                    colors,
                )
                .on_click(cx.listener(move |this, _, _, cx| this.choose_folder(target.clone(), cx)))
                .into_any_element(),
            ),
            colors,
        )
    }

    /// `MarkdownExportOptionsConfig`.
    fn markdown_options(
        &self,
        workflow: &Arc<str>,
        step: &Arc<str>,
        options: &MarkdownOptions,
        filename: Entity<TextInput>,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let field = |label: &'static str,
                     checked: bool,
                     apply: fn(&mut MarkdownOptions, bool),
                     cx: &mut Context<Self>| {
            let (workflow, step) = (workflow.clone(), step.clone());
            div()
                .id(label)
                .flex()
                .items_center()
                .gap(px(6.))
                .text_xs()
                .line_height(px(16.))
                .cursor_pointer()
                .child(checkbox(checked, colors))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.edit_step(
                        &workflow,
                        &step,
                        |step| {
                            if let Step::Markdown { options, .. } = step {
                                apply(
                                    options.get_or_insert_with(MarkdownOptions::default),
                                    !checked,
                                );
                            }
                        },
                        cx,
                    )
                }))
        };
        let label = |text: &'static str| {
            div()
                .text_xs()
                .line_height(px(16.))
                .font_weight(crate::ui::theme::MEDIUM)
                .child(text)
        };
        let mono = crate::ui::theme::monospace_font(cx);
        let code = |text: &'static str| div().font_family(mono.clone()).child(text);
        div()
            .mt_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                // A fieldset's legend sits outside its flex gap.
                div().child(label("Include").mb_2()).child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_x_4()
                                .gap_y_2()
                                .child(field("Memo", options.memo, |o, v| o.memo = v, cx))
                                .child(field("Summary", options.summary, |o, v| o.summary = v, cx))
                                .child(field(
                                    "Transcript",
                                    options.transcript,
                                    |o, v| o.transcript = v,
                                    cx,
                                ))
                                .child(field(
                                    "Collect action items",
                                    options.action_items,
                                    |o, v| o.action_items = v,
                                    cx,
                                )),
                        )
                        .when(!options.has_content(), |view| {
                            view.child(
                                div()
                                    .text_xs()
                                    .line_height(px(16.))
                                    .text_color(colors.destructive)
                                    .child("Choose at least one section to export."),
                            )
                        }),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(label("Filename"))
                    .child(
                        div()
                            .h(px(32.))
                            .w_full()
                            .px_3()
                            .flex()
                            .items_center()
                            .border_1()
                            .border_color(colors.input)
                            .shadow_xs()
                            // `Input`'s `md:text-sm` outlasts the `text-xs` it is given.
                            .text_sm()
                            .child(filename),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(px(16.))
                            .text_color(colors.muted_foreground)
                            .child("Leave blank to use the meeting date and title."),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_x_4()
                            .gap_y_1()
                            .text_xs()
                            .line_height(px(16.))
                            .text_color(colors.muted_foreground)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(code("{title}"))
                                    .child("Session title"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(code("{date}"))
                                    .child("Date"),
                            ),
                    ),
            )
            .child(
                div().flex().flex_col().gap_2().child(
                    field(
                        "Include meeting ID",
                        options.id_suffix,
                        |o, v| o.id_suffix = v,
                        cx,
                    )
                    .child(code("[a1b2c3d4]").text_color(colors.muted_foreground)),
                ),
            )
            .into_any_element()
    }

    /// `IntegrationGate`: the connect button until the integration is connected.
    fn gate(
        &mut self,
        integration: &'static str,
        name: &'static str,
        cx: &mut Context<Self>,
        colors: Theme,
    ) -> Result<Connection, AnyElement> {
        self.request_connections(cx);
        let connection = match &self.connections {
            Some(Load::Loading) => {
                return Err(div()
                    .text_xs()
                    .line_height(px(16.))
                    .text_color(colors.muted_foreground)
                    .child("Checking connection…")
                    .into_any_element());
            }
            Some(Load::Ready(connections)) => connections
                .iter()
                .find(|connection| connection.integration.as_ref() == integration)
                .cloned(),
            Some(Load::Failed) | None => None,
        };
        match connection {
            Some(connection) if !connection.reconnect => Ok(connection),
            connection => {
                let label = if connection.is_some() {
                    format!("Reconnect {name}")
                } else {
                    format!("Connect {name}")
                };
                Err(button(
                    ElementId::Name(format!("connect-{integration}").into()),
                    None,
                    label,
                    Variant::Outline,
                    !self.opening,
                    colors,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.connect(integration, connection.clone(), cx)
                }))
                .into_any_element())
            }
        }
    }

    /// `SlackRecapConfig`, `LinearIssuesConfig` and `NotionUpdateConfig`.
    fn target_config(
        &mut self,
        kind: StepType,
        selected: Option<&Target>,
        workflow: Option<Arc<str>>,
        step: Arc<str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = theme(window);
        let (title, empty, integration, name) = match kind {
            StepType::Slack => (
                "Slack channel",
                "No channel selected yet.",
                "slack",
                "Slack",
            ),
            StepType::Linear => ("Linear team", "No team selected yet.", "linear", "Linear"),
            _ => ("Notion page", "No page selected yet.", "notion", "Notion"),
        };
        let value: SharedString = match (kind, selected) {
            (StepType::Slack, Some(target)) => format!("#{}", target.name).into(),
            (_, Some(target)) => target.name.clone().into(),
            (_, None) => empty.into(),
        };
        let gate = self.gate(integration, name, cx, colors);
        let menu_key: Arc<str> = workflow
            .as_ref()
            .map_or_else(|| STARTER_MENU.into(), |_| step.clone());
        let apply = {
            let workflow = workflow.clone();
            let step = step.clone();
            move |this: &mut Self, target: Target, cx: &mut Context<Self>| match &workflow {
                Some(workflow) => this.edit_step(
                    workflow,
                    &step,
                    |step| {
                        if let Step::Target {
                            target: current, ..
                        } = step
                        {
                            *current = Some(target);
                        }
                    },
                    cx,
                ),
                None => {
                    let id = match kind {
                        StepType::Slack => StarterId::SlackRecap,
                        StepType::Linear => StarterId::LinearActionItems,
                        _ => StarterId::NotionProjectNotes,
                    };
                    let stored =
                        serde_json::json!({"id": target.id, "name": target.name}).to_string();
                    this.starter(id, StarterCommand::SetTarget(stored), cx)
                }
            }
        };
        let control = match gate {
            // Notion's gate sits under its row and stretches across the column.
            Err(element) if kind == StepType::Notion => {
                return div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(config_row(title, value, None, colors))
                    .child(div().flex().flex_col().child(element))
                    .into_any_element();
            }
            Err(element) => element,
            Ok(connection) if kind == StepType::Notion => {
                return self.notion_search(
                    &menu_key,
                    &connection,
                    selected,
                    title,
                    value,
                    apply,
                    colors,
                    cx,
                );
            }
            Ok(connection) => {
                let list = match kind {
                    StepType::Slack => TargetList::Slack,
                    _ => TargetList::Linear(connection.id.clone()),
                };
                self.request_targets(list.clone(), cx);
                let choices = match self.targets.get(&list) {
                    Some(Load::Ready(choices)) => Some(choices.clone()),
                    _ => None,
                };
                let loading = matches!(self.targets.get(&list), Some(Load::Loading));
                let placeholder = match (kind, loading) {
                    (StepType::Slack, true) => "Loading channels…",
                    (StepType::Slack, false) => "Choose a channel",
                    (_, true) => "Loading teams…",
                    (_, false) => "Choose a team",
                };
                let open = self.menu == Some(Menu::Targets(menu_key.clone()));
                let label = selected.map(|target| SharedString::from(target.name.to_string()));
                let apply = Arc::new(apply);
                div()
                    .relative()
                    .w(px(208.))
                    .child(
                        select_trigger(
                            ElementId::Name(format!("targets-{menu_key}").into()),
                            label.unwrap_or_else(|| placeholder.into()),
                            selected.is_none(),
                            !loading,
                            colors,
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener({
                                let menu_key = menu_key.clone();
                                move |this, _, _, cx| {
                                    if !loading {
                                        this.toggle(Menu::Targets(menu_key.clone()), cx);
                                    }
                                }
                            }),
                        ),
                    )
                    .when(open, |view| {
                        let options = choices
                            .unwrap_or_default()
                            .into_iter()
                            .map(|choice| {
                                let apply = apply.clone();
                                let target = Target {
                                    id: choice.id.clone(),
                                    name: choice.name.clone(),
                                };
                                SelectOption {
                                    key: choice.id.to_string().into(),
                                    label: if kind == StepType::Slack && !choice.private {
                                        format!("# {}", choice.name).into()
                                    } else {
                                        choice.name.to_string().into()
                                    },
                                    selected: selected
                                        .is_some_and(|current| current.id == choice.id),
                                    enabled: true,
                                    on_select: Box::new(cx.listener(move |this, _, _, cx| {
                                        apply(this, target.clone(), cx)
                                    })),
                                }
                            })
                            .collect();
                        view.child(select_menu(options, Some(208.), colors))
                    })
                    .into_any_element()
            }
        };
        config_row(title, value, Some(control), colors)
    }

    #[allow(clippy::too_many_arguments)]
    fn notion_search(
        &mut self,
        key: &Arc<str>,
        connection: &Connection,
        selected: Option<&Target>,
        title: &'static str,
        value: SharedString,
        apply: impl Fn(&mut Self, Target, &mut Context<Self>) + 'static,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let input = self.notion_input(key, cx);
        let query = input.read(cx).buffer.text.trim().to_owned();
        let submitted = self
            .targets
            .iter()
            .find(|(kind, _)| matches!(kind, TargetList::Notion(id, q) if id == &connection.id && q.as_deref().unwrap_or("") == query))
            .map(|(_, load)| load);
        let fetching = matches!(submitted, Some(Load::Loading));
        let pages = match submitted {
            Some(Load::Ready(pages)) => Some(pages.clone()),
            _ => None,
        };
        let apply = Arc::new(apply);
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(config_row(title, value, None, colors))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .h(px(32.))
                                    .flex_1()
                                    .min_w_0()
                                    .px_3()
                                    .flex()
                                    .items_center()
                                    .border_1()
                                    .border_color(colors.border)
                                    .bg(colors.accent.opacity(0.5))
                                    .text_xs()
                                    .child(input),
                            )
                            .child(
                                button(
                                    "notion-search",
                                    Some(if fetching {
                                        assets::path("LoaderCircleIcon").into()
                                    } else {
                                        catalog_icons::path("Search01Icon")
                                    }),
                                    "Search",
                                    Variant::Outline,
                                    !fetching,
                                    colors,
                                )
                                .on_click(cx.listener(|this, _, _, cx| this.search_notion(cx))),
                            ),
                    )
                    .map(|view| match pages {
                        Some(pages) if !pages.is_empty() => view.child(
                            div().flex().flex_col().gap_1().children(pages.into_iter().map(|page| {
                                let apply = apply.clone();
                                let target = Target {
                                    id: page.id.clone(),
                                    name: page.name.clone(),
                                };
                                let current = selected.is_some_and(|selected| selected.id == page.id);
                                div()
                                    .id(ElementId::Name(format!("notion-{}", page.id).into()))
                                    .px_3()
                                    .py(px(6.))
                                    .text_xs()
                                    .line_height(px(16.))
                                    .cursor_pointer()
                                    .map(|row| {
                                        if current {
                                            row.bg(colors.accent)
                                        } else {
                                            row.hover(|style| style.bg(colors.accent.opacity(0.5)))
                                        }
                                    })
                                    .child(page.name.to_string())
                                    .on_click(cx.listener(move |this, _, _, cx| apply(this, target.clone(), cx)))
                            })),
                        ),
                        Some(_) => view.child(
                            div()
                                .text_xs()
                                .line_height(px(16.))
                                .text_color(colors.muted_foreground)
                                .child("No pages found. Share the page with the Anarlog integration in Notion first."),
                        ),
                        None => view,
                    }),
            )
            .into_any_element()
    }

    /// `GoogleDriveConfig` and `DriveExportResult`.
    fn drive_config(
        &mut self,
        workflow: &Workflow,
        starter: bool,
        step: &Step,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = theme(window);
        let Step::Drive {
            google_docs,
            connection_id,
            target,
            ..
        } = step
        else {
            return div().into_any_element();
        };
        let key: Arc<str> = if starter {
            STARTER_MENU.into()
        } else {
            step.id().clone()
        };
        let open = self.menu == Some(Menu::DriveFormat(key.clone()));
        let change = {
            let workflow = (!starter).then(|| workflow.id.clone());
            let step = step.clone();
            move |this: &mut Self, google: bool, cx: &mut Context<Self>| {
                let mut next = step.clone();
                if let Step::Drive { google_docs, .. } = &mut next {
                    *google_docs = google;
                }
                match &workflow {
                    Some(workflow) => {
                        let id = next.id().clone();
                        this.edit_step(workflow, &id, |step| *step = next, cx)
                    }
                    None => this.starter(
                        StarterId::GoogleDrive,
                        StarterCommand::ConfigureDrive(next),
                        cx,
                    ),
                }
            }
        };
        let change = Arc::new(change);
        let format = div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .text_sm()
            .line_height(px(20.))
            .child("File format")
            .child(
                div()
                    .relative()
                    .w_full()
                    .child(
                        select_trigger(
                            ElementId::Name(format!("drive-format-{key}").into()),
                            if *google_docs {
                                "Google Docs"
                            } else {
                                "Markdown (.md)"
                            }
                            .into(),
                            false,
                            true,
                            colors,
                        )
                        .h(px(36.))
                        .text_sm()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener({
                                let key = key.clone();
                                move |this, _, _, cx| {
                                    this.toggle(Menu::DriveFormat(key.clone()), cx)
                                }
                            }),
                        ),
                    )
                    .when(open, |view| {
                        let options = [(false, "Markdown (.md)"), (true, "Google Docs")]
                            .into_iter()
                            .map(|(google, label)| {
                                let change = change.clone();
                                SelectOption {
                                    key: label.into(),
                                    label: label.into(),
                                    selected: *google_docs == google,
                                    enabled: true,
                                    on_select: Box::new(
                                        cx.listener(move |this, _, _, cx| change(this, google, cx)),
                                    ),
                                }
                            })
                            .collect();
                        view.child(select_menu(options, None, colors))
                    }),
            );
        let gate = self.gate("google-drive", "Google Drive", cx, colors);
        let picker = match gate {
            Err(element) => element,
            Ok(connection) => div()
                .flex()
                .flex_col()
                .items_start()
                .gap_2()
                .child(
                    button(
                        "drive-folder",
                        None,
                        match target {
                            Some(target) if connection_id.as_str() == connection.id.as_ref() => {
                                target.name.to_string()
                            }
                            _ => "Choose Google Drive folder".into(),
                        },
                        Variant::Outline,
                        true,
                        colors,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toast(
                            Tone::Error,
                            "Choosing a Google Drive folder is not available in this app yet",
                            cx,
                        )
                    })),
                )
                .child(
                    button(
                        "drive-reconnect",
                        None,
                        "Reconnect Google Drive",
                        Variant::Link,
                        !self.opening,
                        colors,
                    )
                    .on_click({
                        let connection = connection.clone();
                        cx.listener(move |this, _, _, cx| {
                            this.connect("google-drive", Some(connection.clone()), cx)
                        })
                    }),
                )
                .when(
                    !connection_id.is_empty() && connection_id.as_str() != connection.id.as_ref(),
                    |view| {
                        view.child(
                            div()
                                .text_xs()
                                .line_height(px(16.))
                                .text_color(colors.destructive)
                                .child("Your Google account changed. Choose a folder again."),
                        )
                    },
                )
                .into_any_element(),
        };
        let result = workflow.drive_result(step).map(|run| {
            let url = (run.success)
                .then(|| run.file_id.clone())
                .flatten()
                .map(|file| format!("https://drive.google.com/file/d/{file}/view"));
            div()
                .mt_2()
                .flex()
                .flex_col()
                .items_start()
                .gap_2()
                .text_xs()
                .map(|view| match url {
                    Some(url) => view.child(
                        button(
                            "drive-open",
                            None,
                            "Open exported file in Google Drive",
                            Variant::Link,
                            true,
                            colors,
                        )
                        .on_click(move |_, _, cx| cx.open_url(&url)),
                    ),
                    None => view
                        .child(div().text_color(colors.destructive).child(
                            if run.detail.is_empty() {
                                "Google Drive export did not finish.".to_owned()
                            } else {
                                run.detail.to_string()
                            },
                        ))
                        .child(button(
                            "drive-retry",
                            None,
                            "Retry Google Drive export",
                            Variant::Outline,
                            false,
                            colors,
                        )),
                })
        });
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap_2()
            .child(format)
            .when(*google_docs, |view| {
                view.child(
                    div()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(colors.muted_foreground)
                        .child("Updating a note replaces the Google document's content, including edits made in Google Docs. Changing formats keeps previously exported files."),
                )
            })
            .child(picker)
            .child(
                div()
                    .text_xs()
                    .line_height(px(16.))
                    .text_color(colors.muted_foreground)
                    .child("Every note summarized after enabling this automation is saved here as one file with its summary and transcript in the selected format. Anarlog must be open."),
            )
            .children(result)
            .into_any_element()
    }

    /// `StarterAutomationDetails`.
    fn render_starter(
        &mut self,
        id: StarterId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = theme(window);
        let starter = super::starter(id);
        let enabled = self.settings.starter_enabled(id);
        let ready = self.settings.starter_ready(id);
        let billing = Self::billing_ready(cx);
        let pro = account::is_pro(cx);
        let pending = self.pending;
        let enable_blocked = !billing || pending || (pro && !ready);
        let header = div()
            .px_5()
            .py_4()
            .flex()
            .flex_wrap()
            .items_start()
            .justify_between()
            .gap_3()
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(list::icon("ZapIcon", 17., colors.primary))
                            .child(
                                div()
                                    .truncate()
                                    .text_sm()
                                    .line_height(px(20.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(starter.title),
                            )
                            .child(badge(
                                if enabled { "Enabled" } else { "Draft" },
                                false,
                                colors,
                            )),
                    )
                    .child(
                        div()
                            .mt_1()
                            .text_xs()
                            .line_height(px(16.))
                            .text_color(colors.muted_foreground)
                            .child("Steps run from top to bottom."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        button(
                            "starter-preview",
                            Some(assets::path("EyeIcon").into()),
                            if self.show_preview {
                                "Hide preview"
                            } else {
                                "Preview"
                            },
                            Variant::Outline,
                            true,
                            colors,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_preview = !this.show_preview;
                            cx.notify();
                        })),
                    )
                    .child(button(
                        "starter-test",
                        Some(assets::path("PlayIcon").into()),
                        "Test",
                        Variant::Outline,
                        false,
                        colors,
                    ))
                    .child(
                        button(
                            "starter-save",
                            Some(assets::path("FloppyDiskIcon").into()),
                            "Save draft",
                            Variant::Primary,
                            billing && !pending,
                            colors,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !Self::billing_ready(cx) || this.pending {
                            } else if !account::is_pro(cx) {
                                this.notify_pro(cx);
                            } else {
                                this.starter(id, StarterCommand::SaveDraft, cx);
                            }
                        })),
                    )
                    .child(if enabled {
                        button(
                            "starter-disable",
                            None,
                            "Disable",
                            Variant::Outline,
                            billing && !pending,
                            colors,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if Self::billing_ready(cx) && !this.pending {
                                this.starter(id, StarterCommand::SetEnabled(false), cx);
                            }
                        }))
                    } else {
                        button(
                            "starter-enable",
                            Some(catalog_icons::path("ZapIcon")),
                            "Save & enable",
                            Variant::Primary,
                            !enable_blocked,
                            colors,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if enable_blocked {
                            } else if !account::is_pro(cx) {
                                this.notify_pro(cx);
                            } else {
                                this.starter(id, StarterCommand::SetEnabled(true), cx);
                            }
                        }))
                    }),
            );
        let mut steps = Vec::new();
        for (index, (kind, title, detail)) in starter.steps.iter().enumerate() {
            let (bg, fg) = step_colors(
                match kind {
                    StepKind::Trigger => WorkflowCardKind::Trigger,
                    StepKind::Ai => WorkflowCardKind::Ai,
                    StepKind::Action => WorkflowCardKind::Action,
                },
                window,
            );
            steps.push(
                div()
                    .p_4()
                    .flex()
                    .items_start()
                    .gap_3()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.card)
                    .child(
                        div()
                            .mt(px(2.))
                            .size(px(28.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(bg)
                            .text_color(fg)
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .map(|badge| {
                                if *kind == StepKind::Ai {
                                    badge.child(list::icon("SparkleIcon", 13., fg))
                                } else {
                                    badge.child(format!("{}", index + 1))
                                }
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .line_height(px(20.))
                                            .font_weight(crate::ui::theme::MEDIUM)
                                            .child(*title),
                                    )
                                    .child(badge(
                                        match kind {
                                            StepKind::Ai => "AI step",
                                            StepKind::Trigger => "Trigger",
                                            StepKind::Action => "Action",
                                        },
                                        true,
                                        colors,
                                    )),
                            )
                            .child(
                                div()
                                    .mt_1()
                                    .text_xs()
                                    .line_height(px(19.5))
                                    .text_color(colors.muted_foreground)
                                    .child(*detail),
                            ),
                    ),
            );
        }
        let count = steps.len();
        let steps = steps.into_iter().enumerate().map(|(index, card)| {
            div()
                .child(card)
                .when(index + 1 < count, |view| view.child(arrow(colors)))
        });
        let config = match id.keys() {
            None => {
                let workflow = self.settings.drive_starter();
                match workflow.steps.first().cloned() {
                    Some(step) => self.drive_config(&workflow, true, &step, window, cx),
                    None => div().into_any_element(),
                }
            }
            Some([_, target_key, _]) => {
                let raw = self.settings.text(target_key).to_owned();
                let step = match id {
                    StarterId::MarkdownExport => Step::Markdown {
                        id: STARTER_MENU.into(),
                        directory: raw,
                        options: None,
                    },
                    _ => Step::Target {
                        id: STARTER_MENU.into(),
                        kind: match id {
                            StarterId::SlackRecap => StepType::Slack,
                            StarterId::LinearActionItems => StepType::Linear,
                            _ => StepType::Notion,
                        },
                        target: Target::parse_stored(&raw),
                    },
                };
                self.render_step_config(None, &step, window, cx)
            }
        };
        let last_run = self.settings.starter_last_run(id);
        let section = div()
            .overflow_hidden()
            .border_1()
            .border_color(colors.border)
            .bg(colors.background)
            .child(header)
            .child(div().p_5().flex().flex_col().gap_2().children(steps))
            .child(
                div()
                    .px_5()
                    .py_4()
                    .border_t_1()
                    .border_color(colors.border)
                    .child(config)
                    .children(last_run_line(last_run.as_ref(), colors)),
            )
            .when(self.show_preview, |view| {
                view.child(
                    div()
                        .px_5()
                        .py_4()
                        .border_t_1()
                        .border_color(colors.border)
                        .bg(colors.muted.opacity(0.35))
                        .child(
                            div()
                                .flex()
                                .items_start()
                                .gap_3()
                                .child(
                                    svg()
                                        .path(assets::path("EyeIcon"))
                                        .mt(px(2.))
                                        .size(px(15.))
                                        .flex_shrink_0()
                                        .text_color(colors.muted_foreground),
                                )
                                .child(
                                    div()
                                        .child(
                                            div()
                                                .text_xs()
                                                .line_height(px(16.))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child("Expected output"),
                                        )
                                        .child(
                                            div()
                                                .mt_1()
                                                .text_xs()
                                                .line_height(px(19.5))
                                                .text_color(colors.muted_foreground)
                                                .child(starter.preview),
                                        ),
                                ),
                        ),
                )
            });
        let actions = self.actions_menu(
            "Remove automation",
            Selection::Starter(id),
            window,
            colors,
            cx,
        );
        self.details(
            starter_icon(id, 16., true, colors),
            starter.title.into(),
            Some(starter.description.into()),
            actions,
            section.into_any_element(),
            colors,
        )
    }

    fn render_notice(
        &self,
        window: &Window,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (_, notice) = self.notice.as_ref()?;
        let dismiss = cx.listener(|this, _, _, cx| {
            this.notice = None;
            cx.notify();
        });
        let pause = cx.listener(|this, _, _, cx| {
            match &mut this.notice {
                Some((_, Notice::Plan(toast))) => toast.paused = !toast.paused,
                Some((_, Notice::Toast(toast))) => toast.paused = !toast.paused,
                None => {}
            }
            cx.notify();
        });
        Some(match notice {
            Notice::Plan(toast) => toast.render(
                [
                    Box::new(cx.listener(|this, _, _, cx| {
                        this.notice = None;
                        cx.emit(AutomationsEvent::Product(ProductRoute::Billing));
                        cx.notify();
                    })),
                    Box::new(dismiss),
                    Box::new(pause),
                ],
                window,
                colors,
            ),
            Notice::Toast(toast) => toast.render(pause, window, colors),
        })
    }
}

#[derive(Clone)]
enum FolderTarget {
    Starter,
    Step(Arc<str>, Arc<str>),
}

impl Toast {
    fn render(
        &self,
        on_pause: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
        window: &Window,
        colors: Theme,
    ) -> AnyElement {
        let (icon, color, bar): (SharedString, Hsla, Hsla) = match self.tone {
            Tone::Success => (
                "product/CheckmarkCircle02Icon.svg".into(),
                rgb(if is_dark(window) { 0x34d399 } else { 0x059669 }).into(),
                rgb(0x10b981).into(),
            ),
            Tone::Error => (
                catalog_icons::path("AlertCircleIcon"),
                colors.destructive,
                colors.destructive,
            ),
        };
        let seconds = self.remaining_ms.div_ceil(1000);
        let progress = self.remaining_ms as f32 / self.total_ms.max(1) as f32;
        deferred(
            div()
                .id("app-toast")
                .absolute()
                .bottom(px(16.))
                .right(px(16.))
                .w(px(320.))
                .overflow_hidden()
                .border_1()
                .border_color(colors.border.opacity(0.7))
                .bg(colors.popover)
                .text_color(colors.popover_foreground)
                .shadow_md()
                .occlude()
                .child(
                    div()
                        .min_h(px(44.))
                        .px(px(10.))
                        .py(px(6.))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .size(px(32.))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(svg().path(icon).size(px(16.)).text_color(color)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(px(13.))
                                .line_height(px(20.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(self.message.clone()),
                        ),
                )
                .child(
                    div()
                        .id("toast-timer")
                        .relative()
                        .min_h(px(32.))
                        .px(px(10.))
                        .py(px(6.))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .border_t_1()
                        .border_color(colors.border.opacity(0.6))
                        .bg(colors.muted.opacity(0.5))
                        .text_size(px(11.))
                        .text_color(colors.muted_foreground)
                        .cursor_pointer()
                        .hover(|style| style.bg(colors.muted))
                        .on_click(on_pause)
                        .child(
                            div()
                                .flex()
                                .gap(px(3.))
                                .child(if self.paused {
                                    "Timer paused"
                                } else {
                                    "Closing in"
                                })
                                .child(
                                    div()
                                        .text_color(colors.foreground)
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(seconds.to_string()),
                                )
                                .child(if seconds == 1 { "second" } else { "seconds" }),
                        )
                        .child(
                            div()
                                .text_color(colors.foreground)
                                .font_weight(crate::ui::theme::MEDIUM)
                                .child(if self.paused { "Resume" } else { "Pause" }),
                        )
                        .child(
                            div()
                                .absolute()
                                .left_0()
                                .bottom_0()
                                .h(px(2.))
                                .w(gpui::relative(progress))
                                .bg(bar),
                        ),
                ),
        )
        .with_priority(1)
        .into_any_element()
    }
}

impl Drop for AutomationsView {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl Render for AutomationsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let current = self.current();
        let content = match &current {
            None => self.render_overview(colors),
            Some(Selection::Starter(id)) => self.render_starter(*id, window, cx),
            Some(selection) => self.render_workflow(selection, window, cx),
        };
        let notice = self.render_notice(window, colors, cx);
        div()
            .id("automations")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .when(is_dark(window), |view| view.bg(colors.accent))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.menu.is_some() {
                        this.menu = None;
                        cx.notify();
                    }
                }),
            )
            .child(content)
            .children(notice)
            .when_some(self.sharing.as_ref(), |view, (dialog, _)| {
                view.child(list::modal(window, dialog.clone().into_any_element()))
            })
    }
}

/// `scroll-fade-y scrollbar-hide overflow-y-auto px-6 pt-3 pb-8` around a centered
/// `max-w-5xl` column with `gap-6`.
fn scroll_area(id: &'static str, column: Div) -> Div {
    div().flex_1().min_h_0().w_full().child(
        div()
            .id(id)
            .size_full()
            .overflow_y_scroll()
            .px_6()
            .pt_3()
            .pb_8()
            .child(column),
    )
}

fn column() -> Div {
    div()
        .mx_auto()
        .w_full()
        .max_w(px(1024.))
        .flex()
        .flex_col()
        .gap_6()
}

fn description(text: &'static str, colors: Theme) -> Div {
    description_text(text.into(), colors)
}

fn description_text(text: SharedString, colors: Theme) -> Div {
    div()
        .max_w(px(672.))
        .text_sm()
        .line_height(px(22.75))
        .text_color(colors.muted_foreground)
        .child(text)
}

/// A starter's brand mark. The Markdown mark is wide; the 16px sidebar box squeezes it.
fn starter_icon(id: StarterId, size: f32, header: bool, colors: Theme) -> AnyElement {
    let name = match id {
        StarterId::GoogleDrive => "google-drive",
        StarterId::SlackRecap => "slack-icon",
        StarterId::NotionProjectNotes => "notion-icon",
        StarterId::LinearActionItems => "linear-icon",
        StarterId::MarkdownExport => {
            let width = if header { size * 208. / 128. } else { size };
            return svg()
                .path(assets::path("MarkdownMark"))
                .w(px(width))
                .h(px(width * 128. / 208.))
                .flex_shrink_0()
                .text_color(colors.foreground)
                .into_any_element();
        }
    };
    img(SharedString::from(assets::path(name)))
        .size(px(size))
        .flex_shrink_0()
        .into_any_element()
}

#[derive(Clone, Copy, PartialEq)]
enum Variant {
    Primary,
    Outline,
    Link,
}

/// `Button size="sm"` (`h-7 px-2 text-xs`), with an optional 14px leading icon.
fn button(
    id: impl Into<ElementId>,
    icon: Option<SharedString>,
    label: impl Into<SharedString>,
    variant: Variant,
    enabled: bool,
    colors: Theme,
) -> Stateful<Div> {
    let foreground = match variant {
        Variant::Primary => colors.primary_foreground,
        Variant::Outline => colors.foreground,
        Variant::Link => colors.primary,
    };
    div()
        .id(id)
        .h(px(28.))
        .px_2()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .gap_2()
        .text_xs()
        .line_height(px(16.))
        .font_weight(crate::ui::theme::MEDIUM)
        .whitespace_nowrap()
        .text_color(foreground)
        .map(|button| match variant {
            Variant::Primary => button.bg(colors.primary).shadow_xs(),
            // Enabled buttons draw their smooth-corner border inside the padding; disabled
            // ones fall back to the CSS border.
            Variant::Outline => button
                .when(enabled, |button| button.px(px(7.)))
                .border_1()
                .border_color(colors.input)
                .bg(colors.background)
                .shadow_xs(),
            Variant::Link => button,
        })
        .when_some(icon, |button, icon| {
            button.child(
                svg()
                    .path(icon)
                    .size(px(14.))
                    .flex_shrink_0()
                    .text_color(foreground),
            )
        })
        .child(label.into())
        .map(|button| {
            if !enabled {
                button.opacity(0.5).cursor_not_allowed()
            } else {
                button.cursor_pointer().hover(move |style| match variant {
                    Variant::Primary => style.bg(colors.primary.opacity(0.9)),
                    Variant::Outline => style.bg(colors.accent),
                    Variant::Link => style.underline(),
                })
            }
        })
}

fn button_ghost_icon(id: &str, colors: Theme) -> Stateful<Div> {
    div()
        .id(ElementId::Name(id.to_owned().into()))
        .ml_auto()
        .size(px(28.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|style| style.bg(colors.accent))
        .child(
            svg()
                .path(assets::path("TrashIcon"))
                .size(px(13.))
                .text_color(colors.muted_foreground),
        )
}

/// `Badge variant="outline"`; `small` is `size="sm"`. Shipping's smooth corners draw the
/// border inside the padding box.
fn badge(label: &'static str, small: bool, colors: Theme) -> Div {
    div()
        .flex_shrink_0()
        .px(px(if small { 7. } else { 9. }))
        .py(px(1.))
        .border_1()
        .border_color(colors.border)
        .text_xs()
        .line_height(px(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colors.foreground)
        .child(label)
}

fn caret(colors: Theme) -> gpui::Svg {
    svg()
        .path(assets::path("ArrowDown01Icon"))
        .size(px(16.))
        .mr(px(-4.))
        .flex_shrink_0()
        .text_color(colors.foreground.opacity(0.5))
}

/// `SelectTrigger className="h-8 text-xs"`.
fn select_trigger(
    id: impl Into<ElementId>,
    label: SharedString,
    placeholder: bool,
    enabled: bool,
    colors: Theme,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(32.))
        .w_full()
        .px(px(11.))
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .border_1()
        .border_color(colors.input)
        .shadow_xs()
        .text_xs()
        .child(
            div()
                .min_w_0()
                .truncate()
                .when(placeholder, |view| view.text_color(colors.muted_foreground))
                .child(label),
        )
        .child(caret(colors))
        .map(|trigger| {
            if enabled {
                trigger.cursor_pointer()
            } else {
                trigger.opacity(0.5).cursor_not_allowed()
            }
        })
}

type Handler = Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App)>;

struct SelectOption {
    key: SharedString,
    label: SharedString,
    selected: bool,
    enabled: bool,
    on_select: Handler,
}

/// `SelectContent` under its trigger: at least the trigger's width.
fn select_menu(options: Vec<SelectOption>, width: Option<f32>, colors: Theme) -> AnyElement {
    dropdown(
        div()
            .id("select-menu")
            .mt_1()
            .min_w(px(width.unwrap_or(128.).max(128.)))
            .p_1()
            .flex()
            .flex_col()
            .border_1()
            .border_color(colors.border)
            .bg(colors.popover)
            .text_color(colors.popover_foreground)
            .shadow_md()
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(options.into_iter().map(|option| {
                div()
                    .id(ElementId::Name(option.key))
                    .relative()
                    .w_full()
                    .py(px(6.))
                    .pl_2()
                    .pr_8()
                    .text_sm()
                    .line_height(px(20.))
                    .whitespace_nowrap()
                    // Radix focuses the selected item when the menu opens.
                    .when(option.selected, |item| item.bg(colors.accent))
                    .map(|item| {
                        if option.enabled {
                            item.cursor_pointer()
                                .hover(|style| style.bg(colors.accent))
                                .on_click(option.on_select)
                        } else {
                            item.opacity(0.5)
                        }
                    })
                    .child(option.label)
                    .when(option.selected, |item| {
                        item.child(
                            div()
                                .absolute()
                                .right_2()
                                .top_0()
                                .bottom_0()
                                .flex()
                                .items_center()
                                .child(list::icon("CheckIcon", 16., colors.foreground)),
                        )
                    })
            })),
    )
    .into_any_element()
}

/// Places `menu` directly under the element it is attached to, above all other content.
fn dropdown(menu: impl IntoElement) -> Div {
    div()
        .absolute()
        .top_full()
        .left_0()
        .child(deferred(anchored().snap_to_window_with_margin(px(8.)).child(menu)).with_priority(1))
}

/// `SettingsPageTitle`: `font-hand text-3xl leading-tight font-semibold`.
fn page_title(title: &'static str) -> Div {
    let hand = if cfg!(target_os = "macos") {
        "Bradley Hand"
    } else if cfg!(target_os = "windows") {
        "Segoe Print"
    } else {
        "Comic Sans MS"
    };
    div()
        .font_family(hand)
        .text_size(px(30.))
        .line_height(px(37.5))
        .font_weight(FontWeight::SEMIBOLD)
        .child(title)
}

/// The native checkbox in shipping's `accent-primary`.
fn checkbox(checked: bool, colors: Theme) -> Div {
    div()
        .size(px(13.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .map(|box_| {
            if checked {
                box_.bg(colors.primary).child(list::icon(
                    "CheckIconBold",
                    11.,
                    colors.primary_foreground,
                ))
            } else {
                box_.border_1()
                    .border_color(rgb(0x767676))
                    .bg(colors.background)
            }
        })
}

fn config_row(
    title: &'static str,
    value: SharedString,
    control: Option<AnyElement>,
    colors: Theme,
) -> AnyElement {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_3()
        .child(
            div()
                .min_w_0()
                .child(
                    div()
                        .text_xs()
                        .line_height(px(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .mt_1()
                        .truncate()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(colors.muted_foreground)
                        .child(value),
                ),
        )
        .children(control)
        .into_any_element()
}

fn arrow(colors: Theme) -> AnyElement {
    div()
        .h(px(24.))
        .pl_6()
        .flex()
        .items_center()
        .child(
            svg()
                .path(assets::path("ArrowRight02Icon"))
                .size(px(13.))
                .with_transformation(Transformation::rotate(percentage(0.25)))
                .text_color(colors.muted_foreground),
        )
        .into_any_element()
}

#[derive(Clone, Copy, PartialEq)]
enum WorkflowCardKind {
    Trigger,
    Ai,
    Action,
}

fn step_colors(kind: WorkflowCardKind, window: &Window) -> (Hsla, Hsla) {
    let dark = is_dark(window);
    let (light_bg, light_fg, dark_bg, dark_fg) = match kind {
        WorkflowCardKind::Ai => (0xede9fe, 0x6d28d9, 0x2e1065, 0xc4b5fd),
        WorkflowCardKind::Trigger => (0xfef3c7, 0xb45309, 0x451a03, 0xfcd34d),
        WorkflowCardKind::Action => (0xdbeafe, 0x1d4ed8, 0x172554, 0x93c5fd),
    };
    if dark {
        (rgb(dark_bg).into(), rgb(dark_fg).into())
    } else {
        (rgb(light_bg).into(), rgb(light_fg).into())
    }
}

/// `WorkflowCard`: the trigger ("1") or an action ("+") with its badges and content.
fn workflow_card(
    kind: WorkflowCardKind,
    title: SharedString,
    label: &'static str,
    ready: bool,
    remove: Option<AnyElement>,
    content: AnyElement,
    colors: Theme,
) -> AnyElement {
    let (bg, fg) = match kind {
        WorkflowCardKind::Trigger => (rgb(0xfef3c7), rgb(0xb45309)),
        _ => (rgb(0xdbeafe), rgb(0x1d4ed8)),
    };
    div()
        .p_4()
        .flex()
        .items_start()
        .gap_3()
        .border_1()
        .border_color(colors.border)
        .bg(colors.card)
        .child(
            div()
                .mt(px(2.))
                .size(px(28.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(bg)
                .text_color(fg)
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .child(if kind == WorkflowCardKind::Trigger {
                    "1"
                } else {
                    "+"
                }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .line_height(px(20.))
                                .font_weight(crate::ui::theme::MEDIUM)
                                .child(title),
                        )
                        .child(badge(label, true, colors))
                        .when(!ready, |view| {
                            view.child(badge("Needs setup", true, colors))
                        })
                        .children(remove),
                )
                .child(div().mt_3().child(content)),
        )
        .into_any_element()
}

/// `AutomationLastRunLine`.
fn last_run_line(run: Option<&super::RunRecord>, colors: Theme) -> Option<AnyElement> {
    let run = run?;
    let when = relative(&run.at).unwrap_or_default();
    Some(
        div()
            .mt_3()
            .truncate()
            .text_xs()
            .line_height(px(16.))
            .text_color(if run.success {
                colors.muted_foreground
            } else {
                colors.destructive
            })
            .child(if run.success {
                format!("Last run {when}: {}", run.detail)
            } else {
                format!("Last run failed {when}: {}", run.detail)
            })
            .into_any_element(),
    )
}

/// `formatDistanceToNow(…, { addSuffix: true })`.
fn relative(at: &str) -> Option<String> {
    let at = DateTime::parse_from_rfc3339(at).ok()?.with_timezone(&Utc);
    Some(crate::chat::state::distance_to_now(at, Utc::now()))
}
