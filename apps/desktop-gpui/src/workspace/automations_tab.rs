//! The Automations tab: `sidebar/automations.tsx` (`AutomationsNav`) on the
//! left, `settings/automations/index.tsx` (`AutomationsContent`, the starter
//! and workflow details, `WorkflowBuilder`) in the middle, and the
//! automations-scoped chat panel on the right (`chat-panels.tsx` forces the
//! right panel open for this tab).

use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, Div, Entity, Focusable as _, MouseButton, Pixels, Point,
    SharedString, Window, div, img, prelude::*, px,
};

use super::Workspace;
use super::menu::{Align, Entry, MenuSpec, Select, Trailing};
use super::settings::{SelectOption, SelectSpec};
use super::toast::FlashVariant;
use crate::automations::{
    self, DRAFT_TEMPLATE_KEY, MARKDOWN_EXPORT_DIRECTORY_KEY, Starter, StarterId, Step, StepKind,
    StepType, Trigger, WORKFLOWS_KEY, Workflow,
};
use crate::db::ChatGroup;
use crate::text_input::{TextInput, TextInputEvent};
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

/// A press handler on the workspace.
type Action = Rc<dyn Fn(&mut Workspace, &mut Context<Workspace>)>;
type WindowAction = Rc<dyn Fn(&mut Workspace, &mut Window, &mut Context<Workspace>)>;

/// `AutomationSelection`
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Selection {
    Starter(StarterId),
    Chat(String),
    Draft(String),
    Workflow(String),
}

/// The sidebar row a right-click opened the menu for.
#[derive(Clone, Debug)]
enum ContextTarget {
    Starter(StarterId),
    Draft(String),
    Workflow(String),
    Chat(String),
}

pub(crate) struct AutomationsState {
    search: Entity<TextInput>,
    /// `useAutomationSelection().selection`
    selection: Option<Selection>,
    /// `draftIds`: unsaved drafts started from the sidebar.
    draft_ids: Vec<String>,
    chat_groups: Vec<ChatGroup>,
    /// `StarterAutomationDetails`'s `showPreview`.
    show_preview: bool,
    actions_open: bool,
    context_menu: Option<(ContextTarget, Point<Pixels>)>,
}

/// `AUTOMATIONS_SURFACE_MIN_WIDTH_PX`
const SURFACE_MIN_WIDTH: f32 = 600.0;
/// `RIGHT_CHAT_PANEL_MIN_WIDTH_PX`
const RIGHT_PANEL_MIN_WIDTH: f32 = 320.0;
/// The `ResizablePanel defaultSize={30}` share of the body.
const RIGHT_PANEL_SHARE: f32 = 0.3;

/// Stable element ids for the per-step selects (`SelectSpec.id` is static).
const STEP_SELECT_IDS: [&str; 12] = [
    "automation-step-0",
    "automation-step-1",
    "automation-step-2",
    "automation-step-3",
    "automation-step-4",
    "automation-step-5",
    "automation-step-6",
    "automation-step-7",
    "automation-step-8",
    "automation-step-9",
    "automation-step-10",
    "automation-step-11",
];
const STEP_FOLDER_IDS: [&str; 12] = [
    "automation-step-folder-0",
    "automation-step-folder-1",
    "automation-step-folder-2",
    "automation-step-folder-3",
    "automation-step-folder-4",
    "automation-step-folder-5",
    "automation-step-folder-6",
    "automation-step-folder-7",
    "automation-step-folder-8",
    "automation-step-folder-9",
    "automation-step-folder-10",
    "automation-step-folder-11",
];
const STEP_CONNECT_IDS: [&str; 12] = [
    "automation-step-connect-0",
    "automation-step-connect-1",
    "automation-step-connect-2",
    "automation-step-connect-3",
    "automation-step-connect-4",
    "automation-step-connect-5",
    "automation-step-connect-6",
    "automation-step-connect-7",
    "automation-step-connect-8",
    "automation-step-connect-9",
    "automation-step-connect-10",
    "automation-step-connect-11",
];

impl Workspace {
    /// `openNew({ type: "automations" })`
    pub(crate) fn open_automations(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_settings(cx);
        self.close_folders(cx);
        self.close_templates(cx);
        self.close_calendar(cx);
        self.close_contacts(cx);
        if self.automations.is_none() {
            let style = self.plain_input_style();
            let search = cx.new(|cx| TextInput::new("Search automations...", style, window, cx));
            cx.subscribe(&search, |this, input, event: &TextInputEvent, cx| {
                match event {
                    TextInputEvent::Escape => input.update(cx, |input, cx| input.set_text("", cx)),
                    TextInputEvent::Changed => {}
                    _ => return,
                }
                if this.automations.is_some() {
                    cx.notify();
                }
            })
            .detach();
            self.automations = Some(AutomationsState {
                search,
                selection: None,
                draft_ids: Vec::new(),
                chat_groups: Vec::new(),
                show_preview: false,
                actions_open: false,
                context_menu: None,
            });
        }
        self.reload_automation_chats(cx);
        cx.notify();
    }

    pub(crate) fn close_automations(&mut self, cx: &mut Context<Self>) {
        if self.automations.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn automations_open(&self) -> bool {
        self.automations.is_some()
    }

    pub(super) fn close_automations_menus(&mut self) -> bool {
        let Some(state) = self.automations.as_mut() else {
            return false;
        };
        let mut closed = false;
        if state.actions_open {
            state.actions_open = false;
            closed = true;
        }
        if state.context_menu.take().is_some() {
            closed = true;
        }
        closed
    }

    pub(crate) fn reload_automation_chats(&mut self, cx: &mut Context<Self>) {
        if self.automations.is_none() {
            return;
        }
        let task = self.store.list_automation_chat_groups();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(groups)) = task.await else {
                return;
            };
            this.update(cx, |this, cx| {
                if let Some(state) = this.automations.as_mut()
                    && state.chat_groups != groups
                {
                    state.chat_groups = groups;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn automation_setting(&self, key: &str) -> Option<String> {
        let legacy = key.strip_prefix("automation_").unwrap_or(key);
        self.provider_settings
            .string_setting(key, &["automations", legacy])
    }

    fn automation_flag(&self, key: &str) -> bool {
        let legacy = key.strip_prefix("automation_").unwrap_or(key);
        self.provider_settings
            .bool_setting(key, &["automations", legacy], false)
    }

    /// `useAutomationWorkflows()`
    pub(crate) fn automation_workflows(&self) -> Vec<Workflow> {
        automations::parse_workflows(&self.automation_setting(WORKFLOWS_KEY).unwrap_or_default())
    }

    /// `saveAutomationWorkflows`
    fn save_automation_workflows(&mut self, workflows: &[Workflow], cx: &mut Context<Self>) {
        self.set_setting(
            WORKFLOWS_KEY,
            serde_json::Value::String(automations::serialize_workflows(workflows)),
            cx,
        );
    }

    /// `useSaveWorkflow` → `persist(next)`
    fn persist_workflow(&mut self, next: Workflow, cx: &mut Context<Self>) {
        let workflows = automations::upsert_workflow(&self.automation_workflows(), next);
        self.save_automation_workflows(&workflows, cx);
    }

    /// `useEffectiveAutomationSelection`: the explicit selection, else the
    /// starter stored as `automation_draft_template`.
    fn effective_automation_selection(&self) -> Option<Selection> {
        let state = self.automations.as_ref()?;
        if let Some(selection) = &state.selection {
            return Some(selection.clone());
        }
        self.automation_setting(DRAFT_TEMPLATE_KEY)
            .as_deref()
            .and_then(StarterId::parse)
            .map(Selection::Starter)
    }

    fn select_automation(&mut self, selection: Selection, cx: &mut Context<Self>) {
        if let Some(state) = self.automations.as_mut() {
            if state.selection.as_ref() != Some(&selection) {
                state.show_preview = false;
            }
            state.selection = Some(selection);
            state.actions_open = false;
            state.context_menu = None;
            cx.notify();
        }
    }

    /// `clearSelection`: only the matching selection is dropped.
    fn clear_automation_selection(&mut self, selection: &Selection, cx: &mut Context<Self>) {
        if let Some(state) = self.automations.as_mut()
            && state.selection.as_ref() == Some(selection)
        {
            state.selection = None;
            cx.notify();
        }
    }

    /// The `+` header button: `createEmptyWorkflow()` prepended and selected.
    fn create_automation_workflow(&mut self, cx: &mut Context<Self>) {
        let workflow = Workflow::new_empty();
        let id = workflow.id.clone();
        let mut workflows = self.automation_workflows();
        workflows.insert(0, workflow);
        self.save_automation_workflows(&workflows, cx);
        self.select_automation(Selection::Workflow(id), cx);
    }

    /// `useEnsuredWorkflow`: the stored workflow, or a fresh one persisted on
    /// first sight for drafts and chat automations.
    fn ensured_workflow(
        &mut self,
        id: Option<&str>,
        chat_group_id: Option<&str>,
        title: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Workflow {
        let workflows = self.automation_workflows();
        let existing = id
            .and_then(|id| workflows.iter().find(|w| w.id == id))
            .or_else(|| {
                chat_group_id.and_then(|group| {
                    workflows
                        .iter()
                        .find(|w| w.chat_group_id.as_deref() == Some(group))
                })
            })
            .cloned();
        if let Some(existing) = existing {
            return existing;
        }
        let mut fallback = Workflow::new_empty();
        if let Some(id) = id {
            fallback.id = id.to_string();
        }
        fallback.chat_group_id = chat_group_id.map(str::to_string);
        if let Some(title) = title {
            fallback.title = title.to_string();
        }
        let mut next = workflows;
        next.insert(0, fallback.clone());
        self.save_automation_workflows(&next, cx);
        fallback
    }

    /// `useDeleteWorkflow`
    fn delete_automation_workflow(&mut self, workflow_id: String, cx: &mut Context<Self>) {
        let workflows = self.automation_workflows();
        let chat_group = workflows
            .iter()
            .find(|w| w.id == workflow_id)
            .and_then(|w| w.chat_group_id.clone());
        let remaining: Vec<Workflow> = workflows
            .into_iter()
            .filter(|w| w.id != workflow_id)
            .collect();
        self.save_automation_workflows(&remaining, cx);
        if let Some(group) = chat_group {
            self.delete_automation_chat_group(group, cx);
        }
        self.clear_automation_selection(&Selection::Workflow(workflow_id), cx);
        self.flash(FlashVariant::Success, "Automation deleted", cx);
    }

    /// `useDeleteChatAutomation`
    fn delete_chat_automation(&mut self, group_id: String, cx: &mut Context<Self>) {
        let workflows = self.automation_workflows();
        let remaining: Vec<Workflow> = workflows
            .iter()
            .filter(|w| w.chat_group_id.as_deref() != Some(group_id.as_str()))
            .cloned()
            .collect();
        if remaining.len() != workflows.len() {
            self.save_automation_workflows(&remaining, cx);
        }
        self.delete_automation_chat_group(group_id.clone(), cx);
        self.clear_automation_selection(&Selection::Chat(group_id), cx);
        self.flash(FlashVariant::Success, "Automation deleted", cx);
    }

    fn delete_automation_chat_group(&mut self, group_id: String, cx: &mut Context<Self>) {
        if let Some(state) = self.automations.as_mut() {
            state.chat_groups.retain(|group| group.id != group_id);
        }
        let task = self.store.delete_chat_group(group_id);
        cx.spawn(async move |this, cx| match task.await {
            Ok(Ok(())) => {
                this.update(cx, |this, cx| this.reload_automation_chats(cx))
                    .ok();
            }
            Ok(Err(error)) => {
                this.update(cx, |this, cx| {
                    this.flash(FlashVariant::Error, error.to_string(), cx)
                })
                .ok();
            }
            Err(_) => {}
        })
        .detach();
    }

    /// `useRemoveStarterDraft`: disable the starter and forget it as the draft.
    fn remove_starter_draft(&mut self, starter: StarterId, cx: &mut Context<Self>) {
        self.set_bool_setting(starter.enabled_key(), false, cx);
        if self.automation_setting(DRAFT_TEMPLATE_KEY).as_deref() == Some(starter.as_str()) {
            self.set_setting(
                DRAFT_TEMPLATE_KEY,
                serde_json::Value::String(String::new()),
                cx,
            );
        }
        self.clear_automation_selection(&Selection::Starter(starter), cx);
        self.flash(FlashVariant::Success, "Automation removed", cx);
    }

    /// `removeDraft`
    fn remove_automation_draft(&mut self, draft_id: String, cx: &mut Context<Self>) {
        if let Some(state) = self.automations.as_mut() {
            state.draft_ids.retain(|id| id != &draft_id);
        }
        self.clear_automation_selection(&Selection::Draft(draft_id), cx);
    }

    /// `saveDraftMutation`
    fn save_starter_draft(&mut self, starter: StarterId, cx: &mut Context<Self>) {
        if !self.is_pro() {
            self.notify_pro_required(cx);
            return;
        }
        self.set_setting(
            DRAFT_TEMPLATE_KEY,
            serde_json::Value::String(starter.as_str().to_string()),
            cx,
        );
        self.flash(FlashVariant::Success, "Automation draft saved", cx);
    }

    /// `setEnabledMutation`
    fn set_starter_enabled(&mut self, starter: StarterId, enabled: bool, cx: &mut Context<Self>) {
        if enabled && !self.is_pro() {
            self.notify_pro_required(cx);
            return;
        }
        self.set_setting(
            DRAFT_TEMPLATE_KEY,
            serde_json::Value::String(starter.as_str().to_string()),
            cx,
        );
        self.set_bool_setting(starter.enabled_key(), enabled, cx);
        self.flash(
            FlashVariant::Success,
            if enabled {
                "Automation enabled"
            } else {
                "Automation disabled"
            },
            cx,
        );
    }

    /// `handleEnable` on a custom workflow.
    fn set_workflow_enabled(&mut self, workflow: &Workflow, enabled: bool, cx: &mut Context<Self>) {
        if enabled && !self.is_pro() {
            self.notify_pro_required(cx);
            return;
        }
        let mut next = workflow.clone();
        next.enabled = enabled;
        self.persist_workflow(next, cx);
    }

    /// `chooseFolderMutation`: the native directory dialog, then the starter
    /// setting or the step's `directory`.
    fn choose_export_folder(
        &mut self,
        target: FolderTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let picker = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picker.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let directory = path.to_string_lossy().to_string();
            if directory.is_empty() {
                return;
            }
            this.update(cx, |this, cx| match &target {
                FolderTarget::Starter => this.set_setting(
                    MARKDOWN_EXPORT_DIRECTORY_KEY,
                    serde_json::Value::String(directory),
                    cx,
                ),
                FolderTarget::Step {
                    workflow_id,
                    step_id,
                } => {
                    if let Some(workflow) = this
                        .automation_workflows()
                        .into_iter()
                        .find(|w| &w.id == workflow_id)
                    {
                        let mut next = workflow;
                        if let Some(step) = next.steps.iter_mut().find(|s| &s.id == step_id) {
                            step.directory = directory;
                        }
                        this.persist_workflow(next, cx);
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    /// `IntegrationGate`'s connect button while signed out:
    /// `openIntegrationUrl` needs a session and reports the failure.
    fn connect_integration(&mut self, cx: &mut Context<Self>) {
        self.flash(
            FlashVariant::Error,
            "No authentication session is available",
            cx,
        );
    }

    /// `billing.isPro`: the shell has no signed-in billing state yet.
    fn is_pro(&self) -> bool {
        false
    }

    /// `billing.isReady`: the claims query never settles without a session,
    /// so the save / enable buttons stay disabled while signed out.
    fn billing_ready(&self) -> bool {
        false
    }

    // ------------------------------------------------------------------
    // Sidebar
    // ------------------------------------------------------------------

    /// `AutomationsNav`
    pub(super) fn render_automations_sidebar(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let Some(state) = self.automations.as_ref() else {
            return div();
        };
        let query = state.search.read(cx).text().trim().to_lowercase();
        let searching = !query.is_empty();
        let selection = self.effective_automation_selection();
        let workflows = self.automation_workflows();
        let starters = automations::starters();

        let filtered_starters: Vec<&Starter> = starters
            .iter()
            .filter(|starter| {
                query.is_empty()
                    || starter.title.to_lowercase().contains(&query)
                    || starter.description.to_lowercase().contains(&query)
            })
            .collect();
        let draft_title = "Untitled automation";
        let filtered_drafts: Vec<&String> =
            if searching && !draft_title.to_lowercase().contains(&query) {
                Vec::new()
            } else {
                state.draft_ids.iter().collect()
            };
        let filtered_workflows: Vec<&Workflow> = workflows
            .iter()
            .filter(|workflow| query.is_empty() || workflow.title.to_lowercase().contains(&query))
            .collect();
        let chat_ids_with_workflow: Vec<&str> = workflows
            .iter()
            .filter_map(|workflow| workflow.chat_group_id.as_deref())
            .collect();
        let filtered_chats: Vec<&ChatGroup> = state
            .chat_groups
            .iter()
            .filter(|group| !chat_ids_with_workflow.contains(&group.id.as_str()))
            .filter(|group| query.is_empty() || group.title.to_lowercase().contains(&query))
            .collect();
        let is_empty = filtered_starters.is_empty()
            && filtered_drafts.is_empty()
            && filtered_workflows.is_empty()
            && filtered_chats.is_empty();

        let search_input = state.search.clone();
        let focus_input = search_input.clone();
        // The sidebar minus the row's `px-3`, the 16px glyph, and the `gap-2`.
        let title_width = px(self.custom_sidebar_width() - 52.0);
        let now = chrono::Utc::now();

        let row = |id: SharedString, is_selected: bool| {
            div()
                .id(id)
                .w_full()
                .rounded_lg()
                .px_3()
                .py_2()
                .tw_text_sm()
                .cursor_pointer()
                .when(is_selected, |row| row.bg(theme.accent))
                .when(!is_selected, |row| {
                    row.hover(move |style| style.bg(alpha(theme.accent, 0.5)))
                })
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        };
        let heading = |text: &'static str| {
            div()
                .px_3()
                .pt_1()
                .pb_1()
                .tw_text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.muted_foreground)
                .child(text)
        };
        // The two-line rows: the medium title over a `mt-0.5 text-xs` line.
        let two_line = |title: String, subtitle: Option<String>| {
            div()
                .min_w_0()
                .flex_grow()
                .flex()
                .flex_col()
                .child(
                    div()
                        .w(title_width)
                        .truncate()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.foreground)
                        .child(SharedString::from(title)),
                )
                .when_some(subtitle, |column, subtitle| {
                    column.child(
                        div()
                            .w(title_width)
                            .mt(px(2.0))
                            .truncate()
                            .tw_text_xs()
                            .text_color(theme.muted_foreground)
                            .child(SharedString::from(subtitle)),
                    )
                })
        };
        let context_menu = |target: ContextTarget, selection: Selection| {
            cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                cx.stop_propagation();
                this.select_automation(selection.clone(), cx);
                if let Some(state) = this.automations.as_mut() {
                    state.context_menu = Some((target.clone(), event.position));
                    cx.notify();
                }
            })
        };

        let mut list = div().pt_1().w_full();
        if !filtered_starters.is_empty() {
            let mut section = div().pb_2().child(heading("Get started"));
            for starter in &filtered_starters {
                let id = starter.id;
                let is_selected = selection == Some(Selection::Starter(id));
                section = section.child(
                    row(
                        format!("automation-starter-{}", id.as_str()).into(),
                        is_selected,
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.select_automation(Selection::Starter(id), cx);
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        context_menu(ContextTarget::Starter(id), Selection::Starter(id)),
                    )
                    .child(
                        div()
                            .flex()
                            .min_w_0()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .size(px(16.0))
                                    .flex_shrink_0()
                                    .items_center()
                                    .justify_center()
                                    .child(starter_icon(id, 16.0, theme.dark)),
                            )
                            .child(two_line(starter.title.to_string(), None)),
                    ),
                );
            }
            list = list.child(section);
        }
        if !filtered_drafts.is_empty()
            || !filtered_workflows.is_empty()
            || !filtered_chats.is_empty()
        {
            let mut section = div().child(heading("My automations"));
            for draft_id in &filtered_drafts {
                let id = (*draft_id).clone();
                let is_selected = selection == Some(Selection::Draft(id.clone()));
                let select_id = id.clone();
                section = section.child(
                    row(format!("automation-draft-{id}").into(), is_selected)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.select_automation(Selection::Draft(select_id.clone()), cx);
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            context_menu(ContextTarget::Draft(id.clone()), Selection::Draft(id)),
                        )
                        .child(
                            div()
                                .flex()
                                .min_w_0()
                                .items_center()
                                .gap_2()
                                .child(icon("lightning", px(16.0), gpui::rgb(0x8e51ff)))
                                .child(two_line(
                                    draft_title.to_string(),
                                    Some("Draft".to_string()),
                                )),
                        ),
                );
            }
            for workflow in &filtered_workflows {
                let id = workflow.id.clone();
                let is_selected = selection == Some(Selection::Workflow(id.clone()));
                let select_id = id.clone();
                section = section.child(
                    row(format!("automation-workflow-{id}").into(), is_selected)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.select_automation(Selection::Workflow(select_id.clone()), cx);
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            context_menu(
                                ContextTarget::Workflow(id.clone()),
                                Selection::Workflow(id),
                            ),
                        )
                        .child(
                            div()
                                .flex()
                                .min_w_0()
                                .items_center()
                                .gap_2()
                                .child(icon("lightning", px(16.0), gpui::rgb(0x8e51ff)))
                                .child(two_line(
                                    workflow.display_title(),
                                    Some(
                                        if workflow.enabled { "Enabled" } else { "Draft" }
                                            .to_string(),
                                    ),
                                )),
                        ),
                );
            }
            for group in &filtered_chats {
                let id = group.id.clone();
                let is_selected = selection == Some(Selection::Chat(id.clone()));
                let select_id = id.clone();
                let created = group
                    .created_at
                    .parse::<chrono::DateTime<chrono::Utc>>()
                    .ok()
                    .map(|at| automations::format_distance_to_now(at, now));
                section = section.child(
                    row(format!("automation-chat-{id}").into(), is_selected)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.select_automation(Selection::Chat(select_id.clone()), cx);
                        }))
                        .on_mouse_down(
                            MouseButton::Right,
                            context_menu(ContextTarget::Chat(id.clone()), Selection::Chat(id)),
                        )
                        .child(
                            div()
                                .flex()
                                .min_w_0()
                                .items_center()
                                .gap_2()
                                .child(icon("lightning", px(16.0), gpui::rgb(0x8e51ff)))
                                .child(two_line(group.title.clone(), created)),
                        ),
                );
            }
            list = list.child(section);
        }

        div()
            .flex()
            .flex_col()
            .h_full()
            .w(px(self.custom_sidebar_width()))
            .flex_shrink_0()
            .pr_1()
            .pb_2()
            .overflow_hidden()
            .child(
                // `CustomSidebarHeader`: back on the left, `+` on the right.
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
                                self.tracked_chrome_button("automations-back", cx)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.close_automations(cx);
                                    }))
                                    .child(icon(
                                        "arrow-left",
                                        px(16.0),
                                        self.chrome_icon_color("automations-back"),
                                    )),
                            )
                            .child(div().flex_1())
                            .child(
                                self.tracked_chrome_button("automations-new", cx)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.create_automation_workflow(cx);
                                    }))
                                    .child(icon(
                                        "plus",
                                        px(16.0),
                                        self.chrome_icon_color("automations-new"),
                                    )),
                            ),
                    ),
            )
            .child(
                div().pb_2().child(
                    div()
                        .id("automations-search")
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
                                    .id("automations-search-clear")
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
                    .id("automations-list")
                    .min_h_0()
                    .w_full()
                    .flex_1()
                    .overflow_y_scroll()
                    .child(if is_empty {
                        div()
                            .px_3()
                            .py_8()
                            .flex()
                            .flex_col()
                            .items_center()
                            .text_color(theme.muted_foreground)
                            .child(div().mb_2().child(icon(
                                "lightning",
                                px(32.0),
                                alpha(theme.muted_foreground, 0.7),
                            )))
                            .child(div().tw_text_sm().child(if searching {
                                "No automations found"
                            } else {
                                "No automations yet"
                            }))
                            .into_any_element()
                    } else {
                        list.into_any_element()
                    }),
            )
    }

    /// The sidebar rows' `useNativeContextMenu`: Edit, then Remove / Delete.
    pub(super) fn render_automations_context_menu(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let state = self.automations.as_ref()?;
        let (target, position) = state.context_menu.clone()?;
        let (edit, remove_label, remove): (Selection, &'static str, Select) = match target {
            ContextTarget::Starter(id) => (
                Selection::Starter(id),
                "Remove",
                Box::new(
                    move |this: &mut Workspace, _: &mut Window, cx: &mut Context<Workspace>| {
                        this.remove_starter_draft(id, cx);
                    },
                ),
            ),
            ContextTarget::Draft(id) => {
                let draft = id.clone();
                (
                    Selection::Draft(id),
                    "Delete",
                    Box::new(
                        move |this: &mut Workspace, _: &mut Window, cx: &mut Context<Workspace>| {
                            this.remove_automation_draft(draft.clone(), cx);
                        },
                    ),
                )
            }
            ContextTarget::Workflow(id) => {
                let workflow = id.clone();
                (
                    Selection::Workflow(id),
                    "Delete",
                    Box::new(
                        move |this: &mut Workspace, _: &mut Window, cx: &mut Context<Workspace>| {
                            this.delete_automation_workflow(workflow.clone(), cx);
                        },
                    ),
                )
            }
            ContextTarget::Chat(id) => {
                let group = id.clone();
                (
                    Selection::Chat(id),
                    "Delete",
                    Box::new(
                        move |this: &mut Workspace, _: &mut Window, cx: &mut Context<Workspace>| {
                            this.delete_chat_automation(group.clone(), cx);
                        },
                    ),
                )
            }
        };
        let spec = MenuSpec {
            id: "automation-row-menu",
            width: 160.0,
            entries: vec![
                Entry::Item {
                    icon: None,
                    dim_icon: false,
                    label: "Edit".into(),
                    trailing: Trailing::None,
                    destructive: false,
                    on_select: Some(Box::new(
                        move |this: &mut Workspace, _: &mut Window, cx: &mut Context<Workspace>| {
                            this.select_automation(edit.clone(), cx);
                        },
                    ) as Select),
                    submenu: None,
                },
                Entry::Separator,
                Entry::Item {
                    icon: None,
                    dim_icon: false,
                    label: remove_label.into(),
                    trailing: Trailing::None,
                    destructive: false,
                    on_select: Some(remove),
                    submenu: None,
                },
            ],
            open_sub: None,
            on_hover_sub: |_, _, _| {},
            on_close: |this, cx| {
                if let Some(state) = this.automations.as_mut() {
                    state.context_menu = None;
                    cx.notify();
                }
            },
        };
        Some(self.render_app_menu(spec, position, Align::Start, window, cx))
    }

    // ------------------------------------------------------------------
    // Main surface
    // ------------------------------------------------------------------

    /// `TabContentAutomations` beside the forced-open right chat panel.
    pub(super) fn render_automations_main(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let selection = self.effective_automation_selection();
        let content = match selection {
            Some(Selection::Starter(id)) => self.render_starter_details(id, window, cx),
            Some(Selection::Chat(group_id)) => {
                let group = self
                    .automations
                    .as_ref()
                    .and_then(|state| state.chat_groups.iter().find(|g| g.id == group_id).cloned());
                let title = group
                    .as_ref()
                    .map(|g| g.title.trim().to_string())
                    .filter(|t| !t.is_empty());
                let workflow = self.ensured_workflow(None, Some(&group_id), title.as_deref(), cx);
                let created = group
                    .as_ref()
                    .and_then(|g| g.created_at.parse::<chrono::DateTime<chrono::Utc>>().ok())
                    .map(|at| {
                        format!(
                            "Created {}",
                            automations::format_distance_to_now(at, chrono::Utc::now())
                        )
                    });
                let heading = title.unwrap_or_else(|| workflow.display_title());
                let on_delete: Action =
                    Rc::new(move |this, cx| this.delete_chat_automation(group_id.clone(), cx));
                self.render_workflow_details(&workflow, heading, created, on_delete, window, cx)
            }
            Some(Selection::Draft(draft_id)) => {
                let workflow = self.ensured_workflow(Some(&draft_id), None, None, cx);
                let on_delete: Action =
                    Rc::new(move |this, cx| this.remove_automation_draft(draft_id.clone(), cx));
                self.render_workflow_details(
                    &workflow,
                    workflow.display_title(),
                    Some(
                        "Add a trigger and actions. Chat on the right can help you refine the workflow."
                            .to_string(),
                    ),
                    on_delete,
                    window,
                    cx,
                )
            }
            Some(Selection::Workflow(workflow_id)) => {
                let workflow = self.ensured_workflow(Some(&workflow_id), None, None, cx);
                let on_delete: Action = Rc::new(move |this, cx| {
                    this.delete_automation_workflow(workflow_id.clone(), cx)
                });
                self.render_workflow_details(
                    &workflow,
                    workflow.display_title(),
                    Some(
                        "Add a trigger and actions. Chat on the right can help you refine the workflow."
                            .to_string(),
                    ),
                    on_delete,
                    window,
                    cx,
                )
            }
            None => self.render_automations_overview(window),
        };

        // `ResizablePanelGroup`: the body at 70% (at least the surface min
        // width) and the right chat panel at 30% (at least its min width).
        let total = f32::from(window.viewport_size().width)
            - if self.sidebar_expanded {
                self.custom_sidebar_width() + 4.0
            } else {
                4.0
            };
        // The body's `min-width` wins over the panel's when both cannot fit.
        let right = (total * RIGHT_PANEL_SHARE)
            .max(RIGHT_PANEL_MIN_WIDTH)
            .min(total - SURFACE_MIN_WIDTH)
            .max(0.0);
        div()
            .flex()
            .size_full()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .flex_col()
                    .overflow_hidden()
                    .bg(theme.card)
                    .child(content),
            )
            .child(
                // `[data-chat-right-panel]`: `border-x bg-card rounded-tr-xl`.
                div()
                    .flex()
                    .flex_col()
                    .w(px(right))
                    .flex_shrink_0()
                    .min_h_0()
                    .border_l_1()
                    .border_r_1()
                    .border_color(theme.border)
                    .bg(theme.card)
                    .rounded_tr(px(12.0))
                    .overflow_hidden()
                    .child(self.render_chat_body(cx)),
            )
            .into_any_element()
    }

    /// `AutomationsOverview`
    fn render_automations_overview(&self, window: &Window) -> AnyElement {
        let theme = self.theme;
        let mut style = window.text_style();
        style.font_size = px(12.0).into();
        style.color = theme.muted_foreground.into();
        if let Some(family) = &self.font_family {
            style.font_family = family.clone();
        }
        let hint =
            "Choose a starter from the sidebar, or create a workflow and add steps like Zapier.";
        let run = style.to_run(hint.len());
        div()
            .id("automations-overview")
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .px_6()
            .pt_3()
            .pb_8()
            .child(
                div()
                    .mx_auto()
                    .flex()
                    .w_full()
                    .max_w(px(1024.0))
                    .flex_col()
                    .gap_6()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .font_family(super::settings::hand_font_family())
                                    .text_size(px(30.0))
                                    .line_height(px(37.0))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.foreground)
                                    .child("Automations"),
                            )
                            .child(
                                div()
                                    .max_w(px(672.0))
                                    .tw_text_sm()
                                    .line_height(px(22.75))
                                    .text_color(theme.muted_foreground)
                                    .child("Automate what happens before, during, or after meetings based on the conditions you choose."),
                            ),
                    )
                    .child(
                        // `min-h-56 rounded-2xl border border-dashed bg-muted/20 px-6 py-10`
                        div()
                            .flex()
                            .min_h(px(224.0))
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .rounded_2xl()
                            .border_1()
                            .border_dashed()
                            .border_color(theme.border)
                            .bg(alpha(theme.muted, 0.2))
                            .px_6()
                            .py_10()
                            .child(
                                div()
                                    .flex()
                                    .size(px(44.0))
                                    .items_center()
                                    .justify_center()
                                    .rounded_2xl()
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.background)
                                    .child(icon("lightning", px(20.0), theme.muted_foreground)),
                            )
                            .child(
                                div()
                                    .mt_4()
                                    .tw_text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.foreground)
                                    .child("No automation draft yet"),
                            )
                            .child(
                                div()
                                    .mt_1()
                                    .max_w(px(384.0))
                                    .tw_text_xs()
                                    .line_height(px(19.5))
                                    .text_color(theme.muted_foreground)
                                    .child(
                                        crate::prose_text::ProseText::new(
                                            hint,
                                            vec![run],
                                            px(12.0),
                                            px(19.5),
                                        )
                                        .centered()
                                        .pretty()
                                        .max_width(px(384.0)),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// `AutomationDetailsLayout`: the 48px header, then the scrolling
    /// `px-6 pt-3 pb-8` column capped at `max-w-5xl`.
    fn render_automation_details_layout(
        &self,
        glyph: AnyElement,
        title: String,
        description: Option<String>,
        actions: AnyElement,
        body: AnyElement,
    ) -> AnyElement {
        let theme = self.theme;
        div()
            .flex()
            .h_full()
            .min_h_0()
            .flex_1()
            .flex_col()
            .child(
                div()
                    .flex()
                    .h(px(48.0))
                    .flex_shrink_0()
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
                                div()
                                    .flex()
                                    .size(px(28.0))
                                    .flex_shrink_0()
                                    .items_center()
                                    .justify_center()
                                    .child(glyph),
                            )
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .tw_text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.foreground)
                                    .child(SharedString::from(title)),
                            ),
                    )
                    .child(actions),
            )
            .child(
                div()
                    .id("automation-details-scroll")
                    .min_h_0()
                    .w_full()
                    .flex_1()
                    .overflow_y_scroll()
                    .px_6()
                    .pt_3()
                    .pb_8()
                    .child(
                        div()
                            .mx_auto()
                            .flex()
                            .w_full()
                            .max_w(px(1024.0))
                            .flex_col()
                            .gap_6()
                            .when_some(description, |column, description| {
                                column.child(
                                    div()
                                        .max_w(px(672.0))
                                        .tw_text_sm()
                                        .line_height(px(22.75))
                                        .text_color(theme.muted_foreground)
                                        .child(SharedString::from(description)),
                                )
                            })
                            .child(body),
                    ),
            )
            .into_any_element()
    }

    /// `AutomationActionsMenu`: the `...` ghost button with one item.
    fn render_automation_actions_menu(
        &self,
        label: &'static str,
        on_action: Action,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let open = self
            .automations
            .as_ref()
            .is_some_and(|state| state.actions_open);
        let menu = MenuSpec {
            id: "automation-actions-menu",
            width: 192.0,
            entries: vec![Entry::Item {
                icon: None,
                dim_icon: false,
                label: label.into(),
                trailing: Trailing::None,
                destructive: false,
                on_select: Some(Box::new(
                    move |this: &mut Workspace, _: &mut Window, cx: &mut Context<Workspace>| {
                        on_action(this, cx);
                    },
                ) as Select),
                submenu: None,
            }],
            open_sub: None,
            on_hover_sub: |_, _, _| {},
            on_close: |this, cx| {
                if let Some(state) = this.automations.as_mut() {
                    state.actions_open = false;
                    cx.notify();
                }
            },
        };
        div()
            .relative()
            .flex()
            .flex_shrink_0()
            .items_center()
            .child(
                div()
                    .id("automation-actions")
                    .flex()
                    .size(px(28.0))
                    .items_center()
                    .justify_center()
                    .rounded(px(8.0))
                    .cursor_pointer()
                    .hover(move |style| style.bg(theme.accent))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(state) = this.automations.as_mut() {
                            state.actions_open = !state.actions_open;
                            cx.notify();
                        }
                    }))
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
                anchor.child(
                    div()
                        .absolute()
                        .top(px(32.0))
                        .right_0()
                        .child(self.render_menu_inline(menu, Align::End, cx)),
                )
            })
            .into_any_element()
    }

    /// `Badge variant="outline"` (`size="sm"` drops to `px-2`): `useSquircleRef`
    /// paints the 1px border inside the box, so it takes no layout space.
    fn outline_badge(&self, text: &'static str, small: bool) -> AnyElement {
        let theme = self.theme;
        div()
            .relative()
            .flex()
            .items_center()
            .px(px(if small { 8.0 } else { 10.0 }))
            .py(px(2.0))
            .tw_text_xs()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(theme.foreground)
            .child(crate::squircle::squircle(
                crate::squircle::CONTROL_RADIUS,
                None,
                Some((1.0, theme.border)),
            ))
            .child(text)
            .into_any_element()
    }

    /// `Button size="sm"` (28px, `px-2 text-xs gap-2 rounded-full`), outline
    /// or default variant, optionally disabled.
    fn small_button(&self, button: SmallButton, cx: &Context<Self>) -> AnyElement {
        let SmallButton {
            id,
            outline,
            glyph,
            label,
            disabled,
            on_click,
        } = button;
        let theme = self.theme;
        let hovered = self.hovered == Some(id) && !disabled;
        let (bg, fg, border) = if outline {
            (
                if hovered {
                    theme.accent
                } else {
                    theme.background
                },
                theme.foreground,
                Some(theme.border),
            )
        } else {
            (
                if hovered {
                    alpha(theme.primary, 0.9)
                } else {
                    theme.primary
                },
                theme.primary_foreground,
                None,
            )
        };
        div()
            .id(id)
            .flex()
            .h(px(28.0))
            .flex_shrink_0()
            .items_center()
            .gap_2()
            .px_2()
            .rounded(px(8.0))
            .bg(bg)
            .when_some(border, |button, border| {
                button.border_1().border_color(border)
            })
            .shadow_xs()
            .tw_text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(fg)
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
            .when_some(glyph, |button, glyph| {
                button.child(icon(glyph, px(14.0), fg))
            })
            .child(label)
            .into_any_element()
    }

    /// `ConfigRow`: the `text-xs font-semibold` title over the muted value,
    /// with the control on the right.
    fn config_row(
        &self,
        title: &'static str,
        value: String,
        control: Option<AnyElement>,
    ) -> AnyElement {
        let theme = self.theme;
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                div()
                    .min_w_0()
                    .child(
                        div()
                            .tw_text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .child(title),
                    )
                    .child(
                        div()
                            .mt_1()
                            .truncate()
                            .tw_text_xs()
                            .text_color(theme.muted_foreground)
                            .child(SharedString::from(value)),
                    ),
            )
            .children(control)
            .into_any_element()
    }

    /// `MarkdownExportConfig`
    fn render_markdown_export_config(
        &self,
        id: &'static str,
        directory: &str,
        target: FolderTarget,
        cx: &Context<Self>,
    ) -> AnyElement {
        let directory = directory.trim().to_string();
        let value = if directory.is_empty() {
            "No folder selected yet.".to_string()
        } else {
            directory
        };
        self.config_row(
            "Export folder",
            value,
            Some(self.small_button(
                SmallButton {
                    id,
                    outline: true,
                    glyph: Some("folder-open"),
                    label: "Choose folder",
                    disabled: false,
                    on_click: Some(Rc::new(move |this, window, cx| {
                        this.choose_export_folder(target.clone(), window, cx)
                    })),
                },
                cx,
            )),
        )
    }

    /// `SlackRecapConfig` / `LinearIssuesConfig` / `NotionUpdateConfig` behind
    /// `IntegrationGate`: signed out there is no connection, so the row shows
    /// the connect button.
    fn render_integration_config(
        &self,
        id: &'static str,
        kind: StepType,
        target: Option<&automations::TargetRef>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let (title, empty, connect) = match kind {
            StepType::SlackRecap => ("Slack channel", "No channel selected yet.", "Connect Slack"),
            StepType::LinearIssues => ("Linear team", "No team selected yet.", "Connect Linear"),
            StepType::NotionUpdate => ("Notion page", "No page selected yet.", "Connect Notion"),
            StepType::MarkdownExport => unreachable!("markdown export has no integration"),
        };
        let value = match (kind, target) {
            (StepType::SlackRecap, Some(target)) => format!("#{}", target.name),
            (_, Some(target)) => target.name.clone(),
            (_, None) => empty.to_string(),
        };
        let connect_button = self.small_button(
            SmallButton {
                id,
                outline: true,
                glyph: None,
                label: connect,
                disabled: false,
                on_click: Some(Rc::new(|this, _, cx| this.connect_integration(cx))),
            },
            cx,
        );
        if kind == StepType::NotionUpdate {
            // The Notion row stacks the gate under the row (`flex-col gap-3`).
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(self.config_row(title, value, None))
                .child(div().flex().child(connect_button))
                .into_any_element()
        } else {
            self.config_row(title, value, Some(connect_button))
        }
    }

    /// `AutomationLastRunLine`
    fn render_last_run_line(&self, run: Option<&automations::RunRecord>) -> Option<AnyElement> {
        let theme = self.theme;
        let run = run?;
        Some(
            div()
                .mt_3()
                .truncate()
                .tw_text_xs()
                .text_color(match run.status {
                    automations::RunStatus::Error => theme.destructive,
                    automations::RunStatus::Success => theme.muted_foreground,
                })
                .child(SharedString::from(run.line(chrono::Utc::now())))
                .into_any_element(),
        )
    }

    /// The `size-7 rounded-full text-xs font-semibold` step number / sparkle.
    fn step_marker(&self, kind: StepKind, index: usize) -> AnyElement {
        let dark = self.theme.dark;
        let (bg, fg) = match (kind, dark) {
            (StepKind::Ai, false) => (gpui::rgb(0xede9fe), gpui::rgb(0x6d28d9)),
            (StepKind::Ai, true) => (gpui::rgb(0x2e1065), gpui::rgb(0xc4b5fd)),
            (StepKind::Trigger, false) => (gpui::rgb(0xfef3c7), gpui::rgb(0xb45309)),
            (StepKind::Trigger, true) => (gpui::rgb(0x451a03), gpui::rgb(0xfcd34d)),
            (StepKind::Action, false) => (gpui::rgb(0xdbeafe), gpui::rgb(0x1d4ed8)),
            (StepKind::Action, true) => (gpui::rgb(0x172554), gpui::rgb(0x93c5fd)),
        };
        div()
            .mt(px(2.0))
            .flex()
            .size(px(28.0))
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .rounded(px(8.0))
            .bg(bg)
            .tw_text_xs()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(fg)
            .child(match kind {
                StepKind::Ai => icon("sparkle", px(13.0), fg).into_any_element(),
                _ => SharedString::from(format!("{}", index + 1)).into_any_element(),
            })
            .into_any_element()
    }

    /// The `rotate-90` arrow between cards (`h-6 pl-6`).
    fn step_connector(&self) -> AnyElement {
        div()
            .flex()
            .h(px(24.0))
            .items_center()
            .pl_6()
            .child(icon("arrow-down", px(13.0), self.theme.muted_foreground))
            .into_any_element()
    }

    /// `StarterAutomationDetails`
    fn render_starter_details(
        &mut self,
        id: StarterId,
        _window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let starters = automations::starters();
        let Some(starter) = starters.iter().find(|starter| starter.id == id) else {
            return div().into_any_element();
        };
        let is_enabled = self.automation_flag(id.enabled_key());
        let target_raw = self.automation_setting(id.target_key()).unwrap_or_default();
        let is_ready = id.is_ready(&target_raw);
        let show_preview = self
            .automations
            .as_ref()
            .is_some_and(|state| state.show_preview);
        let is_pro = self.is_pro();
        let last_run = self
            .automation_setting(id.last_run_key())
            .and_then(|raw| automations::parse_run_record(&raw));

        let mut steps = div().flex().flex_col().gap_2().p_5();
        for (index, step) in starter.steps.iter().enumerate() {
            steps = steps.child(
                div()
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap_3()
                            .rounded_xl()
                            .border_1()
                            .border_color(theme.border)
                            .bg(theme.card)
                            .p_4()
                            .child(self.step_marker(step.kind, index))
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .tw_text_sm()
                                                    .font_weight(gpui::FontWeight::MEDIUM)
                                                    .text_color(theme.foreground)
                                                    .child(step.title),
                                            )
                                            .child(self.outline_badge(
                                                match step.kind {
                                                    StepKind::Ai => "AI step",
                                                    StepKind::Trigger => "Trigger",
                                                    StepKind::Action => "Action",
                                                },
                                                true,
                                            )),
                                    )
                                    .child(
                                        div()
                                            .mt_1()
                                            .tw_text_xs()
                                            .line_height(px(19.5))
                                            .text_color(theme.muted_foreground)
                                            .child(step.detail),
                                    ),
                            ),
                    )
                    .when(index + 1 < starter.steps.len(), |column| {
                        column.child(self.step_connector())
                    }),
            );
        }

        let config = match id {
            StarterId::MarkdownExport => self.render_markdown_export_config(
                "automation-starter-folder",
                &target_raw,
                FolderTarget::Starter,
                cx,
            ),
            StarterId::SlackRecap => self.render_integration_config(
                "automation-starter-connect",
                StepType::SlackRecap,
                automations::parse_target_ref(&target_raw).as_ref(),
                cx,
            ),
            StarterId::LinearActionItems => self.render_integration_config(
                "automation-starter-connect",
                StepType::LinearIssues,
                automations::parse_target_ref(&target_raw).as_ref(),
                cx,
            ),
            StarterId::NotionProjectNotes => self.render_integration_config(
                "automation-starter-connect",
                StepType::NotionUpdate,
                automations::parse_target_ref(&target_raw).as_ref(),
                cx,
            ),
        };

        let billing_ready = self.billing_ready();
        let enable_button = if is_enabled {
            self.small_button(
                SmallButton {
                    id: "automation-starter-disable",
                    outline: true,
                    glyph: None,
                    label: "Disable",
                    disabled: !billing_ready,
                    on_click: Some(Rc::new(move |this, _, cx| {
                        this.set_starter_enabled(id, false, cx)
                    })),
                },
                cx,
            )
        } else {
            // `disabled={!billing.isReady || (billing.isPro && !isReady)}`
            self.small_button(
                SmallButton {
                    id: "automation-starter-enable",
                    outline: false,
                    glyph: Some("lightning"),
                    label: "Save & enable",
                    disabled: !billing_ready || (is_pro && !is_ready),
                    on_click: Some(Rc::new(move |this, _, cx| {
                        this.set_starter_enabled(id, true, cx)
                    })),
                },
                cx,
            )
        };

        let section = div()
            .rounded_2xl()
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .overflow_hidden()
            .child(
                // The header: title + badge, the `Steps run from top to bottom.`
                // line, and the button group.
                div()
                    .flex()
                    .flex_wrap()
                    .items_start()
                    .justify_between()
                    .gap_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .px_5()
                    .py_4()
                    .child(
                        div()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(icon("lightning", px(17.0), theme.primary))
                                    .child(
                                        div()
                                            .truncate()
                                            .tw_text_sm()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(theme.foreground)
                                            .child(starter.title),
                                    )
                                    .child(self.outline_badge(
                                        if is_enabled { "Enabled" } else { "Draft" },
                                        false,
                                    )),
                            )
                            .child(
                                div()
                                    .mt_1()
                                    .tw_text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("Steps run from top to bottom."),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_2()
                            .child(self.small_button(
                                SmallButton {
                                    id: "automation-starter-preview",
                                    outline: true,
                                    glyph: Some("eye"),
                                    label: if show_preview {
                                        "Hide preview"
                                    } else {
                                        "Preview"
                                    },
                                    disabled: false,
                                    on_click: Some(Rc::new(|this, _, cx| {
                                        if let Some(state) = this.automations.as_mut() {
                                            state.show_preview = !state.show_preview;
                                            cx.notify();
                                        }
                                    })),
                                },
                                cx,
                            ))
                            .child(self.small_button(
                                SmallButton {
                                    id: "automation-starter-test",
                                    outline: true,
                                    glyph: Some("play"),
                                    label: "Test",
                                    disabled: true,
                                    on_click: None,
                                },
                                cx,
                            ))
                            .child(self.small_button(
                                SmallButton {
                                    id: "automation-starter-save",
                                    outline: false,
                                    glyph: Some("floppy-disk"),
                                    label: "Save draft",
                                    disabled: !billing_ready,
                                    on_click: Some(Rc::new(move |this, _, cx| {
                                        this.save_starter_draft(id, cx)
                                    })),
                                },
                                cx,
                            ))
                            .child(enable_button),
                    ),
            )
            .child(steps)
            .child(
                div()
                    .border_t_1()
                    .border_color(theme.border)
                    .px_5()
                    .py_4()
                    .child(config)
                    .children(self.render_last_run_line(last_run.as_ref())),
            )
            .when(show_preview, |section| {
                section.child(
                    div()
                        .border_t_1()
                        .border_color(theme.border)
                        .bg(alpha(theme.muted, 0.35))
                        .px_5()
                        .py_4()
                        .child(
                            div()
                                .flex()
                                .items_start()
                                .gap_3()
                                .child(div().mt(px(2.0)).child(icon(
                                    "eye",
                                    px(15.0),
                                    theme.muted_foreground,
                                )))
                                .child(
                                    div()
                                        .child(
                                            div()
                                                .tw_text_xs()
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .text_color(theme.foreground)
                                                .child("Expected output"),
                                        )
                                        .child(
                                            div()
                                                .mt_1()
                                                .tw_text_xs()
                                                .line_height(px(19.5))
                                                .text_color(theme.muted_foreground)
                                                .child(starter.preview),
                                        ),
                                ),
                        ),
                )
            });

        let actions = self.render_automation_actions_menu(
            "Remove automation",
            Rc::new(move |this, cx| this.remove_starter_draft(id, cx)),
            cx,
        );
        self.render_automation_details_layout(
            starter_icon(id, 16.0, theme.dark),
            starter.title.to_string(),
            Some(starter.description.to_string()),
            actions,
            section.into_any_element(),
        )
    }

    /// `CustomWorkflowDetails`: the share button, enable / disable, the
    /// actions menu, and the `WorkflowBuilder`.
    fn render_workflow_details(
        &self,
        workflow: &Workflow,
        title: String,
        description: Option<String>,
        on_delete: Action,
        _window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let is_pro = self.is_pro();
        let billing_ready = self.billing_ready();
        let enable_button = if workflow.enabled {
            let target = workflow.clone();
            self.small_button(
                SmallButton {
                    id: "automation-workflow-disable",
                    outline: true,
                    glyph: None,
                    label: "Disable",
                    disabled: !billing_ready,
                    on_click: Some(Rc::new(move |this, _, cx| {
                        this.set_workflow_enabled(&target, false, cx)
                    })),
                },
                cx,
            )
        } else {
            let target = workflow.clone();
            self.small_button(
                SmallButton {
                    id: "automation-workflow-enable",
                    outline: false,
                    glyph: Some("lightning"),
                    label: "Save & enable",
                    disabled: !billing_ready || (is_pro && !workflow.is_ready()),
                    on_click: Some(Rc::new(move |this, _, cx| {
                        this.set_workflow_enabled(&target, true, cx)
                    })),
                },
                cx,
            )
        };
        let actions = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                // `ResourceShareButton`: the ghost `Share` button.
                div()
                    .id("automation-share")
                    .flex()
                    .h(px(28.0))
                    .items_center()
                    .gap(px(6.0))
                    .px_2()
                    .rounded(px(8.0))
                    .tw_text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.muted_foreground)
                    .cursor_pointer()
                    .hover(move |style| style.bg(theme.accent).text_color(theme.foreground))
                    .child(icon("share-network", px(16.0), theme.muted_foreground))
                    .child("Share"),
            )
            .child(enable_button)
            .child(self.render_automation_actions_menu("Delete automation", on_delete, cx))
            .into_any_element();
        let body = self.render_workflow_builder(workflow, cx);
        self.render_automation_details_layout(
            icon("lightning", px(16.0), gpui::rgb(0x8e51ff)).into_any_element(),
            title,
            description,
            actions,
            body,
        )
    }

    /// `WorkflowBuilder`
    fn render_workflow_builder(&self, workflow: &Workflow, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        let workflow_id = workflow.id.clone();

        let trigger_options: Vec<SelectOption> = [Trigger::NoteEnhanced, Trigger::MeetingCompleted]
            .into_iter()
            .map(|trigger| SelectOption {
                value: trigger.as_str().to_string(),
                label: trigger.label().to_string(),
                detail: None,
                glyph: None,
            })
            .collect();
        let trigger_workflow = workflow.clone();
        let trigger_select = self.render_select_sized(
            SelectSpec {
                id: "automation-trigger",
                current: Some(workflow.trigger.as_str().to_string()),
                placeholder: "",
                options: Rc::new(trigger_options),
                search: None,
                on_select: Rc::new(move |this, value, _, cx| {
                    let mut next = trigger_workflow.clone();
                    next.trigger = Trigger::parse(&value);
                    this.persist_workflow(next, cx);
                }),
            },
            true,
            cx,
        );

        let mut cards = div().flex().flex_col().gap_2().p_5().child(
            self.render_workflow_card(
                WorkflowCard {
                    kind: StepKind::Trigger,
                    title: "When this happens".to_string(),
                    badge: "Trigger",
                    ready: true,
                    on_remove: None,
                },
                div()
                    .max_w(px(320.0))
                    .child(trigger_select)
                    .into_any_element(),
                cx,
            ),
        );

        for (index, step) in workflow
            .steps
            .iter()
            .enumerate()
            .take(STEP_SELECT_IDS.len())
        {
            let step_options: Vec<SelectOption> = StepType::ALL
                .into_iter()
                .map(|kind| SelectOption {
                    value: kind.as_str().to_string(),
                    label: kind.label().to_string(),
                    detail: None,
                    glyph: None,
                })
                .collect();
            let select_workflow = workflow.clone();
            let step_id = step.id.clone();
            let select = self.render_select_sized(
                SelectSpec {
                    id: STEP_SELECT_IDS[index],
                    current: Some(step.kind.as_str().to_string()),
                    placeholder: "",
                    options: Rc::new(step_options),
                    search: None,
                    on_select: Rc::new(move |this, value, _, cx| {
                        let Some(kind) = StepType::parse(&value) else {
                            return;
                        };
                        let mut next = select_workflow.clone();
                        if let Some(slot) = next.steps.iter_mut().find(|s| s.id == step_id) {
                            *slot = Step::new(kind);
                        }
                        this.persist_workflow(next, cx);
                    }),
                },
                true,
                cx,
            );
            let config = match step.kind {
                StepType::MarkdownExport => self.render_markdown_export_config(
                    STEP_FOLDER_IDS[index],
                    &step.directory,
                    FolderTarget::Step {
                        workflow_id: workflow_id.clone(),
                        step_id: step.id.clone(),
                    },
                    cx,
                ),
                kind => self.render_integration_config(
                    STEP_CONNECT_IDS[index],
                    kind,
                    step.target.as_ref(),
                    cx,
                ),
            };
            let remove_workflow = workflow.clone();
            let remove_step_id = step.id.clone();
            let on_remove: Action = Rc::new(move |this, cx| {
                let mut next = remove_workflow.clone();
                next.steps.retain(|s| s.id != remove_step_id);
                this.persist_workflow(next, cx);
            });
            cards = cards.child(
                div().child(self.step_connector()).child(
                    self.render_workflow_card(
                        WorkflowCard {
                            kind: StepKind::Action,
                            title: format!("Then {}", index + 1),
                            badge: "Action",
                            ready: step.is_ready(),
                            on_remove: Some((index, on_remove)),
                        },
                        div()
                            .child(div().max_w(px(320.0)).child(select))
                            .child(div().mt_3().child(config))
                            .into_any_element(),
                        cx,
                    ),
                ),
            );
        }

        // `AddWorkflowStep`
        let add_options: Vec<SelectOption> = StepType::ALL
            .into_iter()
            .map(|kind| SelectOption {
                value: kind.as_str().to_string(),
                label: kind.short_label().to_string(),
                detail: None,
                glyph: None,
            })
            .collect();
        let add_workflow = workflow.clone();
        let add_select = self.render_select_sized(
            SelectSpec {
                id: "automation-add-step",
                current: None,
                placeholder: "Add step",
                options: Rc::new(add_options),
                search: None,
                on_select: Rc::new(move |this, value, _, cx| {
                    let Some(kind) = StepType::parse(&value) else {
                        return;
                    };
                    let mut next = add_workflow.clone();
                    next.steps.push(Step::new(kind));
                    this.persist_workflow(next, cx);
                }),
            },
            true,
            cx,
        );
        cards = cards.child(self.step_connector()).child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .rounded_xl()
                .border_1()
                .border_dashed()
                .border_color(theme.border)
                .bg(alpha(theme.muted, 0.2))
                .p_4()
                .child(
                    div()
                        .child(
                            div()
                                .tw_text_sm()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(theme.foreground)
                                .child("Add an action"),
                        )
                        .child(
                            div()
                                .mt_1()
                                .tw_text_xs()
                                .text_color(theme.muted_foreground)
                                .child("Stack another destination. Steps run top to bottom."),
                        ),
                )
                .child(div().w(px(176.0)).flex_shrink_0().child(add_select)),
        );

        div()
            .rounded_2xl()
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_start()
                    .justify_between()
                    .gap_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .px_5()
                    .py_4()
                    .child(
                        div()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .truncate()
                                            .tw_text_sm()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(theme.foreground)
                                            .child("Workflow"),
                                    )
                                    .child(self.outline_badge(
                                        if workflow.enabled { "Enabled" } else { "Draft" },
                                        false,
                                    )),
                            )
                            .child(
                                div()
                                    .mt_1()
                                    .tw_text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("Add a trigger, then stack actions like Zapier."),
                            ),
                    ),
            )
            .child(cards)
            .child(
                div()
                    .border_t_1()
                    .border_color(theme.border)
                    .px_5()
                    .py_4()
                    .when(!workflow.is_ready(), |footer| {
                        footer.child(
                            div()
                                .tw_text_xs()
                                .text_color(theme.muted_foreground)
                                .child("Add at least one configured action before enabling."),
                        )
                    })
                    .children(self.render_last_run_line(workflow.last_run.as_ref())),
            )
            .into_any_element()
    }

    /// `WorkflowCard`: marker, title with badges (and the trash button for
    /// steps), then the body.
    fn render_workflow_card(
        &self,
        card: WorkflowCard,
        body: AnyElement,
        cx: &Context<Self>,
    ) -> AnyElement {
        let WorkflowCard {
            kind,
            title,
            badge,
            ready,
            on_remove,
        } = card;
        let theme = self.theme;
        let marker_text = if kind == StepKind::Trigger { "1" } else { "+" };
        let (bg, fg) = match (kind, theme.dark) {
            (StepKind::Trigger, false) => (gpui::rgb(0xfef3c7), gpui::rgb(0xb45309)),
            (StepKind::Trigger, true) => (gpui::rgb(0x451a03), gpui::rgb(0xfcd34d)),
            (_, false) => (gpui::rgb(0xdbeafe), gpui::rgb(0x1d4ed8)),
            (_, true) => (gpui::rgb(0x172554), gpui::rgb(0x93c5fd)),
        };
        div()
            .flex()
            .items_start()
            .gap_3()
            .rounded_xl()
            .border_1()
            .border_color(theme.border)
            .bg(theme.card)
            .p_4()
            .child(
                div()
                    .mt(px(2.0))
                    .flex()
                    .size(px(28.0))
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.0))
                    .bg(bg)
                    .tw_text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(fg)
                    .child(marker_text),
            )
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .tw_text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(theme.foreground)
                                    .child(SharedString::from(title)),
                            )
                            .child(self.outline_badge(badge, true))
                            .when(!ready, |row| {
                                row.child(self.outline_badge("Needs setup", true))
                            })
                            .when_some(on_remove, |row, (index, on_remove)| {
                                // `Button size="icon" variant="ghost"` at `size-7`, pushed
                                // right by `ml-auto` (a spacer here: Taffy's wrapping lines
                                // leave the auto margin short).
                                row.child(div().flex_1()).child(
                                    div()
                                        .id(SharedString::from(format!(
                                            "automation-remove-step-{index}"
                                        )))
                                        .flex()
                                        .size(px(28.0))
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(8.0))
                                        .cursor_pointer()
                                        .hover(move |style| style.bg(theme.accent))
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation()
                                        })
                                        .on_click(cx.listener(
                                            move |this, _: &ClickEvent, _, cx| {
                                                on_remove(this, cx);
                                            },
                                        ))
                                        .child(icon("trash", px(13.0), theme.muted_foreground)),
                                )
                            }),
                    )
                    .child(div().mt_3().child(body)),
            )
            .into_any_element()
    }
}

/// `WorkflowCard`'s props.
struct WorkflowCard {
    kind: StepKind,
    title: String,
    badge: &'static str,
    ready: bool,
    on_remove: Option<(usize, Action)>,
}

struct SmallButton {
    id: &'static str,
    outline: bool,
    glyph: Option<&'static str>,
    label: &'static str,
    disabled: bool,
    on_click: Option<WindowAction>,
}

/// Where a chosen export folder is written.
#[derive(Clone, Debug)]
enum FolderTarget {
    Starter,
    Step {
        workflow_id: String,
        step_id: String,
    },
}

/// `starter.renderIcon(size)`: the brand marks, or the Markdown mark
/// (`dark:invert`).
fn starter_icon(id: StarterId, size: f32, dark: bool) -> AnyElement {
    let path = match id {
        StarterId::SlackRecap => "brands/slack.svg",
        StarterId::NotionProjectNotes => "brands/notion.svg",
        StarterId::LinearActionItems => "brands/linear.svg",
        StarterId::MarkdownExport => {
            // `img.w-auto` with `height: size` inside the `size-4` span: the
            // preflight `max-width: 100%` squeezes the 208×128 mark to a square.
            return gpui::svg()
                .path("brands/markdown-mark.svg")
                .size(px(size))
                .flex_shrink_0()
                .text_color(if dark {
                    gpui::rgb(0xffffff)
                } else {
                    gpui::rgb(0x000000)
                })
                .into_any_element();
        }
    };
    img(gpui::ImageSource::Resource(gpui::Resource::Embedded(
        path.into(),
    )))
    .size(px(size))
    .flex_shrink_0()
    .into_any_element()
}
