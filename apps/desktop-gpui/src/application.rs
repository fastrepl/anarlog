mod automation;
mod chrome;
mod cloud_api;
mod enterprise_capture;
pub mod instruction;
pub(crate) mod lifecycle;
pub mod lock;
mod meeting_schedule;
mod note_windows;
mod notifications;
mod release_notes;
pub mod team_invitations;
pub mod toasts;

use std::{path::PathBuf, sync::Arc, time::Duration};

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
        recovery,
    },
    product::{
        OpenWorkspaceSection, ProductPane,
        cloud::{auth::SecureAuthProvider, host::NativeHost},
        local::HostEffects,
    },
    runtime_bridge,
    ui::theme::theme,
    workspace::{WorkspaceAction, WorkspaceView, navigation::Route, pins},
};

struct ProfileStorage(PathBuf);

impl anlg_storage::StorageRuntime for ProfileStorage {
    fn global_base(&self) -> Result<PathBuf, anlg_storage::Error> {
        Ok(self.0.clone())
    }

    fn vault_base(&self) -> Result<PathBuf, anlg_storage::Error> {
        Ok(self.0.join("vault"))
    }
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
    automation: Entity<crate::meeting::automation::MeetingAutomation>,
    dictation: Option<Entity<crate::dictation::Dictation>>,
    product: Option<Entity<ProductPane>>,
    trial_dialogs: Entity<crate::product::trial::TrialDialogs>,
    instruction: Entity<instruction::Instruction>,
    services: Option<MeetingServices>,
    native: Option<NativeServices>,
    #[cfg(target_os = "linux")]
    native_starting: bool,
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

    #[cfg(target_os = "linux")]
    pub fn accepts_launch_links(&self) -> bool {
        !self.native_starting && !self.closing
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
        profile: PathBuf,
        storage_pointer: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
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
            cx.on_focus_lost(window, |this, window, cx| {
                if this.product.is_none() && !this.closing && !this.writers_paused {
                    cx.focus_view(&this.workspace, window);
                    cx.notify();
                }
            }),
            cx.subscribe_in(&workspace, window, |this, _, event, window, cx| {
                this.workspace_event(event, window, cx)
            }),
            cx.subscribe(&workspace, |this, _, event, cx| match event {
                WorkspaceAction::SidebarChanged => cx.notify(),
                WorkspaceAction::OpenShared(id) => this.mount_shared(id.to_string(), false, cx),
                WorkspaceAction::NewNoteWindow => this.open_note_window(None, cx),
                WorkspaceAction::OpenNoteWindow(id) => this.open_note_window(Some(id.clone()), cx),
                WorkspaceAction::PinnedChanged(routes) => this.save_pins(routes.clone(), cx),
                WorkspaceAction::OpenFloatingPanel => this.meeting_host.show_floating(cx),
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
            let result = match pins::load(&pins_runtime) {
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
        let editor_vault = profile.join("vault");
        let tray_settings = profile.join("tray.json");
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
                        crate::platform::windows::request_restart();
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
            let native = NativeServices::new(
                startup_runtime.clone(),
                profile.clone(),
                storage_pointer,
                effects.clone(),
                host,
            )
            .await?;
            let storage = Arc::new(ProfileStorage(profile));
            let vault = storage.0.join("vault");
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
            let reports = startup_runtime
                .service(move |_| async move {
                    recovery::recover_startup(&recovery_runtime, vault, activities, resolver).await
                })?
                .receive()
                .await?;
            let warnings = reports
                .into_iter()
                .filter_map(|report| {
                    report
                        .result
                        .err()
                        .map(|error| format!("{}: {error}", report.session.0))
                })
                .collect::<Vec<_>>();
            let services = MeetingServices {
                capture,
                playback: Playback::spawn()?,
                vault: audio_vault,
            };
            effects.attach(services.clone());
            let capture = services.capture.clone();
            let ai = native
                .providers
                .ai_services(Arc::new(move |session| capture.is_active(session)))?;
            Ok((services, native, ai, warnings))
        });
        cx.spawn(async move |this, cx| {
            let result = startup.await;
            #[cfg(target_os = "linux")]
            let _ = this.update(cx, |this, _| this.native_starting = false);
            let _ = this.update(cx, |this, cx| match result {
                Ok((services, native, ai, warnings)) => {
                    cx.set_global(services.clone());
                    cx.set_global(ai);
                    cx.set_global(native.providers.clone());
                    if let Some(dictation) = &this.dictation {
                        let voice = dictation.read(cx).voice(native.providers.clone());
                        cx.set_global(voice);
                    }
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
                    this.start_schedule(cx);
                    this.watch_preferences(cx);
                    this.start_lifecycle(cx);
                    this.watch_updates(cx);
                    this.open_onboarding_if_needed(cx);
                    if !warnings.is_empty() {
                        toasts::show(
                            toasts::Toast::warning(format!(
                                "Interrupted capture recovery needs attention: {}",
                                warnings.join("; ")
                            )),
                            cx,
                        );
                    }
                }
                Err(error) => this.status(&format!("Meeting startup failed: {error}"), cx),
            });
        })
        .detach();
        let trial_dialogs =
            cx.new(|cx| crate::product::trial::TrialDialogs::new(runtime.clone(), cx));
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
            automation,
            dictation,
            product: None,
            trial_dialogs,
            instruction,
            services: None,
            native: None,
            #[cfg(target_os = "linux")]
            native_starting: true,
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
                            let result = match url::Url::parse(&raw) {
                                Ok(url)
                                    if matches!(
                                        link,
                                        desktop_runtime::deeplink::DeepLink::Auth { .. }
                                    ) =>
                                {
                                    cloud.handle_deep_link(url).await.map(|_| ())
                                }
                                Ok(_) => cloud.refresh().await,
                                Err(_) => Err(ServiceError::Failed("Invalid deep link".into())),
                            };
                            let _ = this.update(cx, |this, cx| match result {
                                Ok(_) => {
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
                self.capture_session = update.session
            }
            Phase::Idle => {
                if let Some(session) = self.capture_session.take() {
                    lock::capture_ended(&session, cx);
                    crate::meeting::automation::capture_completed(cx);
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
                    lock::capture_ended(&session, cx);
                }
            }
        }
        self.schedule_capture_phase(cx);
    }

    pub(super) fn resume_writers(&mut self, cx: &mut Context<Self>) {
        self.writers_paused = false;
        if let Some(editor) = &self.editor {
            editor.update(cx, |editor, cx| editor.set_read_only(false, cx));
        }
        for window in self.note_windows.values() {
            let _ = window.update(cx, |view, _, cx| view.resume(cx));
        }
        cx.notify();
    }

    fn flush_drafts(
        &mut self,
        cx: &mut Context<Self>,
    ) -> BoxFuture<'static, desktop_runtime::Result<()>> {
        if self.pins_saving
            || self.pins_pending.is_some()
            || !self.workspace.update(cx, |view, cx| view.can_close(cx))
        {
            return Box::pin(async { Err(ServiceError::Busy) });
        }
        let flush = self
            .editor
            .as_ref()
            .map(|editor| editor.update(cx, |editor, _| editor.flush()));
        let runtime = self.runtime.clone();
        let notes = self
            .note_windows
            .values()
            .filter_map(|window| window.update(cx, |view, _, cx| view.flush(false, cx)).ok())
            .collect::<Vec<_>>();
        Box::pin(async move {
            if let Some(flush) = flush {
                flush.await.map_err(|_| ServiceError::Closed)??;
            }
            for flush in notes {
                flush.await?;
            }
            runtime.flush()?.receive().await
        })
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
        if self.services.as_ref().is_some_and(|s| {
            matches!(
                s.capture.take_update(u64::MAX).phase,
                Phase::Loading | Phase::Listening | Phase::Finalizing
            )
        }) {
            return Box::pin(async { Err(ServiceError::Busy) });
        }
        self.writers_paused = true;
        if let Some(editor) = &self.editor {
            editor.update(cx, |editor, cx| editor.set_read_only(true, cx));
        }
        let flush = self.flush_drafts(cx);
        let notes = self
            .note_windows
            .values()
            .filter_map(|window| window.update(cx, |view, _, cx| view.flush(true, cx)).ok())
            .collect::<Vec<_>>();
        let cloud = self.native.as_ref().and_then(|n| n.cloud.service.clone());
        let runtime = self.runtime.clone();
        Box::pin(async move {
            flush.await?;
            for note in notes {
                note.await?;
            }
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
                    let result = cloud.refresh().await;
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
        let reply = match pins::save(&self.runtime, base, routes.clone()) {
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

    /// Live recording markers and the floating bar follow the live meeting.
    fn live_changed(&mut self, cx: &mut Context<Self>) {
        for (session, active) in self.meeting_host.recording_changes(cx) {
            self.workspace.update(cx, |workspace, cx| {
                workspace.set_recording(session, active, cx)
            });
        }
        let enabled = self.notifications.floating_bar_enabled();
        if let Err(error) = self
            .meeting_host
            .sync_floating(enabled, self.services.as_ref(), cx)
        {
            self.status(&error.to_string(), cx);
        }
    }

    fn open_meeting(&mut self, intent: MeetingIntent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(services) = self.services.clone() else {
            self.status("Meeting services are still starting or recovery failed. Retry after resolving the startup error.", cx);
            return;
        };
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
        let Some(native) = &self.native else {
            self.status("Native services are still starting.", cx);
            return;
        };
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
        if general_settings
            && let Route::Settings(section) = self.workspace.read(cx).current_route()
        {
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
        if self.drained {
            cx.quit();
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
        let notes = self
            .note_windows
            .values()
            .filter_map(|window| window.update(cx, |view, _, cx| view.flush(true, cx)).ok())
            .collect::<Vec<_>>();
        self.closing = true;
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
                if let Some(flush) = flush { flush.await.map_err(|_| ServiceError::Closed)??; }
                for flush in notes { flush.await?; }
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
            let _ = this.update(cx, |this, cx| {
                this.closing = false;
                match result {
                    Ok(()) => { this.drained = true; cx.quit(); }
                    Err(error) => {
                        if let Some(editor) = &this.editor { editor.update(cx, |editor, cx| editor.set_read_only(false, cx)); }
                        for window in this.note_windows.values() { let _ = window.update(cx, |view, _, cx| view.resume(cx)); }
                        toasts::dismiss("quit", cx);
                        this.status(&format!("Quit paused: {error}"), cx);
                    }
                }
            });
        }).detach();
    }
}

impl Render for ApplicationView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let _ = &self.subscriptions;
        self.sync_notice(window, cx);
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
