mod automation;
mod chrome;
mod cloud_api;
mod device_approval;
mod enterprise_capture;
pub mod instruction;
pub(crate) mod lifecycle;
pub mod lock;
mod meeting_schedule;
mod note_windows;
mod notifications;
mod release_notes;
mod shared_notes;
mod startup;
pub mod team_invitations;
pub mod toasts;

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use desktop_runtime::{DocumentSnapshot, Reply, RuntimeHandle, ServiceError, SessionId};
use futures::{StreamExt, future::BoxFuture};
use gpui::{
    AppContext, Context, Entity, Focusable, Render, Subscription, Window, div, prelude::*, px,
};

use crate::ui::a11y::Accessible;
use crate::{
    application_services::{Effects, HostCommand, NativeServices},
    contracts::{
        EditorEvent, EditorInit, LaneContext, MeetingEvent, MeetingIntent, ProductEvent,
        ProductRoute, WorkspaceEvent,
    },
    editor::{EditorPane, menu::EditorRequest},
    meeting::{
        MeetingPane, MeetingServices,
        capture::{CaptureService, Phase},
        host::{self, MeetingHost},
        playback::{self, Playback},
        recovery, retention,
    },
    product::{
        OpenWorkspaceSection, ProductPane,
        cloud::{auth::SecureAuthProvider, host::NativeHost},
        local::HostEffects,
    },
    runtime_bridge,
    ui::theme::theme,
    workspace::{NoteEvent, WorkspaceAction, WorkspaceView, navigation::Route, pins},
};

struct ProfileStorage {
    root: PathBuf,
    vault: PathBuf,
}

impl anlg_storage::StorageRuntime for ProfileStorage {
    fn global_base(&self) -> Result<PathBuf, anlg_storage::Error> {
        Ok(self.root.clone())
    }

    fn vault_base(&self) -> Result<PathBuf, anlg_storage::Error> {
        Ok(self.vault.clone())
    }
}

struct PendingProduct {
    route: ProductRoute,
    workspace_route: Route,
    settings_section: Option<String>,
}

pub struct ApplicationView {
    runtime: RuntimeHandle,
    window: gpui::AnyWindowHandle,
    notifications: notifications::Notifications,
    schedule: meeting_schedule::MeetingSchedule,
    preferences_task: Option<gpui::Task<()>>,
    chrome: chrome::Chrome,
    workspace: Entity<WorkspaceView>,
    editor: Option<Entity<EditorPane>>,
    meeting: Option<Entity<MeetingPane>>,
    meeting_host: MeetingHost,
    main_window_active: bool,
    floating_pinned_session: Option<SessionId>,
    automation: Entity<crate::meeting::automation::MeetingAutomation>,
    dictation: Option<Entity<crate::dictation::Dictation>>,
    product: Option<Entity<ProductPane>>,
    pending_product: Option<PendingProduct>,
    startup_error: Option<String>,
    trial_dialogs: Entity<crate::product::trial::TrialDialogs>,
    instruction: Entity<instruction::Instruction>,
    services: Option<MeetingServices>,
    native: Option<NativeServices>,
    editor_vault: PathBuf,
    writers_paused: bool,
    lifecycle: lifecycle::Lifecycle,
    toasts: Entity<toasts::Toasts>,
    meeting_open: bool,
    capture_session: Option<SessionId>,
    closing: bool,
    drained: bool,
    subscriptions: Vec<Subscription>,
    editor_subscriptions: Vec<Subscription>,
    meeting_subscription: Option<Subscription>,
    product_subscriptions: Vec<Subscription>,
    pins_revision: Option<pins::Revision>,
    pins_saving: bool,
    pins_pending: Option<Arc<[Route]>>,
    note_windows:
        std::collections::HashMap<SessionId, gpui::WindowHandle<crate::note_window::NoteWindow>>,
    note_subscriptions: std::collections::HashMap<SessionId, Vec<Subscription>>,
}

impl ApplicationView {
    pub fn native_ready(&self) -> bool {
        self.native.is_some()
    }

    pub fn workspace(&self) -> &Entity<WorkspaceView> {
        &self.workspace
    }

    pub fn trial_dialogs(&self) -> &Entity<crate::product::trial::TrialDialogs> {
        &self.trial_dialogs
    }

    pub fn new(
        runtime: RuntimeHandle,
        ready: Reply<()>,
        storage: crate::launch::Storage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let profile = storage.root.clone();
        let shipping_store = storage.shipping_store();
        let shipping_webview = storage.shipping_webview();
        let trial_mirror = storage.shipping_webview();
        cx.set_global(crate::shipping_store::ShippingStore(shipping_store.clone()));
        // Installed first: the note and transcript views read the live meeting from it.
        let meeting_host = MeetingHost::install(cx);
        let toasts = toasts::install(cx);
        let live = meeting_host.live().clone();
        let automation = cx.new(|cx| {
            crate::meeting::automation::MeetingAutomation::new(
                runtime.clone(),
                Some(profile.clone()),
                window,
                cx,
            )
        });
        let workspace = cx.new(|cx| {
            WorkspaceView::new(
                LaneContext {
                    runtime: runtime.clone(),
                },
                ready,
                window,
                cx,
            )
        });
        let mut subscriptions = vec![
            cx.observe(&live, |this, _, cx| this.live_changed(cx)),
            cx.observe_window_activation(window, |this, window, cx| {
                let active = window.is_window_active();
                if this.main_window_active != active {
                    this.main_window_active = active;
                    if active {
                        this.floating_pinned_session = None;
                    }
                    this.sync_floating_bar(cx);
                }
            }),
            cx.on_focus_lost(window, |this, window, cx| {
                if this.product.is_none() && !this.closing && !this.writers_paused {
                    cx.focus_view(&this.workspace, window);
                    cx.notify();
                }
            }),
            cx.subscribe_in(&workspace, window, |this, _, event, window, cx| {
                this.workspace_event(event, window, cx)
            }),
            cx.observe(&workspace, |this, workspace, cx| {
                if let Some(pending) = &this.pending_product
                    && workspace.read(cx).current_route() != pending.workspace_route
                {
                    this.pending_product = None;
                }
            }),
            cx.subscribe(&workspace, |this, _, event, cx| match event {
                WorkspaceAction::SidebarChanged => cx.notify(),
                WorkspaceAction::OpenShared(id) => this.mount_shared(id.to_string(), false, cx),
                WorkspaceAction::NewNoteWindow => this.open_note_window(None, cx),
                WorkspaceAction::OpenNoteWindow(id) => this.open_note_window(Some(id.clone()), cx),
                WorkspaceAction::PinnedChanged(routes) => this.save_pins(routes.clone(), cx),
                WorkspaceAction::OpenFloatingPanel => {
                    this.floating_pinned_session = this
                        .meeting_host
                        .live()
                        .read(cx)
                        .current()
                        .filter(|live| live.mode == crate::meeting::live::SessionMode::Active)
                        .map(|live| live.session.clone());
                    this.sync_floating_bar(cx);
                    this.meeting_host.show_floating(cx);
                }
            }),
            // Plan-gated surfaces read the account while rendering.
            cx.observe_global::<crate::product::account::Account>(|_, cx| cx.notify()),
            cx.subscribe_in(
                &automation,
                window,
                |this, _, request: &crate::meeting::automation::Request, window, cx| {
                    this.automation_request(request, window, cx)
                },
            ),
            cx.subscribe(&automation, |this, _, observed, cx| {
                this.meeting_observed(observed, cx)
            }),
        ];
        subscriptions.push(lock::install(&runtime, window, cx));
        startup::install(&runtime, cx);
        subscriptions.push(cx.observe_global::<startup::StartupGate>(|_, cx| cx.notify()));
        let dictation = match crate::dictation::Dictation::install(runtime.clone(), cx) {
            Ok(dictation) => {
                subscriptions.push(cx.subscribe(&dictation, |this, _, failed, cx| {
                    let crate::dictation::DictationFailed(message) = failed;
                    this.status(message, cx)
                }));
                Some(dictation)
            }
            Err(error) => {
                tracing::warn!(%error, "dictation is unavailable");
                None
            }
        };
        crate::product::settings::apply_stored_preferences(&runtime, cx);
        let pins_runtime = runtime.clone();
        let pins_store = shipping_store.clone();
        cx.spawn(async move |this, cx| {
            let mut state = pins_runtime.state();
            loop {
                if matches!(
                    *state.borrow_and_update(),
                    desktop_runtime::RuntimeState::Ready
                ) {
                    break;
                }
                if matches!(*state.borrow(), desktop_runtime::RuntimeState::Closed(_)) {
                    return;
                }
                if state.changed().await.is_err() {
                    return;
                }
            }
            let result = match pins::load(&pins_runtime, pins_store) {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| match result {
                Ok((revision, routes)) => {
                    this.pins_revision = Some(revision);
                    this.workspace.update(cx, |workspace, cx| {
                        workspace.restore_pinned(&routes, cx);
                        workspace.set_pin_persistence(true);
                    });
                }
                Err(error) => this.status(&error.to_string(), cx),
            });
            let _ = this.update(cx, |_, cx| {
                release_notes::open_after_update(pins_runtime.clone(), cx)
            });
        })
        .detach();
        let startup_runtime = runtime.clone();
        let editor_vault = storage.vault();
        let tray_settings = match shipping_store.clone() {
            Some(store) => lifecycle::TraySettings::Shipping(store),
            None => lifecycle::TraySettings::File(profile.join("tray.json")),
        };
        let (effects, mut commands) = Effects::channel(runtime.clone());
        let flush_effects = effects.clone();
        let instruction = cx.new(|cx| instruction::Instruction::new(window.window_handle(), cx));
        subscriptions.push(
            cx.subscribe_in(&instruction, window, |this, _, event, window, cx| {
                this.instruction_event(event, window, cx)
            }),
        );
        let card = instruction.downgrade();
        let host = NativeHost::install(
            cx,
            move |_, _| flush_effects.flush_drafts(),
            move |url, handoff, cx| instruction::hand_off(card.clone(), url, handoff, cx),
        );
        cx.spawn(async move |this, cx| {
            while let Some(command) = commands.next().await {
                match command {
                    HostCommand::Flush(reply) => {
                        let result = match this.update(cx, |this, cx| this.flush_drafts(cx)) {
                            Ok(flush) => flush.await,
                            Err(_) => Err(ServiceError::Closed),
                        };
                        let _ = reply.send(result);
                    }
                    HostCommand::Pause(reply) => {
                        let result = match this.update(cx, |this, cx| this.pause_writers(cx)) {
                            Ok(flush) => flush.await,
                            Err(_) => Err(ServiceError::Closed),
                        };
                        // A pause that could not drain must not leave the app read-only.
                        if result.is_err() {
                            let _ = this.update(cx, |this, cx| {
                                if this.writers_paused {
                                    this.resume_writers(cx);
                                }
                            });
                        }
                        let _ = reply.send(result);
                    }
                    HostCommand::Resume(reply) => {
                        let result = this
                            .update(cx, |this, cx| {
                                this.resume_writers(cx);
                            })
                            .map_err(|_| ServiceError::Closed);
                        let _ = reply.send(result);
                    }
                    HostCommand::Relaunch(reply) => {
                        crate::platform::windows::request_restart(
                            crate::platform::windows::Restart::App,
                        );
                        let _ = reply.send(Ok(()));
                        cx.background_executor()
                            .timer(Duration::from_millis(100))
                            .await;
                        let _ = this.update(cx, |this, cx| this.request_quit(cx));
                    }
                }
            }
        })
        .detach();
        let vault = editor_vault.clone();
        let startup = cx.background_executor().spawn(async move {
            let mut state = startup_runtime.state();
            loop {
                match state.borrow_and_update().clone() {
                    desktop_runtime::RuntimeState::Ready => break,
                    desktop_runtime::RuntimeState::Closed(result) => {
                        return result.and(Err(ServiceError::Closed));
                    }
                    _ => {}
                }
                state.changed().await.map_err(|_| ServiceError::Closed)?;
            }
            if let Some(store) = shipping_store {
                let imported =
                    crate::product::onboarding::import_shipping(&startup_runtime, store).await;
                if let Err(error) = imported {
                    tracing::warn!(%error, "the shipping app's onboarding state was not read");
                }
            }
            if let Ok(reply) = crate::product::settings::watch(&startup_runtime)
                && let Ok(watch) = reply.receive().await
            {
                let snapshot =
                    crate::product::settings::decode(&watch.snapshots.borrow().rows).ok();
                let _ = watch.unsubscribe().await;
                if let Some(snapshot) = snapshot {
                    let (telemetry, crash) = crate::telemetry::consent_from_settings(&snapshot);
                    crate::telemetry::set_telemetry_enabled(telemetry);
                    crate::telemetry::set_crash_reporting_enabled(crash);
                }
            }
            crate::telemetry::track("app_opened", &[]);
            if let Some(mirror) = shipping_webview {
                crate::shipping_webview::import(&mirror, &profile);
            }
            let native =
                NativeServices::new(startup_runtime.clone(), storage, effects.clone(), host)
                    .await?;
            let storage = Arc::new(ProfileStorage {
                root: profile,
                vault: vault.clone(),
            });
            let audio_vault = vault.clone();
            let capture = CaptureService::spawn(
                startup_runtime.clone(),
                Arc::new(anlg_audio_actual::ActualAudio),
                storage,
                native.providers.start_resolver(),
            )?;
            let activities = capture.activities.clone();
            let recovery_runtime = startup_runtime.clone();
            let resolver = native.providers.recovery_resolver();
            let recovery_result = match startup_runtime.service(move |_| async move {
                recovery::recover_startup(&recovery_runtime, vault, activities, resolver).await
            }) {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let (interrupted, recovery_error) = match recovery_result {
                Ok(reports) => (
                    reports
                        .into_iter()
                        .filter(|report| report.result.is_err())
                        .collect::<Vec<_>>(),
                    None,
                ),
                Err(error) => {
                    tracing::error!(%error, "interrupted capture recovery failed on startup");
                    (Vec::new(), Some(error))
                }
            };
            let services = MeetingServices {
                capture,
                playback: Playback::spawn()?,
                vault: audio_vault,
            };
            effects.attach(&services);
            let capture = services.capture.clone();
            let ai = native
                .providers
                .ai_services(Arc::new(move |session| capture.is_active(session)))?;
            Ok((services, native, ai, interrupted, recovery_error))
        });
        cx.spawn(async move |this, cx| {
            let result = startup.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok((services, native, ai, interrupted, recovery_error)) => {
                    cx.set_global(services.clone());
                    cx.set_global(ai);
                    cx.set_global(native.providers.clone());
                    this.subscriptions
                        .push(crate::product::providers::configure_when_paid(
                            native.providers.clone(),
                            cx,
                        ));
                    if let Some(dictation) = &this.dictation {
                        let voice = dictation.read(cx).voice(native.providers.clone());
                        cx.set_global(voice);
                    }
                    let vault = services.vault.clone();
                    this.start_retention(vault.clone(), services.capture.activities.clone(), cx);
                    this.services = Some(services);
                    let calendar = crate::workspace::calendar_accounts::HostCalendarAccounts::new(
                        this.runtime.clone(),
                        native.local.clone(),
                        native.cloud.service.clone(),
                    );
                    this.workspace.update(cx, |workspace, cx| {
                        workspace.set_calendar_accounts(Some(Arc::new(calendar)), cx)
                    });
                    if let Some(cloud) = native.cloud.service.clone() {
                        this.watch_cloud(cloud, cx);
                    }
                    cloud_api::start(&native, cx);
                    this.native = Some(native);
                    if let Some(pending) = this.pending_product.take()
                        && this.workspace.read(cx).current_route() == pending.workspace_route
                    {
                        let window = this.window;
                        let view = cx.entity().downgrade();
                        let route = pending.route;
                        let expected = pending.workspace_route;
                        let section = pending.settings_section;
                        cx.defer(move |cx| {
                            let _ = cx.update_window(window, |_, window, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    if this.product.is_none()
                                        && this.workspace.read(cx).current_route() == expected
                                    {
                                        this.open_product_with_section(route, section, window, cx);
                                    }
                                });
                            });
                        });
                    }
                    this.start_schedule(cx);
                    this.watch_preferences(cx);
                    this.start_lifecycle(cx);
                    this.watch_updates(cx);
                    this.open_onboarding_if_needed(cx);
                    if let Some(error) = recovery_error {
                        this.status(
                            &format!("Interrupted recording recovery failed: {error}"),
                            cx,
                        );
                    }
                    for capture in interrupted {
                        this.offer_discard(capture, vault.clone(), cx);
                    }
                }
                Err(error) => {
                    this.pending_product = None;
                    this.startup_error = Some(error.to_string());
                    this.status(&format!("Meeting startup failed: {error}"), cx);
                }
            });
        })
        .detach();
        let trial_dialogs = cx
            .new(|cx| crate::product::trial::TrialDialogs::new(runtime.clone(), trial_mirror, cx));
        subscriptions.push(cx.subscribe(
            &trial_dialogs,
            |this, _, _: &crate::product::trial::ReferFriend, cx| {
                this.workspace.update(cx, |workspace, cx| {
                    workspace.open_route(Route::settings("referrals"), true, cx)
                })
            },
        ));
        Self {
            window: window.window_handle(),
            notifications: notifications::Notifications::default(),
            schedule: Default::default(),
            preferences_task: None,
            runtime,
            chrome: chrome::Chrome::default(),
            workspace,
            editor: None,
            meeting: None,
            meeting_host,
            main_window_active: window.is_window_active(),
            floating_pinned_session: None,
            automation,
            dictation,
            product: None,
            pending_product: None,
            startup_error: None,
            trial_dialogs,
            instruction,
            services: None,
            native: None,
            editor_vault,
            writers_paused: false,
            lifecycle: lifecycle::Lifecycle::new(tray_settings),
            toasts,
            capture_session: None,
            meeting_open: false,
            closing: false,
            drained: false,
            subscriptions,
            editor_subscriptions: Vec::new(),
            meeting_subscription: None,
            product_subscriptions: Vec::new(),
            pins_revision: None,
            pins_saving: false,
            pins_pending: None,
            note_windows: Default::default(),
            note_subscriptions: Default::default(),
        }
    }

    pub fn native_event(
        &mut self,
        event: crate::native_events::NativeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::native_events::NativeEvent;
        match event {
            NativeEvent::NewNote => {
                self.show_main(window, cx);
                self.workspace.update(cx, |view, cx| view.create(false, cx));
            }
            NativeEvent::About => self.show_about(cx),
            NativeEvent::CloseMain => self.close_main(window, cx),
            NativeEvent::Quit => self.quit_now(window, cx),
            NativeEvent::Action(action) => self.lifecycle_action(action, window, cx),
            NativeEvent::DeepLink { raw, .. } => {
                if let Ok(link) = desktop_runtime::deeplink::DeepLink::parse(&raw)
                    && instruction::dismissed_by(&link)
                {
                    self.instruction.update(cx, |card, cx| card.dismiss(cx));
                    if matches!(link, desktop_runtime::deeplink::DeepLink::Integration(_)) {
                        self.workspace
                            .update(cx, |view, cx| view.integrations_changed(cx));
                    }
                }
                match desktop_runtime::deeplink::DeepLink::parse(&raw) {
                    Ok(desktop_runtime::deeplink::DeepLink::ShareAccount { share_id }) => {
                        self.open_shared(share_id, false, cx)
                    }
                    Ok(desktop_runtime::deeplink::DeepLink::ShareHandoff { request_id }) => {
                        self.open_shared(request_id, true, cx)
                    }
                    Ok(desktop_runtime::deeplink::DeepLink::OnboardingComplete) => {
                        let Some(services) = self.services.clone() else {
                            return;
                        };
                        let Some(session) = services.capture.take_update(u64::MAX).session else {
                            return;
                        };
                        let active = session.clone();
                        let reply = self.runtime.read(desktop_runtime::CancellationToken::new(), move |services| async move {
                        let rows = services.executor.execute("SELECT id FROM sessions WHERE id=? AND deleted_at IS NULL AND CASE WHEN json_valid(event_json) THEN json_extract(event_json,'$.tracking_id') END='anarlog-onboarding-demo-v1'".into(), vec![serde_json::json!(session.0)]).await.map_err(|error| ServiceError::Failed(error.to_string().into()))?;
                        Ok(!rows.is_empty())
                    });
                        cx.spawn(async move |this, cx| {
                            let result: desktop_runtime::Result<()> = async {
                                if reply?.receive().await? && services.capture.is_active(&active) {
                                    services
                                        .capture
                                        .stop()?
                                        .await
                                        .map_err(|_| ServiceError::Closed)??;
                                }
                                Ok(())
                            }
                            .await;
                            if let Err(error) = result {
                                let _ =
                                    this.update(cx, |this, cx| this.status(&error.to_string(), cx));
                            }
                        })
                        .detach();
                    }
                    Ok(desktop_runtime::deeplink::DeepLink::Integration(callback)) => {
                        let Some(native) = &self.native else {
                            return;
                        };
                        let calendar = native.local.calendar.clone();
                        let runtime = self.runtime.clone();
                        let return_to = callback.return_to.clone();
                        let job = runtime.clone().service(move |services| async move {
                            calendar
                                .reconcile(&runtime, callback, services.shutdown_requested)
                                .await
                        });
                        cx.spawn(async move |this, cx| {
                            let result = async { job?.receive().await }.await;
                            let _ = this.update(cx, |this, cx| match result {
                                Ok(()) => {
                                    if matches!(
                                        return_to.as_deref(),
                                        Some("calendar" | "settings-calendar")
                                    ) {
                                        this.leave_product(Route::Calendar, cx);
                                    }
                                    toasts::success("Integration refreshed.", cx);
                                }
                                Err(error) => this.status(&error.to_string(), cx),
                            });
                        })
                        .detach();
                    }
                    Ok(link) => {
                        let cloud = self
                            .native
                            .as_ref()
                            .and_then(|native| native.cloud.service.clone());
                        let Some(cloud) = cloud else {
                            self.status("Configure cloud sign-in before opening this link.", cx);
                            return;
                        };
                        cx.spawn(async move |this, cx| {
                            let is_auth =
                                matches!(link, desktop_runtime::deeplink::DeepLink::Auth { .. });
                            let result = match url::Url::parse(&raw) {
                                Ok(url) if is_auth => cloud.handle_deep_link(url).await.map(|_| ()),
                                Ok(_) => {
                                    let billing = matches!(
                                        link,
                                        desktop_runtime::deeplink::DeepLink::BillingRefresh
                                    );
                                    cloud.refresh(billing).await
                                }
                                Err(_) => Err(ServiceError::Failed("Invalid deep link".into())),
                            };
                            let _ = this.update(cx, |this, cx| match result {
                                Ok(_) => {
                                    if is_auth {
                                        crate::telemetry::track("user_signed_in", &[]);
                                    }
                                    cloud.publish_account(cx);
                                    toasts::success("Account updated.", cx)
                                }
                                Err(error) => this.status(&error.to_string(), cx),
                            });
                        })
                        .detach();
                    }
                    Err(error) => self.status(&error.to_string(), cx),
                }
            }
        }
    }

    /// `share/open` links: an account link refreshes the cached copy, then opens the shared
    /// note; a handoff link opens a view-only preview.
    fn open_shared(&mut self, id: String, handoff: bool, cx: &mut Context<Self>) {
        if !self
            .workspace
            .update(cx, |workspace, cx| workspace.can_close(cx))
        {
            return;
        }
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.read(cx).is_dirty())
        {
            self.status("Save the note before opening a shared note.", cx);
            return;
        }
        if handoff {
            self.leave_product(Route::SharedPreview(id.clone().into()), cx);
            self.mount_shared(id, true, cx);
            return;
        }
        let route = Route::SharedSession(id.clone().into());
        let cloud = self
            .native
            .as_ref()
            .and_then(|native| native.cloud.service.clone())
            .filter(|_| crate::product::account::current(cx).signed_in());
        let Some(cloud) = cloud else {
            self.leave_product(route, cx);
            return;
        };
        cx.spawn(async move |this, cx| {
            let _ = cloud
                .refresh_shared(id, desktop_runtime::CancellationToken::new())
                .await;
            let _ = this.update(cx, |this, cx| this.leave_product(route, cx));
        })
        .detach();
    }

    /// Shows a shared note or preview once its route is current.
    fn mount_shared(&mut self, id: String, preview: bool, cx: &mut Context<Self>) {
        let route = if preview {
            Route::SharedPreview(id.clone().into())
        } else {
            Route::SharedSession(id.clone().into())
        };
        if self.product.is_some() || self.workspace.read(cx).current_route() != route {
            return;
        }
        let runtime = self.runtime.clone();
        let view = cx.new(|cx| {
            crate::product::sharing::shared_note::SharedNoteView::new(runtime, id, preview, cx)
        });
        self.workspace.update(cx, |workspace, cx| {
            workspace.set_route_content(route, view.into(), cx)
        });
    }

    pub fn native_tick(&mut self, cx: &mut Context<Self>) {
        self.sync_recording_title(cx);
        self.poll_downloads(cx);
        let Some(services) = &self.services else {
            return;
        };
        self.meeting_host.tick(services, cx);
        let update = services.capture.take_update(u64::MAX);
        match update.phase {
            Phase::Loading | Phase::Listening | Phase::Finalizing => {
                if self.capture_session.is_none() && update.session.is_some() {
                    crate::telemetry::track(
                        "recording_started",
                        &[("provider", serde_json::json!(update.provider.as_ref()))],
                    );
                }
                self.capture_session = update.session
            }
            Phase::Idle => {
                if let Some(session) = self.capture_session.take() {
                    crate::telemetry::track("recording_completed", &[]);
                    lock::capture_ended(&session, cx);
                    crate::meeting::automation::capture_completed(cx);
                    if !self.closing && !self.shutdown_begun() {
                        crate::meeting::upload::Uploads::global(cx).update(cx, |uploads, cx| {
                            uploads.capture_transcribed(session.clone(), cx)
                        });
                    }
                    self.workspace.update(cx, |workspace, cx| {
                        workspace.run_automations(
                            crate::workspace::automation_runner::Trigger::MeetingCompleted,
                            session,
                            cx,
                        )
                    });
                }
            }
            Phase::Failed => {
                if let Some(session) = self.capture_session.take() {
                    crate::telemetry::track("recording_failed", &[]);
                    lock::capture_ended(&session, cx);
                }
            }
        }
        self.schedule_capture_phase(cx);
    }

    pub(super) fn resume_writers(&mut self, cx: &mut Context<Self>) {
        self.writers_paused = false;
        if self.closing || self.drained || self.shutdown_begun() {
            cx.notify();
            return;
        }
        if let Some(editor) = &self.editor {
            editor.update(cx, |editor, cx| editor.set_read_only(false, cx));
        }
        let note = self.workspace.read(cx).note().clone();
        note.update(cx, |note, cx| note.resume_summaries(cx));
        for window in self.note_windows.values() {
            let _ = window.update(cx, |view, _, cx| view.resume(cx));
        }
        cx.notify();
    }

    fn can_flush_drafts(&mut self, cx: &mut Context<Self>) -> bool {
        if self.pins_saving
            || self.pins_pending.is_some()
            || self.note_windows.values().any(|window| {
                window
                    .read(cx)
                    .is_ok_and(|view| view.note().read(cx).has_unsaved_title(cx))
            })
        {
            return false;
        }
        let mounted_editor = self
            .editor
            .as_ref()
            .map(|editor| editor.read(cx).init.session_id.clone());
        self.workspace.update(cx, |view, cx| {
            view.can_flush_to_close(mounted_editor.as_ref(), cx)
        })
    }

    fn start_draft_flushes(
        &mut self,
        pause: bool,
        cx: &mut Context<Self>,
    ) -> BoxFuture<'static, desktop_runtime::Result<()>> {
        let dirty_editor = pause
            .then(|| {
                self.editor
                    .as_ref()
                    .filter(|editor| editor.read(cx).is_dirty())
                    .cloned()
            })
            .flatten();
        let editor_flush = self.editor.as_ref().map(|editor| {
            editor.update(cx, |editor, cx| {
                if pause {
                    editor.set_read_only(true, cx);
                }
                editor.flush()
            })
        });
        let main_note = self.workspace.read(cx).note().clone();
        let summaries = main_note.update(cx, |note, cx| note.flush_summaries(pause, cx));
        let mut dirty_notes = Vec::new();
        if pause {
            if !summaries.is_empty() {
                dirty_notes.push(main_note);
            }
            for window in self.note_windows.values() {
                if let Ok(note) = window.read(cx).map(|view| view.note())
                    && note.read(cx).has_unsaved_summary(cx)
                {
                    dirty_notes.push(note);
                }
            }
        }
        let notes = self
            .note_windows
            .values()
            .filter_map(|window| {
                window
                    .update(cx, |view, _, cx| {
                        if pause {
                            view.pause(cx)
                        } else {
                            view.flush(false, cx)
                        }
                    })
                    .ok()
            })
            .collect::<Vec<_>>();
        let (wake_tx, mut wake_rx) = futures::channel::mpsc::unbounded::<()>();
        let rename_failed = Arc::new(AtomicBool::new(false));
        let mut subscriptions = Vec::new();
        if let Some(editor) = &dirty_editor {
            let tx = wake_tx.clone();
            subscriptions.push(cx.observe(editor, move |_, _, _| {
                let _ = tx.unbounded_send(());
            }));
        }
        for note in &dirty_notes {
            let tx = wake_tx.clone();
            subscriptions.push(cx.observe(note, move |_, _, _| {
                let _ = tx.unbounded_send(());
            }));
            let tx = wake_tx.clone();
            let failed = rename_failed.clone();
            subscriptions.push(cx.subscribe(note, move |_, _, event: &NoteEvent, _| {
                if matches!(event, NoteEvent::RenameFailed) {
                    failed.store(true, Ordering::SeqCst);
                    let _ = tx.unbounded_send(());
                }
            }));
        }
        drop(wake_tx);
        let runtime = self.runtime.clone();
        Box::pin(cx.spawn(async move |_, cx| -> desktop_runtime::Result<()> {
            let _subscriptions = subscriptions;
            let mut first_error = None;
            if let Some(flush) = editor_flush
                && let Err(error) = flush
                    .await
                    .map_err(|_| ServiceError::Closed)
                    .and_then(|result| result)
            {
                first_error.get_or_insert(error);
            }
            for summary in summaries {
                if let Err(error) = summary
                    .await
                    .map_err(|_| ServiceError::Closed)
                    .and_then(|result| result)
                {
                    first_error.get_or_insert(error);
                }
            }
            for note in notes {
                if let Err(error) = note.await {
                    first_error.get_or_insert(error);
                }
            }
            if let Some(error) = first_error {
                return Err(error);
            }
            if let Some(editor) = dirty_editor {
                let editor = editor.downgrade();
                while editor
                    .read_with(cx, |editor, _| editor.is_dirty())
                    .map_err(|_| ServiceError::Closed)?
                {
                    if wake_rx.next().await.is_none() {
                        return Err(ServiceError::Closed);
                    }
                }
            }
            for note in dirty_notes {
                let note = note.downgrade();
                loop {
                    if rename_failed.load(Ordering::SeqCst) {
                        return Err(ServiceError::Failed("Title save failed.".into()));
                    }
                    let (unsaved_summary, renaming, unsaved_title) = note
                        .read_with(cx, |note, cx| {
                            (
                                note.has_unsaved_summary(cx),
                                note.is_renaming(),
                                note.has_unsaved_title(cx),
                            )
                        })
                        .map_err(|_| ServiceError::Closed)?;
                    match (unsaved_summary, renaming, unsaved_title) {
                        (false, false, false) => break,
                        (false, false, true) => return Err(ServiceError::Busy),
                        _ => {
                            if wake_rx.next().await.is_none() {
                                return Err(ServiceError::Closed);
                            }
                        }
                    }
                }
            }
            runtime.flush()?.receive().await
        }))
    }

    fn flush_drafts(
        &mut self,
        cx: &mut Context<Self>,
    ) -> BoxFuture<'static, desktop_runtime::Result<()>> {
        if !self.can_flush_drafts(cx) {
            return Box::pin(async { Err(ServiceError::Busy) });
        }
        self.start_draft_flushes(false, cx)
    }

    fn open_note_window(&mut self, id: Option<SessionId>, cx: &mut Context<Self>) {
        self.note_windows
            .retain(|_, window| window.read(cx).is_ok());
        self.note_subscriptions
            .retain(|id, _| self.note_windows.contains_key(id));
        if let Some(id) = &id
            && let Some(window) = self.note_windows.get(id)
        {
            let _ = window.update(cx, |_, window, _| window.activate_window());
            return;
        }
        if self.note_windows.len() >= 16 || self.writers_paused || self.closing {
            self.status(
                "Close a note window or finish pending work before opening another.",
                cx,
            );
            return;
        }
        let request = match id {
            Some(id) => self
                .runtime
                .open_session(id, desktop_runtime::CancellationToken::new()),
            None => self.runtime.create_note("".into()),
        };
        cx.spawn(async move |this, cx| {
            let result = match request {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                let result = result.and_then(|note| {
                    let id = note.summary.id.clone();
                    if this.closing || this.writers_paused || this.note_windows.len() >= 16 {
                        return Err(ServiceError::Busy);
                    }
                    if let Some(window) = this.note_windows.get(&id) {
                        let _ = window.update(cx, |_, window, _| window.activate_window());
                        return Ok(());
                    }
                    let options = crate::platform::windows::WindowIdentity::Note(id.clone())
                        .options(None, cx)?;
                    let runtime = this.runtime.clone();
                    let vault = this.editor_vault.clone();
                    let workspace = this.workspace.downgrade();
                    let window = cx
                        .open_window(options, move |window, cx| {
                            cx.new(|cx| {
                                crate::note_window::NoteWindow::new(
                                    runtime, note, vault, workspace, window, cx,
                                )
                            })
                        })
                        .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
                    let view = window
                        .update(cx, |_, _, cx| cx.entity())
                        .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
                    let subscriptions = this.note_window_subscriptions(&view, cx);
                    this.note_subscriptions.insert(id.clone(), subscriptions);
                    this.note_windows.insert(id, window);
                    Ok(())
                });
                if let Err(error) = result {
                    this.status(&error.to_string(), cx);
                }
            });
        })
        .detach();
    }

    pub(super) fn pause_writers(
        &mut self,
        cx: &mut Context<Self>,
    ) -> BoxFuture<'static, desktop_runtime::Result<()>> {
        if self.closing
            || self.drained
            || self.shutdown_begun()
            || self.services.as_ref().is_some_and(|s| {
                matches!(
                    s.capture.take_update(u64::MAX).phase,
                    Phase::Loading | Phase::Listening | Phase::Finalizing
                )
            })
            || !self.can_flush_drafts(cx)
        {
            return Box::pin(async { Err(ServiceError::Busy) });
        }
        self.writers_paused = true;
        let flush = self.start_draft_flushes(true, cx);
        let cloud = self.native.as_ref().and_then(|n| n.cloud.service.clone());
        let runtime = self.runtime.clone();
        Box::pin(async move {
            flush.await?;
            if let Some(cloud) = cloud {
                cloud.shutdown().await?;
            }
            runtime
                .submit(|services| async move {
                    services
                        .executor
                        .execute("PRAGMA wal_checkpoint(TRUNCATE)".into(), vec![])
                        .await
                        .map_err(|e| ServiceError::Failed(e.to_string().into()))?;
                    Ok(())
                })?
                .receive()
                .await
        })
    }

    fn watch_cloud(&mut self, cloud: crate::product::cloud::CloudServices, cx: &mut Context<Self>) {
        self.trial_dialogs
            .update(cx, |dialogs, _| dialogs.set_cloud(cloud.clone()));
        enterprise_capture::watch(cloud.clone(), cx);
        cx.set_global(crate::product::sharing::SharingCloud(cloud.clone()));
        cx.set_global(crate::product::cloud::calls::AccountCloud(Arc::new(
            cloud.clone(),
        )));
        team_invitations::watch(cx);
        shared_notes::watch(cloud.clone(), cx);
        device_approval::watch(cloud.clone(), cx);
        let mut identities = cloud.identities();
        let identity_cloud = cloud.clone();
        cx.spawn(async move |this, cx| {
            loop {
                let identity = identities.borrow_and_update().clone();
                let _ = this.update(cx, |this, cx| {
                    let status = identity_cloud.clone();
                    this.automation.update(cx, |automation, cx| {
                        automation.set_account(
                            identity.account_id.clone(),
                            Some(Arc::new(move || status.sync_status().last_sync_at_ms)),
                            cx,
                        )
                    });
                    this.workspace.update(cx, |view, cx| {
                        view.set_viewer(identity.account_id.clone(), cx);
                        view.set_automation_client(None, cx);
                    });
                    if let Some(product) = &this.product {
                        product.update(cx, |view, cx| view.set_scope(identity_cloud.scope(), cx));
                    }
                    identity_cloud.publish_account(cx);
                    cx.notify();
                });
                if identity.account_id.is_some() {
                    let client = identity_cloud.automation_client().await.ok();
                    let _ = this.update(cx, |this, cx| {
                        this.workspace
                            .update(cx, |view, cx| view.set_automation_client(client, cx))
                    });
                }
                if identities.changed().await.is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            loop {
                let paused = this
                    .update(cx, |this, _| this.writers_paused || this.closing)
                    .unwrap_or(true);
                if !paused {
                    let result = cloud.refresh(false).await;
                    let _ = this.update(cx, |_, cx| cloud.publish_account(cx));
                    if let Err(error) = result {
                        let _ = this.update(cx, |this, cx| {
                            this.status(&format!("Account refresh: {error}"), cx)
                        });
                    }
                }
                cx.background_executor()
                    .timer(Duration::from_secs(60))
                    .await;
                if this.upgrade().is_none() {
                    break;
                }
            }
        })
        .detach();
    }

    fn save_pins(&mut self, routes: Arc<[Route]>, cx: &mut Context<Self>) {
        if self.pins_saving {
            self.pins_pending = Some(routes);
            return;
        }
        let Some(base) = self.pins_revision.clone() else {
            return;
        };
        let shipping = crate::shipping_store::ShippingStore::get(cx);
        let reply = match pins::save(&self.runtime, shipping, base, routes.clone()) {
            Ok(reply) => reply,
            Err(error) => {
                self.pins_pending = Some(routes);
                self.status(&format!("Pins not saved: {error}"), cx);
                return;
            }
        };
        self.pins_saving = true;
        cx.spawn(async move |this, cx| {
            let result = reply.receive().await;
            let _ = this.update(cx, |this, cx| {
                this.pins_saving = false;
                match result {
                    Ok(revision) => {
                        this.pins_revision = Some(revision);
                        if let Some(routes) = this.pins_pending.take() {
                            this.save_pins(routes, cx);
                        }
                    }
                    Err(error) => {
                        this.pins_pending.get_or_insert(routes);
                        this.status(
                            &format!("Pins not saved; in-memory tabs retained: {error}"),
                            cx,
                        );
                    }
                }
            });
        })
        .detach();
    }

    /// Shipping's audio retention sweep: once after startup recovery, then every minute.
    fn start_retention(
        &self,
        vault: PathBuf,
        activities: retention::Activities,
        cx: &mut Context<Self>,
    ) {
        let runtime = self.runtime.clone();
        cx.spawn(async move |this, cx| {
            loop {
                let (work, vault, activities) =
                    (runtime.clone(), vault.clone(), activities.clone());
                let reply = runtime.service(move |_| async move {
                    retention::sweep(&work, vault, activities).await
                });
                match async { reply?.receive().await }.await {
                    Err(ServiceError::Closed) => break,
                    Err(error) => tracing::warn!(%error, "audio retention sweep failed"),
                    Ok(()) => {}
                }
                cx.background_executor().timer(retention::SWEEP_EVERY).await;
                if this.update(cx, |_, _| ()).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// An interrupted recording that recovery couldn't finish blocks new recordings in its note
    /// until a later launch recovers it or the user discards it here.
    fn offer_discard(
        &self,
        capture: recovery::InterruptedCapture,
        vault: PathBuf,
        cx: &mut Context<Self>,
    ) {
        let error = capture
            .result
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        let title = if capture.title.is_empty() {
            "Untitled"
        } else {
            &capture.title
        };
        let runtime = self.runtime.clone();
        let (session, transcript, retained) =
            (capture.session, capture.transcript, capture.retained);
        toasts::show(
            toasts::Toast::warning(format!(
                "The interrupted recording in “{title}” could not be recovered"
            ))
            .id(format!("capture-recovery-{}", session.0))
            .description(format!(
                "{error} Anarlog retries at the next launch, or you can discard its unrecovered audio."
            ))
            .sticky()
            .action("Discard", move |_, cx| {
                let work = runtime.clone();
                let (vault, session, transcript) =
                    (vault.clone(), session.clone(), transcript.clone());
                let reply = runtime.service(move |_| async move {
                    recovery::discard(&work, &vault, session, transcript, retained).await
                });
                cx.spawn(async move |cx| {
                    if let Err(error) = async { reply?.receive().await }.await {
                        cx.update(|cx| {
                            toasts::error(
                                format!("The interrupted recording could not be discarded: {error}"),
                                cx,
                            )
                        });
                    }
                })
                .detach();
            }),
            cx,
        );
    }

    /// App errors appear as shipping's error toasts.
    fn status(&mut self, message: &str, cx: &mut Context<Self>) {
        toasts::error(message.to_owned(), cx);
    }

    fn workspace_event(
        &mut self,
        event: &WorkspaceEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            WorkspaceEvent::AttachFile(id) => {
                if let Some(editor) = &self.editor
                    && &editor.read(cx).init.session_id == id
                {
                    editor.update(cx, |editor, cx| editor.choose_attachment(cx));
                }
            }
            WorkspaceEvent::SessionsDeleted(ids) => {
                if self
                    .editor
                    .as_ref()
                    .is_some_and(|editor| ids.contains(&editor.read(cx).init.session_id))
                {
                    self.editor_subscriptions.clear();
                    self.editor = None;
                }
                self.close_note_windows(ids, cx);
                cx.notify();
            }
            WorkspaceEvent::OpenEditor {
                session_id,
                document,
            } => self.open_editor(session_id.clone(), document.clone(), window, cx),
            WorkspaceEvent::Meeting(intent) => self.open_meeting(intent.clone(), window, cx),
            WorkspaceEvent::Product(route) => self.open_product(route.clone(), window, cx),
        }
    }

    fn open_editor(
        &mut self,
        session_id: SessionId,
        document: DocumentSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &self.editor
            && editor.read(cx).init.session_id == session_id
        {
            self.workspace.update(cx, |workspace, cx| {
                workspace.set_editor_content(&session_id, editor.clone().into(), cx)
            });
            return;
        }
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.read(cx).is_dirty())
        {
            self.status(
                "Unsaved editor retained. Save before opening another note.",
                cx,
            );
            return;
        }
        self.pending_product = None;
        let return_focus = self.workspace.read(cx).focus_handle(cx);
        let editor = cx.new(|cx| {
            let mut editor = EditorPane::new(
                LaneContext {
                    runtime: self.runtime.clone(),
                },
                EditorInit {
                    session_id: session_id.clone(),
                    document,
                    return_focus,
                },
                window,
                cx,
            );
            editor.configure_attachments(self.editor_vault.clone(), cx);
            let records = cx.new(|cx| {
                crate::workspace::MeetingRecords::new(
                    self.runtime.clone(),
                    crate::editor::AttachmentService::new(
                        self.runtime.clone(),
                        self.editor_vault.clone(),
                    ),
                    session_id.clone(),
                    cx,
                )
            });
            editor.set_footer(Some(records.into()), cx);
            editor
        });
        self.editor_subscriptions = vec![
            cx.subscribe(&editor, |this, editor, event, cx| match event {
                EditorEvent::Dirty { session_id, dirty } => {
                    this.workspace.update(cx, |workspace, cx| {
                        workspace.set_session_dirty(session_id.clone(), *dirty, cx)
                    })
                }
                EditorEvent::Saved(snapshot) => {
                    let dirty = editor.read(cx).is_dirty();
                    this.workspace.update(cx, |workspace, cx| {
                        workspace.set_session_dirty(snapshot.session_id.clone(), dirty, cx)
                    });
                }
                EditorEvent::SaveFailed { session_id, .. } => {
                    this.workspace.update(cx, |workspace, cx| {
                        workspace.set_session_dirty(session_id.clone(), true, cx)
                    });
                }
                EditorEvent::MentionHuman(id) => this.workspace.update(cx, |workspace, cx| {
                    workspace.open_route(Route::Human(id.0.clone()), false, cx)
                }),
                EditorEvent::OpenLink(url) => {
                    if url.starts_with("https://")
                        || url.starts_with("http://")
                        || url.starts_with("mailto:")
                    {
                        cx.open_url(url);
                    } else {
                        this.status(
                            "This link scheme is not supported by the native preview.",
                            cx,
                        );
                    }
                }
                EditorEvent::OpenAttachment(id) => {
                    let attachments = crate::editor::AttachmentService::new(
                        this.runtime.clone(),
                        this.editor_vault.clone(),
                    );
                    let session = editor.read(cx).init.session_id.clone();
                    let id = id.0.to_string();
                    let runtime = this.runtime.clone();
                    cx.spawn(async move |this, cx| {
                        let result = async {
                            let attachment = attachments
                                .resolve(session, id, desktop_runtime::CancellationToken::new())
                                .await
                                .map_err(|error| ServiceError::Failed(error.into()))?;
                            runtime
                                .service(move |_| async move {
                                    tokio::task::spawn_blocking(move || {
                                        crate::platform::os::open(attachment.path.as_ref())
                                    })
                                    .await
                                    .map_err(|error| {
                                        ServiceError::Failed(error.to_string().into())
                                    })?
                                    .map_err(|error| ServiceError::Failed(error.to_string().into()))
                                })?
                                .receive()
                                .await
                        }
                        .await;
                        if let Err(error) = result {
                            let _ = this.update(cx, |this, cx| this.status(&error.to_string(), cx));
                        }
                    })
                    .detach();
                }
            }),
            cx.subscribe(&editor, |this, _, event, cx| match event {
                EditorRequest::OpenSession(id) => this.workspace.update(cx, |workspace, cx| {
                    workspace.open_session(SessionId(id.clone().into()), cx)
                }),
                EditorRequest::OpenOrganization(id) => {
                    this.workspace.update(cx, |workspace, cx| {
                        workspace.open_route(Route::Organization(id.clone().into()), false, cx)
                    })
                }
                EditorRequest::OpenTask(route) => this.workspace.update(cx, |workspace, cx| {
                    workspace.open_route(route.clone(), true, cx)
                }),
                EditorRequest::MentionSearch { .. } => {}
            }),
        ];
        self.workspace.update(cx, |workspace, cx| {
            workspace.set_editor_content(&session_id, editor.clone().into(), cx)
        });
        self.editor = Some(editor);
        self.meeting_open = false;
        cx.notify();
    }

    pub(super) fn sync_floating_bar(&mut self, cx: &mut Context<Self>) {
        let active_session = self
            .meeting_host
            .live()
            .read(cx)
            .current()
            .filter(|live| live.mode == crate::meeting::live::SessionMode::Active)
            .map(|live| &live.session);
        if active_session != self.floating_pinned_session.as_ref() {
            self.floating_pinned_session = None;
        }
        let enabled = self.notifications.floating_bar_enabled()
            && (!self.main_window_active
                || (active_session.is_some()
                    && active_session == self.floating_pinned_session.as_ref()));
        if let Err(error) = self
            .meeting_host
            .sync_floating(enabled, self.services.as_ref(), cx)
        {
            self.status(&error.to_string(), cx);
        }
    }

    /// Live recording markers and the floating bar follow the live meeting.
    fn live_changed(&mut self, cx: &mut Context<Self>) {
        for (session, active) in self.meeting_host.recording_changes(cx) {
            self.workspace.update(cx, |workspace, cx| {
                workspace.set_recording(session, active, cx)
            });
        }
        self.sync_floating_bar(cx);
    }

    fn open_meeting(&mut self, intent: MeetingIntent, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(intent, MeetingIntent::Start { .. })
            && (self.writers_paused || self.closing || self.drained || self.shutdown_begun())
        {
            self.status("Finish pending work before starting capture.", cx);
            return;
        }
        let Some(services) = self.services.clone() else {
            self.status("Meeting services are still starting or recovery failed. Retry after resolving the startup error.", cx);
            return;
        };
        self.pending_product = None;
        // Shipping records inside the note itself; only an explicit open shows the meeting pane.
        let reply = match &intent {
            MeetingIntent::Start { session_id } => Some(host::start(&services, session_id.clone())),
            MeetingIntent::Stop => Some(host::stop(&services)),
            MeetingIntent::Open(_) => None,
        };
        if let Some(reply) = reply {
            cx.spawn(async move |this, cx| {
                if let Err(error) = host::acknowledged(reply).await {
                    let _ = this.update(cx, |this, cx| this.status(&error.to_string(), cx));
                }
            })
            .detach();
            return;
        }
        let meeting = cx.new(|cx| {
            MeetingPane::new(
                LaneContext {
                    runtime: self.runtime.clone(),
                },
                intent,
                window,
                cx,
            )
        });
        self.meeting_subscription = Some(cx.subscribe(&meeting, |this, _, event, cx| {
            match event {
                MeetingEvent::NoteEnhanced(session) => {
                    crate::meeting::automation::summary_ready(session, "", cx);
                    this.workspace.update(cx, |workspace, cx| {
                        workspace.run_automations(
                            crate::workspace::automation_runner::Trigger::NoteEnhanced,
                            session.clone(),
                            cx,
                        );
                    })
                }
                MeetingEvent::Recording { session_id, active } => {
                    this.workspace.update(cx, |workspace, cx| {
                        workspace.set_recording(session_id.clone(), *active, cx)
                    })
                }
                MeetingEvent::OpenSession(id) => this
                    .workspace
                    .update(cx, |workspace, cx| workspace.open_session(id.clone(), cx)),
                MeetingEvent::Failed(error) => this.status(&error.to_string(), cx),
            }
        }));
        self.meeting = Some(meeting);
        self.meeting_open = true;
        cx.notify();
    }

    fn open_product(&mut self, route: ProductRoute, window: &mut Window, cx: &mut Context<Self>) {
        self.open_product_with_section(route, None, window, cx);
    }

    fn open_product_with_section(
        &mut self,
        route: ProductRoute,
        section: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(native) = &self.native else {
            if let Some(error) = &self.startup_error {
                self.status(&format!("Native services failed to start: {error}"), cx);
            } else {
                let workspace_route = self.workspace.read(cx).current_route();
                let settings_section = section.or_else(|| match (&route, &workspace_route) {
                    (ProductRoute::Settings, Route::Settings(section)) => Some(section.to_string()),
                    _ => None,
                });
                self.pending_product = Some(PendingProduct {
                    route,
                    workspace_route,
                    settings_section,
                });
            }
            return;
        };
        self.pending_product = None;
        let local = native.local.clone();
        let cloud = native.cloud.service.clone();
        let scope = cloud.as_ref().map(|c| c.scope()).unwrap_or_default();
        if self
            .product
            .as_ref()
            .is_some_and(|product| !product.update(cx, |product, cx| product.can_close(cx)))
        {
            return;
        }
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.read(cx).is_dirty())
        {
            self.status("Save the note before opening this service.", cx);
            return;
        }
        if !self
            .workspace
            .update(cx, |workspace, cx| workspace.can_close(cx))
        {
            return;
        }
        let general_settings = matches!(route, ProductRoute::Settings);
        let settings_section = section.or_else(|| match self.workspace.read(cx).current_route() {
            Route::Settings(section) => Some(section.to_string()),
            _ => None,
        });
        let product = cx.new(|cx| {
            ProductPane::with_services(
                LaneContext {
                    runtime: self.runtime.clone(),
                },
                route,
                local,
                cloud,
                scope,
                window,
                cx,
            )
        });
        if general_settings && let Some(section) = settings_section {
            product.update(cx, |product, cx| product.navigate_section(&section, cx));
        }
        self.product_subscriptions = vec![
            cx.observe(&product, |_, _, cx| cx.notify()),
            cx.subscribe(&product, |this, _, event, cx| match event {
                ProductEvent::NavigateWorkspace => this.leave_product(Route::Empty, cx),
                ProductEvent::OpenSession(id) => this.leave_product(Route::Session(id.clone()), cx),
                ProductEvent::Failed(error) => this.status(&error.to_string(), cx),
            }),
            cx.subscribe(&product, |this, _, event: &OpenWorkspaceSection, cx| {
                let route = match event.0 {
                    "contacts" => Route::Contacts,
                    "folders" => Route::Folders,
                    "templates" => Route::Templates,
                    "automations" => Route::Automations,
                    "calendar" => Route::Calendar,
                    // Shipping opens release notes in a new tab beside Settings.
                    "changelog" => return this.leave_product_to(Route::Changelog, true, cx),
                    _ => Route::settings(event.0),
                };
                this.leave_product(route, cx);
            }),
        ];
        self.product = Some(product);
        cx.notify();
    }

    /// Like shipping's `/app` route guard: a profile that has not finished onboarding opens it.
    fn open_onboarding_if_needed(&mut self, cx: &mut Context<Self>) {
        let runtime = self.runtime.clone();
        let window = self.window;
        cx.spawn(async move |this, cx| {
            let key = crate::product::onboarding::NEEDED_KEY.into();
            let stored = crate::product::local::data::setting_value(&runtime, key).await;
            // An unreadable flag skips onboarding, as shipping's `getOnboardingNeeded` does.
            if stored.is_ok_and(|value| crate::product::onboarding::needed(value.as_ref())) {
                let _ = cx.update_window(window, |_, window, cx| {
                    this.update(cx, |this, cx| {
                        this.open_product(ProductRoute::Onboarding, window, cx)
                    })
                });
            }
        })
        .detach();
    }

    fn leave_product(&mut self, route: Route, cx: &mut Context<Self>) {
        self.leave_product_to(route, false, cx)
    }

    fn leave_product_to(&mut self, route: Route, new_slot: bool, cx: &mut Context<Self>) {
        if self
            .product
            .as_ref()
            .is_some_and(|product| !product.update(cx, |product, cx| product.can_close(cx)))
        {
            return;
        }
        self.pending_product = None;
        self.product = None;
        self.product_subscriptions.clear();
        self.workspace.update(cx, |workspace, cx| {
            workspace.open_route(route, new_slot, cx)
        });
        let view = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = view.update(cx, |this, cx| {
                if this.product.is_none() {
                    let focus = this.workspace.read(cx).focus_handle(cx);
                    let _ = this
                        .window
                        .update(cx, |_, window, cx| focus.focus(window, cx));
                }
            });
        });
        cx.notify();
    }

    pub fn request_quit(&mut self, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        // A runtime that closed (or never started) has nothing left to drain.
        if self.drained
            || matches!(
                *self.runtime.state().borrow(),
                desktop_runtime::RuntimeState::Closed(_)
            )
        {
            crate::telemetry::track("app_exit_requested", &[]);
            quit_drained(cx);
            return;
        }
        if self.pins_saving || self.pins_pending.is_some() {
            if !self.pins_saving
                && let Some(routes) = self.pins_pending.take()
            {
                self.save_pins(routes, cx);
            }
            self.status(
                "Pinned tabs are still saving; retry quit after the save completes.",
                cx,
            );
            return;
        }
        if self
            .product
            .as_ref()
            .is_some_and(|product| !product.update(cx, |product, cx| product.can_close(cx)))
        {
            self.status("Save or restore product edits before quitting.", cx);
            return;
        }
        let flush = self
            .editor
            .as_ref()
            .filter(|editor| editor.read(cx).is_dirty())
            .map(|editor| editor.update(cx, |editor, _| editor.flush()));
        let note = self.workspace.read(cx).note().clone();
        let summaries = note.update(cx, |note, cx| note.flush_summaries(true, cx));
        let notes = self
            .note_windows
            .values()
            .filter_map(|window| window.update(cx, |view, _, cx| view.flush(true, cx)).ok())
            .collect::<Vec<_>>();
        let pending = self
            .workspace
            .update(cx, |workspace, cx| workspace.save_pending(cx));
        self.closing = true;
        crate::telemetry::track("app_exit_requested", &[]);
        // Undo windows end at quit so deleted notes do not keep their folders.
        self.workspace
            .update(cx, |workspace, cx| workspace.confirm_pending_deletions(cx));
        if let Some(editor) = &self.editor {
            editor.update(cx, |editor, cx| editor.set_read_only(true, cx));
        }
        toasts::show(
            toasts::Toast::loading("Finishing saves and capture before quitting…").id("quit"),
            cx,
        );
        let services = self.services.clone();
        cx.spawn(async move |this, cx| {
            let result: desktop_runtime::Result<()> = async {
                // Every started save finishes before the first error stops the quit: dropping
                // the template save would cancel it, and its failed edits restore only from it.
                let mut saved = Vec::new();
                for flush in flush.into_iter().chain(summaries) { saved.push(flush.await.map_err(|_| ServiceError::Closed).and_then(|result| result)); }
                for flush in notes { saved.push(flush.await); }
                saved.push(pending.await);
                saved.into_iter().collect::<desktop_runtime::Result<()>>()?;
                if let Some(services) = services {
                    services.playback.send(playback::Command::Pause)?;
                    let update = services.capture.take_update(u64::MAX);
                    if matches!(update.phase, Phase::Loading | Phase::Listening | Phase::Finalizing) {
                        services.capture.stop()?.await.map_err(|_| ServiceError::Closed)??;
                        let deadline = std::time::Instant::now() + Duration::from_secs(30);
                        loop {
                            let update = services.capture.take_update(u64::MAX);
                            match update.phase {
                                Phase::Idle => break,
                                Phase::Failed => return Err(update.error.unwrap_or(ServiceError::Closed)),
                                _ if std::time::Instant::now() >= deadline => return Err(ServiceError::Failed("Capture is still finalizing; retry after it completes.".into())),
                                _ => { cx.background_executor().timer(Duration::from_millis(50)).await; }
                            }
                        }
                    }
                }
                let runtime = this.update(cx, |this, cx| {
                    if !this.workspace.update(cx, |workspace, cx| workspace.can_close(cx)) {
                        return Err(ServiceError::Failed("Save or restore the title and finish pending work before quitting.".into()));
                    }
                    Ok(this.runtime.clone())
                }).map_err(|_| ServiceError::Closed)??;
                let drain = cx.update(|cx| runtime_bridge::drain_before_quit(runtime, cx));
                drain.await
            }.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(()) => {
                    this.closing = false;
                    this.drained = true;
                    quit_drained(cx);
                }
                Err(error) if this.shutdown_begun() => this.finish_drain(error, cx),
                Err(error) => {
                    this.closing = false;
                    // A storage move holds its pause until the relaunch.
                    if !this.writers_paused {
                        if let Some(editor) = &this.editor { editor.update(cx, |editor, cx| editor.set_read_only(false, cx)); }
                        note.update(cx, |note, cx| note.resume_summaries(cx));
                        for window in this.note_windows.values() { let _ = window.update(cx, |view, _, cx| view.resume(cx)); }
                    }
                    toasts::dismiss("quit", cx);
                    this.status(&format!("Quit paused: {error}"), cx);
                }
            });
        }).detach();
    }

    /// The runtime stopped admitting work, so nothing can save again in this process.
    pub(super) fn shutdown_begun(&self) -> bool {
        !matches!(
            *self.runtime.state().borrow(),
            desktop_runtime::RuntimeState::Starting | desktop_runtime::RuntimeState::Ready
        )
    }

    /// Notes stay read-only once the runtime began shutting down. The drain keeps running and
    /// the app quits when it finishes; if it fails, the next quit request does not wait again.
    fn finish_drain(&mut self, error: ServiceError, cx: &mut Context<Self>) {
        self.status(&format!("Quit paused: {error}"), cx);
        let runtime = self.runtime.clone();
        cx.spawn(async move |this, cx| {
            let result = runtime.shutdown().await;
            let _ = this.update(cx, |this, cx| {
                this.closing = false;
                this.drained = true;
                match result {
                    Ok(()) => quit_drained(cx),
                    Err(error) => {
                        toasts::dismiss("quit", cx);
                        this.status(&format!("Quit paused: {error}"), cx);
                    }
                }
            });
        })
        .detach();
    }
}

impl Render for ApplicationView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let _ = &self.subscriptions;
        self.sync_notice(window, cx);
        if let Some(gate) = startup::gate(window, cx) {
            return gate;
        }
        if let Some(gate) = lock::gate(window, cx) {
            return gate;
        }
        // Shipping's `/app/instruction` route replaces the app while the browser hand-off runs.
        if self.instruction.read(cx).is_active() {
            return self.instruction_root(cx);
        }
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(colors.background)
            .text_color(colors.foreground)
            .capture_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if this.closing || this.writers_paused {
                    cx.stop_propagation();
                    return;
                }
                if this.chrome_key(event, window, cx) {
                    cx.stop_propagation();
                }
            }))
            .when(!cfg!(target_os = "macos"), |view| {
                view.child(self.title_bar(window, cx))
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .when_some(
                        self.product
                            .clone()
                            .filter(|_| !self.closing && !self.writers_paused),
                        |view, product| view.child(product),
                    )
                    .when(
                        self.product.is_none() && !self.closing && !self.writers_paused,
                        |view| {
                            view.child(
                                div()
                                    .flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(self.workspace.clone()),
                            )
                            .when_some(
                                self.meeting.clone().filter(|_| self.meeting_open),
                                |view, meeting| {
                                    view.child(
                                        div()
                                            .w(px(420.))
                                            .h_full()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .id("hide-meeting")
                                                    .a11y_button("Hide transcript")
                                                    .p_2()
                                                    .cursor_pointer()
                                                    .child("Hide transcript")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.meeting_open = false;
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(div().flex_1().min_h_0().child(meeting)),
                                    )
                                },
                            )
                        },
                    ),
            )
            .child(self.toasts.clone())
            .when_some(self.title_menu(window, cx), |view, menu| view.child(menu))
            .child(self.trial_dialogs.clone())
    }
}

/// Quits after the drain, starting a requested restart first on macOS, where quitting ends
/// the process before `main` regains control.
fn quit_drained(cx: &mut gpui::App) {
    #[cfg(target_os = "macos")]
    if let Err(error) = crate::platform::windows::run_requested_restart() {
        tracing::error!(%error, "the app could not relaunch");
    }
    cx.quit();
}
