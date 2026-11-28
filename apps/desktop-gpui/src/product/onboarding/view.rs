//! `OnboardingScreenContent` (`onboarding/index.tsx`): the step flow and what each step does.
//! `sections` renders the screen.

use std::{collections::HashSet, sync::Arc, time::Duration};

use anlg_supabase_auth::Claims;
use desktop_runtime::{CancellationToken, RuntimeHandle, ServiceError};
use gpui::{Context, EventEmitter, Global, ScrollHandle, SharedString, Subscription, Window};

use super::{
    Access, Billing, ConnectAction, Permission, PermissionAction, Step, TrialPhase, bgm::Bgm,
    connect_action, next, permission_action, permissions_complete, steps, trial_decision,
};
use crate::{
    contracts::ProductEvent,
    product::{
        cloud::CloudServices,
        local::{PERMISSION_ALLOWED, PERMISSION_NOT_REQUIRED},
        services::{
            Action, Mutation, Operation, Outcome, ProductServices, Request, Scope, Surface,
        },
    },
    workspace::calendar_sidebar::{self, CalendarRow},
};

mod sections;

const SCROLL_DELAY: Duration = Duration::from_millis(70);

/// Everything the sections render. Live onboarding fills it from the product services; a
/// [`OnboardingPreview`] supplies it directly.
#[derive(Clone, Debug, Default)]
pub struct State {
    pub microphone: Access,
    pub system_audio: Access,
    pub accessibility: Access,
    pub calendar: Access,
    pub pending: Option<Permission>,
    pub signed_in: bool,
    pub skipped_login: bool,
    pub signing_in: bool,
    pub trial: Option<TrialPhase>,
    pub enabled_calendars: usize,
    /// The calendars connected accounts discovered, which start out disabled.
    pub calendars: Vec<CalendarRow>,
    pub google_connected: bool,
    pub outlook_connected: bool,
    pub calendar_opening: Option<&'static str>,
    pub troubleshooting: bool,
    pub imported: Option<SharedString>,
    pub importing: bool,
    pub finishing: bool,
    pub finish_failed: bool,
    pub muted: bool,
}

/// Seeded onboarding for headless captures and tests. While this global is set, onboarding
/// renders it and never touches permissions, accounts, storage or audio.
#[derive(Clone, Default)]
pub struct OnboardingPreview {
    pub step: Option<Step>,
    pub state: State,
}

impl Global for OnboardingPreview {}

pub struct OnboardingView {
    services: Arc<dyn ProductServices>,
    cloud: Option<CloudServices>,
    runtime: RuntimeHandle,
    steps: &'static [Step],
    current: Step,
    permissions_continued: bool,
    state: State,
    live: bool,
    /// Grants asked for this session. The native check cannot tell "never asked" from
    /// "denied", so a refused request is what marks a grant denied.
    requested: HashSet<Permission>,
    imports_open: bool,
    import_menu: bool,
    troubleshooting_actions: bool,
    bgm: Option<Bgm>,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ProductEvent> for OnboardingView {}

impl OnboardingView {
    pub fn new(
        services: Arc<dyn ProductServices>,
        cloud: Option<CloudServices>,
        runtime: RuntimeHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let steps = steps(cfg!(target_os = "macos"));
        let live = !cx.has_global::<OnboardingPreview>();
        let mut this = Self {
            services,
            cloud,
            runtime,
            steps,
            current: steps[0],
            permissions_continued: false,
            state: State::default(),
            live,
            requested: HashSet::new(),
            imports_open: false,
            import_menu: false,
            troubleshooting_actions: false,
            bgm: None,
            scroll: ScrollHandle::new(),
            _subscriptions: Vec::new(),
        };
        if live {
            Self::track_step_event("onboarding_step_viewed", this.current);
            this.bgm = Some(Bgm::play());
            this._subscriptions
                .push(cx.observe_window_activation(window, |this, window, cx| {
                    // Grants are made in System Settings while the app is in the background.
                    // The native check opens capture devices, so only the steps showing grants
                    // re-check instead of polling like shipping.
                    if window.is_window_active()
                        && matches!(this.current, Step::Permissions | Step::Calendar)
                    {
                        this.refresh_permissions(cx);
                        this.refresh_calendars(cx);
                    }
                }));
            this.state.signed_in = this.account().is_some();
            this.watch_identity(cx);
            this.refresh_permissions(cx);
            this.refresh_calendars(cx);
            this.after_change(cx);
        } else {
            this.preview(cx);
            this._subscriptions
                .push(cx.observe_global::<OnboardingPreview>(|this, cx| this.preview(cx)));
        }
        this
    }

    fn preview(&mut self, cx: &mut Context<Self>) {
        let preview = cx.global::<OnboardingPreview>().clone();
        self.state = preview.state;
        if let Some(step) = preview.step {
            self.current = step;
        }
        cx.notify();
    }

    fn account(&self) -> Option<Arc<str>> {
        self.cloud
            .as_ref()
            .and_then(|cloud| cloud.auth_provider().identity().account_id)
    }

    fn request(&self) -> Request {
        Request {
            scope: Scope {
                account_id: self.account(),
                ..Scope::default()
            },
            generation: 0,
            cancel: CancellationToken::new(),
        }
    }

    fn perform(
        &self,
        surface: Surface,
        action: Action,
        target: Option<&str>,
        fields: &[(&str, &str)],
    ) -> futures::future::BoxFuture<'static, desktop_runtime::Result<Outcome>> {
        self.services
            .perform(surface, self.request(), mutation(action, target, fields))
    }

    fn step_id(step: Step) -> &'static str {
        match step {
            Step::Permissions => "permissions",
            Step::Login => "login",
            Step::Calendar => "calendar",
            Step::Imports => "imports",
            Step::Final => "final",
        }
    }

    fn access_status(access: Access) -> &'static str {
        match access {
            Access::Authorized => "authorized",
            Access::Denied => "denied",
            Access::NeverRequested => "neverRequested",
        }
    }

    fn track_step_event(event: &'static str, step: Step) {
        crate::telemetry::track(
            event,
            &[
                ("step", serde_json::json!(Self::step_id(step))),
                ("platform", serde_json::json!(std::env::consts::OS)),
            ],
        );
    }

    fn step_next(&mut self, cx: &mut Context<Self>) {
        if let Some(step) = next(self.steps, self.current) {
            self.go_to(step, cx);
        }
    }

    fn advance(&mut self, cx: &mut Context<Self>) {
        if self.live {
            Self::track_step_event("onboarding_step_completed", self.current);
        }
        self.step_next(cx);
    }

    fn go_to(&mut self, step: Step, cx: &mut Context<Self>) {
        self.current = step;
        if self.live {
            Self::track_step_event("onboarding_step_viewed", step);
        }
        self.after_change(cx);
        // Bring the new section to the top once its content has laid out.
        let index = self
            .steps
            .iter()
            .position(|candidate| *candidate == step)
            .unwrap_or(0);
        let scroll = self.scroll.clone();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SCROLL_DELAY).await;
            let _ = this.update(cx, |_, cx| {
                scroll.scroll_to_top_of_item(index);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Steps that move on by themselves: granted permissions and a signed-in account.
    fn after_change(&mut self, cx: &mut Context<Self>) {
        if !self.live {
            return;
        }
        if self.current == Step::Permissions
            && !self.permissions_continued
            && permissions_complete(
                self.state.microphone,
                self.state.system_audio,
                self.state.accessibility,
            )
        {
            self.permissions_continued = true;
            self.advance(cx);
            return;
        }
        if self.current == Step::Login && self.state.signed_in && self.state.trial.is_none() {
            self.start_trial(cx);
        }
    }

    fn skip(&mut self, cx: &mut Context<Self>) {
        if self.live {
            if self.current == Step::Login {
                crate::telemetry::track("onboarding_login_skipped", &[]);
            }
            Self::track_step_event("onboarding_step_skipped", self.current);
        }
        if self.current == Step::Login {
            self.state.skipped_login = true;
        }
        self.step_next(cx);
    }

    fn set_muted(&mut self, cx: &mut Context<Self>) {
        self.state.muted = !self.state.muted;
        if let Some(bgm) = &self.bgm {
            bgm.set_muted(self.state.muted);
        }
        cx.notify();
    }

    fn fail(&mut self, error: ServiceError, cx: &mut Context<Self>) {
        cx.emit(ProductEvent::Failed(error));
        cx.notify();
    }

    fn refresh_permissions(&mut self, cx: &mut Context<Self>) {
        if !self.live || !cfg!(target_os = "macos") {
            return;
        }
        let load = self.services.load(Surface::Permissions, self.request());
        cx.spawn(async move |this, cx| {
            let result = load.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(panel) => {
                    for row in panel.rows.iter() {
                        let Some(permission) = [
                            Permission::Microphone,
                            Permission::SystemAudio,
                            Permission::Accessibility,
                            Permission::Calendar,
                        ]
                        .into_iter()
                        .find(|permission| permission.id() == &*row.id) else {
                            continue;
                        };
                        let access = if [PERMISSION_ALLOWED, PERMISSION_NOT_REQUIRED]
                            .contains(&&*row.detail)
                        {
                            Access::Authorized
                        } else if this.requested.contains(&permission) {
                            Access::Denied
                        } else {
                            Access::NeverRequested
                        };
                        let prev = *this.access_mut(permission);
                        *this.access_mut(permission) = access;
                        if prev != access && this.requested.contains(&permission) {
                            crate::telemetry::track(
                                "permission_resolved",
                                &[
                                    ("permission", serde_json::json!(permission.id())),
                                    ("status", serde_json::json!(Self::access_status(access))),
                                    ("entry_point", serde_json::json!("onboarding")),
                                ],
                            );
                        }
                    }
                    this.after_change(cx);
                    cx.notify();
                }
                Err(error) => this.fail(error, cx),
            });
        })
        .detach();
    }

    fn access_mut(&mut self, permission: Permission) -> &mut Access {
        match permission {
            Permission::Microphone => &mut self.state.microphone,
            Permission::SystemAudio => &mut self.state.system_audio,
            Permission::Accessibility => &mut self.state.accessibility,
            Permission::Calendar => &mut self.state.calendar,
        }
    }

    fn act_on_permission(&mut self, permission: Permission, cx: &mut Context<Self>) {
        let access = *self.access_mut(permission);
        if !self.live || self.state.pending.is_some() || access == Access::Authorized {
            return;
        }
        let action = match permission_action(permission, access, cfg!(target_os = "macos")) {
            PermissionAction::Request => Action::RequestPermission,
            PermissionAction::OpenSettings => Action::OpenPermissionSettings,
        };
        let action_name = match action {
            Action::RequestPermission => "request",
            _ => "open_settings",
        };
        crate::telemetry::track(
            "permission_requested",
            &[
                ("permission", serde_json::json!(permission.id())),
                (
                    "previous_status",
                    serde_json::json!(Self::access_status(access)),
                ),
                ("entry_point", serde_json::json!("onboarding")),
                ("action", serde_json::json!(action_name)),
            ],
        );
        self.requested.insert(permission);
        self.state.pending = Some(permission);
        let job = self.perform(
            Surface::Permissions,
            action.clone(),
            Some(permission.id()),
            &[],
        );
        cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.state.pending = None;
                // A refused request is reported as an error; only a failure to open
                // System Settings is unexpected.
                if let Err(error) = result
                    && action == Action::OpenPermissionSettings
                {
                    this.fail(error, cx);
                }
                this.refresh_permissions(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn watch_identity(&mut self, cx: &mut Context<Self>) {
        let Some(cloud) = self.cloud.clone() else {
            return;
        };
        let mut identities = cloud.auth_provider().identities();
        cx.spawn(async move |this, cx| {
            while identities.changed().await.is_ok() {
                let signed_in = identities.borrow_and_update().account_id.is_some();
                let alive = this.update(cx, |this, cx| {
                    if this.state.signed_in != signed_in {
                        this.state.signed_in = signed_in;
                        this.state.trial = None;
                        this.after_change(cx);
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn sign_in(&mut self, cx: &mut Context<Self>) {
        if !self.live || self.state.signing_in {
            return;
        }
        crate::telemetry::track(
            "auth_started",
            &[
                ("entry_point", serde_json::json!("desktop_onboarding")),
                ("method", serde_json::json!("browser_handoff")),
            ],
        );
        self.state.signing_in = true;
        let job = self.perform(Surface::Account, Action::SignIn, None, &[]);
        cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.state.signing_in = false;
                if let Err(error) = result {
                    if !matches!(error, ServiceError::Cancelled) {
                        crate::telemetry::track(
                            "auth_failed",
                            &[
                                ("entry_point", serde_json::json!("desktop_onboarding")),
                                ("method", serde_json::json!("browser_handoff")),
                                ("failure_stage", serde_json::json!("open_browser")),
                            ],
                        );
                    }
                    this.fail(error, cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// `useTrialFlow`: report the account's plan, or start its trial, then continue.
    fn start_trial(&mut self, cx: &mut Context<Self>) {
        let (Some(cloud), Some(account)) = (self.cloud.clone(), self.account()) else {
            return;
        };
        self.state.trial = Some(TrialPhase::Checking);
        let services = self.services.clone();
        let request = self.request();
        cx.spawn(async move |this, cx| {
            let decision: desktop_runtime::Result<TrialPhase> = async {
                let lease = cloud.auth_provider().credential(account).await?;
                let claims = Claims::decode_insecure(lease.access_token()?)
                    .map_err(|_| ServiceError::Failed("Invalid account claims".into()))?;
                let billing = Billing {
                    paid: claims.is_paid(),
                    trialing: claims.has_active_trial(),
                };
                let eligible = !billing.paid
                    && !billing.trialing
                    && services
                        .load(Surface::Billing, request.clone())
                        .await?
                        .operations
                        .iter()
                        .any(|operation| operation.action == Action::StartTrial);
                Ok(trial_decision(billing, eligible))
            }
            .await;
            let mut phase = match decision {
                Ok(phase) => phase,
                Err(error) => {
                    if !matches!(error, desktop_runtime::ServiceError::Cancelled) {
                        crate::telemetry::track(
                            "trial_flow_client_error",
                            &[("stage", serde_json::json!("decision"))],
                        );
                        crate::telemetry::capture_operational_error(
                            "onboarding_trial",
                            Some("decision"),
                            &error.to_string(),
                            Some("anarlog"),
                            None,
                        );
                    }
                    TrialPhase::Failed
                }
            };
            let _ = this.update(cx, |this, cx| {
                this.state.trial = Some(phase);
                cx.notify();
            });
            if phase == TrialPhase::Starting {
                let start = mutation(Action::StartTrial, None, &[("interval", "monthly")]);
                phase = match services.perform(Surface::Billing, request, start).await {
                    Ok(_) => TrialPhase::Started { days: None },
                    Err(error) => {
                        if !matches!(error, desktop_runtime::ServiceError::Cancelled) {
                            crate::telemetry::track(
                                "trial_flow_client_error",
                                &[("stage", serde_json::json!("start_trial"))],
                            );
                            crate::telemetry::capture_operational_error(
                                "onboarding_trial",
                                Some("start_trial"),
                                &error.to_string(),
                                Some("anarlog"),
                                None,
                            );
                        }
                        TrialPhase::Failed
                    }
                };
                let _ = this.update(cx, |this, cx| {
                    this.state.trial = Some(phase);
                    cx.notify();
                });
            }
            if let Some(delay) = phase.continue_after_ms() {
                cx.background_executor()
                    .timer(Duration::from_millis(delay))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if this.current == Step::Login {
                        this.advance(cx);
                    }
                });
            }
        })
        .detach();
    }

    fn refresh_calendars(&mut self, cx: &mut Context<Self>) {
        if !self.live {
            return;
        }
        let reply = self
            .runtime
            .read(CancellationToken::new(), |services| async move {
                services
                    .executor
                    .execute(calendar_sidebar::CALENDAR_QUERY.into(), vec![])
                    .await
                    .map_err(|error| ServiceError::Failed(error.to_string().into()))
            });
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(rows) => {
                    let calendars = calendar_sidebar::rows(&rows);
                    this.state.enabled_calendars =
                        calendars.iter().filter(|row| row.enabled).count();
                    let has = |name: &str| calendars.iter().any(|row| &*row.provider == name);
                    this.state.google_connected = has("google");
                    this.state.outlook_connected = has("outlook");
                    this.state.calendars = calendars;
                    cx.notify();
                }
                Err(error) => this.fail(error, cx),
            });
        })
        .detach();
    }

    /// `CalendarSelection`'s toggle, as in the calendar sidebar.
    pub(super) fn toggle_calendar(&mut self, id: Arc<str>, cx: &mut Context<Self>) {
        if !self.live {
            return;
        }
        let Some(row) = self.state.calendars.iter_mut().find(|row| row.id == id) else {
            return;
        };
        row.enabled = !row.enabled;
        let enabled = row.enabled;
        self.state.enabled_calendars = self
            .state
            .calendars
            .iter()
            .filter(|row| row.enabled)
            .count();
        cx.notify();
        let reply = calendar_sidebar::set_enabled(&self.runtime, id, enabled);
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| {
                this.refresh_calendars(cx);
                if let Err(error) = result {
                    this.fail(error, cx);
                }
            });
        })
        .detach();
    }

    fn connect_apple(&mut self, cx: &mut Context<Self>) {
        if !self.live || self.state.pending.is_some() {
            return;
        }
        if self.state.calendar == Access::Authorized {
            let job = self.perform(
                Surface::Permissions,
                Action::OpenPermissionSettings,
                Some(Permission::Calendar.id()),
                &[],
            );
            cx.spawn(async move |this, cx| {
                if let Err(error) = job.await {
                    let _ = this.update(cx, |this, cx| this.fail(error, cx));
                }
            })
            .detach();
            return;
        }
        self.state.troubleshooting = true;
        self.request_calendar(cx);
    }

    fn request_calendar(&mut self, cx: &mut Context<Self>) {
        self.requested.insert(Permission::Calendar);
        self.state.pending = Some(Permission::Calendar);
        let job = self.perform(
            Surface::Calendar,
            Action::ConnectCalendar,
            Some("apple"),
            &[],
        );
        cx.spawn(async move |this, cx| {
            let _ = job.await;
            let _ = this.update(cx, |this, cx| {
                this.state.pending = None;
                this.refresh_permissions(cx);
                this.refresh_calendars(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn open_calendar_settings(&mut self, cx: &mut Context<Self>) {
        if !self.live {
            return;
        }
        let job = self.perform(
            Surface::Permissions,
            Action::OpenPermissionSettings,
            Some(Permission::Calendar.id()),
            &[],
        );
        cx.spawn(async move |this, cx| {
            if let Err(error) = job.await {
                let _ = this.update(cx, |this, cx| this.fail(error, cx));
            }
        })
        .detach();
    }

    fn connect_provider(&mut self, provider: &'static str, cx: &mut Context<Self>) {
        if !self.live {
            return;
        }
        match connect_action(self.state.signed_in, self.state.calendar_opening.is_some()) {
            ConnectAction::SignIn => {
                self.go_to(Step::Login, cx);
                self.sign_in(cx);
            }
            ConnectAction::Ignore => {}
            ConnectAction::Connect => {
                self.state.calendar_opening = Some(provider);
                let job = self.perform(
                    Surface::Calendar,
                    Action::ConnectCalendar,
                    Some(provider),
                    &[],
                );
                cx.spawn(async move |this, cx| {
                    let result = job.await;
                    let _ = this.update(cx, |this, cx| {
                        this.state.calendar_opening = None;
                        if let Err(error) = result {
                            this.fail(error, cx);
                        }
                        this.refresh_calendars(cx);
                        cx.notify();
                    });
                })
                .detach();
                cx.notify();
            }
        }
    }

    fn connect_import(&mut self, cx: &mut Context<Self>) {
        if !self.live {
            return;
        }
        if !self.state.signed_in {
            self.sign_in(cx);
            return;
        }
        let job = self.perform(
            Surface::Imports,
            Action::ConnectImport,
            Some("google-meet"),
            &[],
        );
        cx.spawn(async move |this, cx| {
            if let Err(error) = job.await {
                let _ = this.update(cx, |this, cx| this.fail(error, cx));
            }
        })
        .detach();
    }

    fn import_files(&mut self, cx: &mut Context<Self>) {
        self.import_menu = false;
        if !self.live || self.state.importing {
            cx.notify();
            return;
        }
        self.state.importing = true;
        let job = self.perform(Surface::Imports, Action::ImportFiles, None, &[]);
        cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.state.importing = false;
                match result {
                    Ok(_) => {
                        this.imports_open = true;
                        this.state.imported = Some("Meetings imported".into());
                    }
                    Err(ServiceError::Cancelled) => {}
                    Err(error) => this.fail(error, cx),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// `finishOnboarding`: stop the music, mark onboarding done with the welcome note, open it.
    fn finish(&mut self, cx: &mut Context<Self>) {
        if !self.live || self.state.finishing {
            return;
        }
        self.state.finishing = true;
        self.state.finish_failed = false;
        self.bgm = None;
        let job = self.perform(Surface::Onboarding, Action::CompleteOnboarding, None, &[]);
        cx.spawn(async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.state.finishing = false;
                match result {
                    Ok(Outcome::OpenSession(session)) => {
                        Self::track_step_event("onboarding_step_completed", Step::Final);
                        crate::telemetry::track("onboarding_completed", &[]);
                        cx.emit(ProductEvent::OpenSession(session))
                    }
                    _ => this.state.finish_failed = true,
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

fn mutation(action: Action, target: Option<&str>, fields: &[(&str, &str)]) -> Mutation {
    Mutation {
        operation: Operation {
            id: format!("onboarding-{action:?}").into(),
            label: "".into(),
            action,
            target_id: target.map(Into::into),
            disabled_reason: None,
            confirmation: None,
        },
        fields: fields
            .iter()
            .map(|(key, value)| (Arc::from(*key), Arc::from(*value)))
            .collect(),
    }
}
