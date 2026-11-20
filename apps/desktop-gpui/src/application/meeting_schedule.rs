//! Hosts `meeting::schedule` in the application: the scheduled-meeting watch and timer, the
//! auto-stop confirmation and its prompt, and the disclosure and chat/screen capture that run
//! while a meeting is listening. Decisions live in `meeting::schedule`; this file only feeds
//! them the clock, the calendar, the capture phase and the meeting-app adapter.

use std::{collections::VecDeque, sync::Arc, time::Duration};

use desktop_runtime::{CancellationToken, Result, RuntimeHandle, SessionId};
use futures::future::Either;
use gpui::{Context, Task};
use serde_json::json;

use super::ApplicationView;
use crate::{
    meeting::{
        capture::Phase,
        host,
        model::failure,
        schedule::{
            apps::{self, MeetingApps, MicApp},
            auto_start::{self, Live, Meeting, Outcome, Scheduler, Step},
            auto_stop::{self, MicCheck, Pending, Prompts, Verdict},
            capture::{self, ChatState, Consent, Delivery},
            records,
        },
    },
    platform::notifications::Action,
    product::settings::Snapshot,
    workspace::navigation::Route,
};

const MAX_DISCLOSED: usize = 256;
const CALENDAR_SYNC_EVERY: Duration = Duration::from_secs(60);

#[derive(Default)]
pub(super) struct MeetingSchedule {
    preferences: Option<Arc<Snapshot>>,
    scheduler: Scheduler,
    timer: Option<Task<()>>,
    tasks: Vec<Task<()>>,
    started: bool,
    phase: Option<(Phase, Option<SessionId>)>,
    /// Apps found carrying the live call besides detection's own triggers.
    triggers: Vec<String>,
    pending: Option<Pending>,
    pending_task: Option<Task<()>>,
    prompts: Prompts,
    /// The listening session's disclosure and chat and screen capture.
    capture: Option<(SessionId, Vec<Task<()>>)>,
    disclosure: Option<Task<()>>,
    disclosed: VecDeque<SessionId>,
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

impl MeetingSchedule {
    fn flag(&self, key: &str, default: bool) -> bool {
        self.preferences
            .as_ref()
            .and_then(|snapshot| snapshot.get(key))
            .and_then(|setting| setting.value.as_bool())
            .unwrap_or(default)
    }

    fn prompts_enabled(&self) -> bool {
        self.flag("notification_recording", true) && !self.flag("notification_disabled", false)
    }
}

impl ApplicationView {
    fn live_phase(&self) -> (Phase, Option<SessionId>) {
        self.services
            .as_ref()
            .map_or((Phase::Idle, None), |services| {
                let update = services.capture.take_update(u64::MAX);
                (update.phase, update.session)
            })
    }

    fn auto_start_enabled(&self, cx: &Context<Self>) -> bool {
        crate::product::account::is_pro(cx)
            && self.schedule.flag("auto_start_scheduled_meetings", true)
    }

    fn meeting_apps(&self, cx: &Context<Self>) -> Option<Arc<dyn MeetingApps>> {
        apps::installed(cx)
    }

    /// Once native services are up: watch scheduled meetings, poll microphone holds near
    /// them, and (only with the real meeting-app adapter) keep calendars fresh.
    pub(super) fn start_schedule(&mut self, cx: &mut Context<Self>) {
        if self.schedule.started {
            return;
        }
        self.schedule.started = true;
        // Every meeting in the calendars' history outgrows a watch's 1,000 rows, so a small
        // watch signals changes and each change reads the meetings in full.
        let reply = self.runtime.watch_changes(
            "SELECT EXISTS(SELECT 1 FROM events) AS events, EXISTS(SELECT 1 FROM app_settings) AS settings"
                .into(),
            vec![],
        );
        let runtime = self.runtime.clone();
        self.schedule.tasks.push(cx.spawn(async move |this, cx| {
            let Ok(mut watch) = async { reply?.receive().await }.await else {
                return;
            };
            loop {
                if watch.terminal_error().is_some() || watch.errors.try_recv().is_ok() {
                    break;
                }
                watch.snapshots.borrow_and_update();
                let read = async {
                    runtime
                        .read_rows(auto_start::MEETINGS_SQL.into(), vec![])?
                        .receive()
                        .await
                };
                match read.await {
                    Ok(rows) => {
                        let meetings = rows.iter().map(Meeting::from_row).collect();
                        let updated = this.update(cx, |this, cx| {
                            this.schedule.scheduler.replace(meetings);
                            this.schedule_tick(cx);
                        });
                        if updated.is_err() {
                            break;
                        }
                    }
                    // The last schedule stays until the next change reads it again.
                    Err(error) => tracing::warn!(%error, "scheduled meetings could not be read"),
                }
                if watch.snapshots.changed().await.is_err() {
                    break;
                }
            }
            let _ = watch.unsubscribe().await;
        }));
        self.schedule.tasks.push(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(auto_start::MIC_HOLD_POLL)
                    .await;
                let Ok(apps) = this.update(cx, |this, cx| {
                    let watch = this
                        .schedule
                        .scheduler
                        .watches_mic(this.auto_start_enabled(cx), now_ms());
                    if !watch {
                        this.schedule.scheduler.clear_holds();
                    }
                    this.meeting_apps(cx).filter(|_| watch)
                }) else {
                    break;
                };
                let Some(apps) = apps else { continue };
                match apps.mic_apps().await {
                    Ok(mic) => {
                        let _ = this.update(cx, |this, _| {
                            this.schedule.scheduler.observe_mic(&mic, now_ms())
                        });
                    }
                    Err(error) => tracing::warn!(%error, "failed to read microphone usage"),
                }
            }
        }));
        if self.meeting_apps(cx).is_some() {
            // Shipping refreshes calendars every minute, which keeps the attendance snapshot
            // fresh enough to start on. Only the real app does: tests never read calendars.
            self.schedule.tasks.push(cx.spawn(async move |this, cx| {
                loop {
                    if this
                        .update(cx, |this, cx| {
                            this.workspace
                                .update(cx, |workspace, cx| workspace.sync_calendars(cx))
                        })
                        .is_err()
                    {
                        break;
                    }
                    cx.background_executor().timer(CALENDAR_SYNC_EVERY).await;
                }
            }));
        }
        self.schedule_tick(cx);
    }

    pub(super) fn schedule_preferences(&mut self, snapshot: Arc<Snapshot>, cx: &mut Context<Self>) {
        self.schedule.preferences = Some(snapshot);
        // Turning automatic stop off withdraws a stop it already has under way.
        if !self.schedule.flag("auto_stop_meetings", true) {
            self.schedule.pending = None;
            self.schedule.pending_task = None;
            if let (_, Some(session)) = self.live_phase() {
                self.schedule.prompts.cancel(&session);
            }
        }
        if self.schedule.started {
            self.schedule_tick(cx);
        }
    }

    /// Called every native tick: reacts to capture phase changes like shipping's listener
    /// store subscriptions.
    pub(super) fn schedule_capture_phase(&mut self, cx: &mut Context<Self>) {
        let current = self.live_phase();
        let previous = self.schedule.phase.replace(current.clone());
        if previous.as_ref() == Some(&current) {
            return;
        }
        let was_listening = previous
            .as_ref()
            .and_then(|(phase, session)| (*phase == Phase::Listening).then(|| session.clone()))
            .flatten();
        let listening = (current.0 == Phase::Listening)
            .then(|| current.1.clone())
            .flatten();
        if was_listening.is_some() && was_listening != listening {
            self.schedule.scheduler.recording_ended(now_ms());
            self.schedule.capture = None;
            self.schedule.pending = None;
            self.schedule.pending_task = None;
        }
        if let Some(session) = listening.filter(|session| was_listening.as_ref() != Some(session)) {
            // A new recording invalidates any earlier "Did your meeting end?" countdown.
            self.schedule.prompts.cancel(&session);
            self.schedule.triggers.clear();
            self.start_meeting_capture(session, cx);
        }
        if self.schedule.started {
            self.schedule_tick(cx);
        }
    }

    fn schedule_tick(&mut self, cx: &mut Context<Self>) {
        let enabled = self.auto_start_enabled(cx);
        let live = match self.live_phase().0 {
            Phase::Listening => Live::Recording,
            Phase::Loading | Phase::Finalizing => Live::Busy,
            Phase::Idle | Phase::Failed => Live::Idle,
        };
        let step = self.schedule.scheduler.tick(enabled, live, now_ms());
        self.schedule_step(step, cx);
    }

    fn schedule_step(&mut self, step: Step, cx: &mut Context<Self>) {
        match step {
            Step::Idle => {
                if !self.schedule.scheduler.starting() {
                    self.schedule.timer = None;
                }
            }
            Step::Wait(delay) => {
                self.schedule.timer = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(delay).await;
                    let _ = this.update(cx, |this, cx| this.schedule_tick(cx));
                }));
            }
            Step::Start(meeting) => {
                self.schedule.timer = None;
                cx.spawn(async move |this, cx| {
                    let outcome = Self::start_scheduled(this.clone(), meeting, cx).await;
                    let _ = this.update(cx, |this, cx| {
                        let outcome = outcome.unwrap_or_else(|error| {
                            tracing::error!(%error, "failed to auto-start scheduled meeting");
                            Outcome::Blocked
                        });
                        let step = this.schedule.scheduler.finished(outcome, now_ms());
                        this.schedule_step(step, cx);
                    });
                })
                .detach();
            }
        }
    }

    /// Shipping's `startScheduledMeeting` and `ScheduledSessionAutoStart`: every check is
    /// repeated right before the link opens and capture starts, so a last-second decline,
    /// cancellation or reschedule triggers neither.
    async fn start_scheduled(
        this: gpui::WeakEntity<Self>,
        meeting: Meeting,
        cx: &mut gpui::AsyncApp,
    ) -> Result<Outcome> {
        let closed = || desktop_runtime::ServiceError::Closed;
        let check = |this: &mut Self, cx: &mut Context<Self>| -> Option<Outcome> {
            // A storage move or quit holds writers paused; a new note would land in the old
            // profile.
            if !this.auto_start_enabled(cx)
                || this.services.is_none()
                || this.writers_paused
                || this.closing
            {
                return Some(Outcome::Blocked);
            }
            match this.live_phase().0 {
                Phase::Listening => Some(Outcome::Ignored),
                Phase::Loading | Phase::Finalizing => Some(Outcome::Blocked),
                _ => None,
            }
        };
        let (runtime, viewer) = match this.update(cx, |this, cx| {
            check(this, cx).map_or(Ok((this.runtime.clone(), this.viewer())), Err)
        }) {
            Ok(Ok(found)) => found,
            Ok(Err(outcome)) => return Ok(outcome),
            Err(_) => return Err(closed()),
        };
        let Some(current) = read_due(&runtime, &meeting.id).await? else {
            return Ok(Outcome::Ineligible);
        };
        if current.ignored {
            return Ok(Outcome::Ignored);
        }
        let session = crate::workspace::calendar::open_event(&runtime, current.id.clone(), viewer)?
            .receive()
            .await?;
        let Some(current) = read_due(&runtime, &current.id).await? else {
            return Ok(Outcome::Ineligible);
        };
        if current.ignored {
            return Ok(Outcome::Ignored);
        }
        if !session_eligible(&runtime, &session).await? {
            return Ok(Outcome::Ineligible);
        }
        this.update(cx, |this, cx| {
            if let Some(outcome) = check(this, cx) {
                return Ok(outcome);
            }
            if this.schedule.flag("auto_join_scheduled_meetings", false)
                && let Some(apps) = this.meeting_apps(cx)
                && let Err(error) = apps.open_url(&current.meeting_link)
            {
                tracing::warn!(%error, "failed to open the meeting link");
            }
            this.workspace.update(cx, |workspace, cx| {
                workspace.open_route(Route::Session(session.clone()), true, cx)
            });
            let Some(services) = this.services.clone() else {
                return Ok(Outcome::Blocked);
            };
            let reply = host::start(&services, session);
            cx.spawn(async move |this, cx| {
                if let Err(error) = host::acknowledged(reply).await {
                    let _ = this.update(cx, |this, cx| this.status(&error.to_string(), cx));
                }
            })
            .detach();
            Ok(Outcome::Started)
        })
        .map_err(|_| closed())?
    }

    /// Shipping's `triggerAppIds`: detection's triggers plus apps auto-stop found on the call.
    fn triggers(&self, cx: &gpui::App) -> Vec<String> {
        let mut triggers = crate::meeting::schedule::notices(cx)
            .map(|notices| notices.trigger_apps(cx))
            .unwrap_or_default();
        for id in &self.schedule.triggers {
            if !triggers.contains(id) {
                triggers.push(id.clone());
            }
        }
        triggers
    }

    fn viewer(&self) -> Option<Arc<str>> {
        self.native
            .as_ref()
            .and_then(|native| native.cloud.service.as_ref())
            .and_then(|cloud| cloud.scope().account_id)
    }

    /// Entry point for meeting detection: apps that started using the microphone. While a
    /// meeting is listening they become its triggers (shipping's `captureTriggerApps`).
    pub fn meeting_mic_started(&mut self, apps: &[MicApp], cx: &mut Context<Self>) {
        let (phase, session) = self.live_phase();
        if !matches!(phase, Phase::Listening | Phase::Loading) {
            return;
        }
        if apps.iter().any(|app| self.triggers(cx).contains(&app.id)) {
            self.schedule.pending = None;
            self.schedule.pending_task = None;
            if let Some(session) = &session {
                self.schedule.prompts.cancel(session);
            }
        }
        for app in apps {
            if !self.schedule.triggers.contains(&app.id) {
                self.schedule.triggers.push(app.id.clone());
            }
        }
        cx.notify();
    }

    /// Entry point for meeting detection: apps that stopped using the microphone.
    pub fn meeting_mic_stopped(&mut self, apps: &[MicApp], cx: &mut Context<Self>) {
        self.schedule.scheduler.mic_stopped(apps);
        if self.schedule.started {
            self.schedule_tick(cx);
        }
        if !self.schedule.flag("auto_stop_meetings", true) {
            return;
        }
        let (Phase::Listening, Some(session)) = self.live_phase() else {
            return;
        };
        if let Some(pending) = Pending::after_stop(
            session,
            &self.triggers(cx),
            apps,
            self.schedule.pending.as_ref(),
        ) {
            self.confirm_auto_stop(auto_stop::CONFIRM_DELAY, pending, cx);
        }
    }

    fn confirm_auto_stop(&mut self, delay: Duration, pending: Pending, cx: &mut Context<Self>) {
        let Some(apps) = self.meeting_apps(cx) else {
            return;
        };
        self.schedule.pending = Some(pending.clone());
        self.schedule.pending_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            let current = |this: &Self, cx: &gpui::App| {
                this.schedule.flag("auto_stop_meetings", true)
                    && this.schedule.pending.as_ref() == Some(&pending)
                    && this.live_phase() == (Phase::Listening, Some(pending.session.clone()))
                    && pending
                        .candidates
                        .iter()
                        .any(|id| this.triggers(cx).contains(id))
            };
            if !this
                .update(cx, |this, cx| current(this, cx))
                .unwrap_or(false)
            {
                return;
            }
            let mic = apps.mic_apps().await;
            let Ok(check) = this.update(cx, |this, cx| {
                current(this, cx)
                    .then(|| pending.check_mic(&this.triggers(cx), mic.as_deref().map_err(|_| ())))
            }) else {
                return;
            };
            match check {
                None | Some(MicCheck::StillActive | MicCheck::Unconfirmed) => {}
                Some(MicCheck::OnCall(on_call)) => {
                    let _ = this.update(cx, |this, cx| {
                        this.schedule.prompts.cancel(&pending.session);
                        this.meeting_mic_started(&on_call, cx);
                        this.schedule.pending = None;
                    });
                }
                Some(MicCheck::Inspect) => {
                    let timeout = cx
                        .background_executor()
                        .timer(auto_stop::ACCESSIBILITY_TIMEOUT);
                    let timeout = std::pin::pin!(timeout);
                    let inspections = match futures::future::select(apps.inspect(), timeout).await {
                        Either::Left((Ok(inspections), _)) => Some(inspections),
                        _ => None,
                    };
                    let _ = this.update(cx, |this, cx| {
                        if !current(this, cx) {
                            return;
                        }
                        match pending.decide(&this.triggers(cx), inspections.as_deref()) {
                            Verdict::Recheck => this.confirm_auto_stop(
                                auto_stop::CONFIRM_DELAY,
                                pending.clone(),
                                cx,
                            ),
                            Verdict::Prompt => {
                                this.schedule.pending = None;
                                this.show_meeting_ended_prompt(&pending, cx);
                            }
                            Verdict::Stop => {
                                this.schedule.pending = None;
                                this.stop_capture(cx);
                            }
                        }
                    });
                }
            }
        }));
    }

    fn show_meeting_ended_prompt(&mut self, pending: &Pending, cx: &mut Context<Self>) {
        let Some(notices) =
            crate::meeting::schedule::notices(cx).filter(|_| self.schedule.prompts_enabled())
        else {
            return;
        };
        let key = self.schedule.prompts.key(&pending.session);
        if let Err(error) = notices.show(auto_stop::prompt(key, pending), cx) {
            tracing::warn!(%error, "failed to show the meeting-ended prompt");
            self.schedule.prompts.cancel(&pending.session);
        }
    }

    /// Notification actions for keys this host posted. Returns whether the key was ours.
    pub fn schedule_notification_action(
        &mut self,
        key: &str,
        action: Action,
        cx: &mut Context<Self>,
    ) -> bool {
        if auto_stop::parse(key).is_none() {
            return false;
        }
        // Clicking the banner only opens the app; Stop and the countdown stop listening.
        // The countdown stops only while automatic stop is still on.
        let stops = match action {
            Action::Accept => true,
            Action::Timeout => self.schedule.flag("auto_stop_meetings", true),
            _ => false,
        };
        if stops
            && let Some(session) = self.schedule.prompts.consume(key)
            && self.live_phase() == (Phase::Listening, Some(session))
        {
            self.stop_capture(cx);
        }
        true
    }

    fn stop_capture(&mut self, cx: &mut Context<Self>) {
        let Some(services) = self.services.clone() else {
            return;
        };
        let reply = host::stop(&services);
        cx.spawn(async move |this, cx| {
            if let Err(error) = host::acknowledged(reply).await {
                let _ = this.update(cx, |this, cx| this.status(&error.to_string(), cx));
            }
        })
        .detach();
    }

    /// Shipping's `useStartListening` tail: chat and screen capture run for the whole
    /// recording (each checks its setting every round), the disclosure is posted once.
    fn start_meeting_capture(&mut self, session: SessionId, cx: &mut Context<Self>) {
        let Some(apps) = self.meeting_apps(cx) else {
            return;
        };
        let tasks = vec![
            self.capture_chat(session.clone(), apps.clone(), cx),
            self.capture_screens(session.clone(), apps.clone(), cx),
        ];
        self.schedule.capture = Some((session.clone(), tasks));
        if self.schedule.flag("consent_auto_send_chat", false)
            && !self.schedule.disclosed.contains(&session)
        {
            self.schedule.disclosure = Some(self.post_disclosure(session, apps, cx));
        }
    }

    fn listening(&self, session: &SessionId) -> bool {
        self.live_phase() == (Phase::Listening, Some(session.clone()))
    }

    fn capture_chat(
        &mut self,
        session: SessionId,
        apps: Arc<dyn MeetingApps>,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        let runtime = self.runtime.clone();
        cx.spawn(async move |this, cx| {
            let mut state = ChatState::default();
            loop {
                let enabled = |this: &Self| this.schedule.flag("capture_meeting_chat", false);
                let Ok(on) = this.update(cx, |this, _| enabled(this)) else {
                    return;
                };
                if !on {
                    state.reset();
                } else {
                    let result = apps.capture_chat().await;
                    let Ok(on) = this.update(cx, |this, _| enabled(this)) else {
                        return;
                    };
                    match result {
                        _ if !on => state.reset(),
                        Err(error) => tracing::warn!(%error, "failed to capture meeting chat"),
                        Ok(result) => {
                            let sample = state.observe(&result);
                            if let Some(warning) = sample.warning {
                                let _ = this.update(cx, |_, cx| {
                                    warn("meeting-chat-capture-warning", warning.message(), cx)
                                });
                            }
                            if !sample.entries.is_empty() {
                                match persist_chat(&runtime, &session, &sample.entries).await {
                                    Ok(declined) => {
                                        for (_, signature) in sample.entries {
                                            state.remember(signature);
                                        }
                                        if declined {
                                            let _ = this.update(cx, |this, cx| {
                                                if this.listening(&session) {
                                                    warn(
                                                        "meeting-consent-declined",
                                                        capture::PARTICIPANT_DECLINED,
                                                        cx,
                                                    );
                                                    this.stop_capture(cx);
                                                }
                                            });
                                        }
                                    }
                                    Err(error) => {
                                        tracing::warn!(%error, "failed to persist meeting chat")
                                    }
                                }
                            }
                        }
                    }
                }
                cx.background_executor().timer(capture::CHAT_INTERVAL).await;
            }
        })
    }

    fn capture_screens(
        &mut self,
        session: SessionId,
        apps: Arc<dyn MeetingApps>,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        let runtime = self.runtime.clone();
        let attachments =
            crate::editor::AttachmentService::new(runtime.clone(), self.editor_vault.clone());
        cx.spawn(async move |this, cx| {
            let mut reset = true;
            let mut warned = false;
            loop {
                let enabled = |this: &Self| this.schedule.flag("capture_shared_screens", false);
                let Ok(on) = this.update(cx, |this, _| enabled(this)) else {
                    return;
                };
                if !on {
                    reset = true;
                } else if apps.screen_recording_denied() {
                    if !warned {
                        warned = true;
                        let _ = this.update(cx, |_, cx| {
                            warn(
                                "meeting-screen-capture-warning",
                                capture::SCREEN_PERMISSION,
                                cx,
                            )
                        });
                    }
                } else {
                    let result = apps.capture_screen(reset).await;
                    reset = false;
                    let Ok(on) = this.update(cx, |this, _| enabled(this)) else {
                        return;
                    };
                    match result {
                        _ if !on => reset = true,
                        Err(error) => tracing::warn!(%error, "failed to capture shared screen"),
                        Ok(share) => {
                            if let Some(jpeg) = share.jpeg {
                                let captured_at = now_iso();
                                let name = format!(
                                    "shared-screen-{}.jpg",
                                    captured_at.replace([':', '.'], "-")
                                );
                                let saved =
                                    attachments.import_bytes(session.clone(), name, jpeg).await;
                                let statement = saved.map(|saved| {
                                    records::screen_statement(
                                        &session.0,
                                        &saved.id,
                                        share.platform.as_ref(),
                                        share.width,
                                        share.height,
                                        &captured_at,
                                        now_ms(),
                                    )
                                });
                                let result = match statement {
                                    Ok(statement) => transact(&runtime, vec![statement])
                                        .await
                                        .map_err(|error| error.to_string()),
                                    Err(error) => Err(error),
                                };
                                if let Err(error) = result {
                                    reset = true;
                                    tracing::warn!(%error, "failed to persist shared screen");
                                }
                            }
                        }
                    }
                }
                cx.background_executor()
                    .timer(capture::SCREEN_INTERVAL)
                    .await;
            }
        })
    }

    /// Shipping's `sendMeetingRecordingDisclosure`: up to 30 attempts a second apart while
    /// the session keeps listening, each attempt recorded once.
    fn post_disclosure(
        &mut self,
        session: SessionId,
        apps: Arc<dyn MeetingApps>,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        let runtime = self.runtime.clone();
        cx.spawn(async move |this, cx| {
            let cancelled = |cx: &mut gpui::AsyncApp| {
                this.update(cx, |this, _| !this.listening(&session))
                    .unwrap_or(true)
            };
            let mut target = ("unknown", "unknown");
            let mut reason = "meeting chat disclosure was not sent".to_owned();
            for attempt in 0..capture::DISCLOSURE_ATTEMPTS {
                if attempt > 0 {
                    cx.background_executor()
                        .timer(capture::DISCLOSURE_RETRY)
                        .await;
                }
                if cancelled(cx) {
                    record_disclosure(&runtime, &session, target, Delivery::Cancelled, "").await;
                    return;
                }
                let sent = match apps.mic_apps().await {
                    Err(error) => Err(error),
                    Ok(mic) => {
                        let mut ids: Vec<String> = mic
                            .into_iter()
                            .map(|app| app.id.trim().to_owned())
                            .filter(|id| !id.is_empty())
                            .collect();
                        ids.dedup();
                        apps.send_chat(capture::DISCLOSURE_MESSAGE.into(), ids)
                            .await
                    }
                };
                match sent {
                    Ok(result) => {
                        target = capture::disclosure_target(&result.platform, &result.surface);
                        if result.sent {
                            record_disclosure(&runtime, &session, target, Delivery::Sent, "").await;
                            let _ = this.update(cx, |this, _| {
                                this.schedule.disclosed.push_back(session.clone());
                                while this.schedule.disclosed.len() > MAX_DISCLOSED {
                                    this.schedule.disclosed.pop_front();
                                }
                            });
                            return;
                        }
                        reason = Some(result.warnings.join("; "))
                            .filter(|warnings| !warnings.is_empty())
                            .unwrap_or_else(|| "meeting chat mutation was rejected".into());
                    }
                    Err(error) => {
                        target = ("unknown", "unknown");
                        reason = error;
                    }
                }
            }
            record_disclosure(&runtime, &session, target, Delivery::NotSent, &reason).await;
            tracing::warn!(%reason, "meeting disclosure was not sent");
            let _ = this.update(cx, |_, cx| {
                warn(
                    "meeting-disclosure-send-failed",
                    capture::DISCLOSURE_FAILED,
                    cx,
                )
            });
        })
    }
}

/// Shipping's capture warnings: a `toast.warning` that stays until dismissed.
fn warn(id: &'static str, message: &'static str, cx: &mut gpui::App) {
    super::toasts::show(super::toasts::Toast::warning(message).id(id).sticky(), cx);
}

async fn transact(
    runtime: &RuntimeHandle,
    statements: Vec<anlg_db_execute::TransactionStatement>,
) -> Result<()> {
    runtime
        .submit(move |services| async move {
            services
                .executor
                .execute_transaction(statements)
                .await
                .map(|_| ())
                .map_err(failure)
        })?
        .receive()
        .await
}

/// Stores new chat and the consent replies among it; true when someone declined.
async fn persist_chat(
    runtime: &RuntimeHandle,
    session: &SessionId,
    entries: &[(apps::MeetingCapturedChatMessage, String)],
) -> Result<bool> {
    let captured_at = now_iso();
    transact(
        runtime,
        records::chat_statements(&session.0, entries, &captured_at, now_ms()),
    )
    .await?;
    let mut declined = false;
    for (message, _) in entries {
        let Some(consent) = capture::consent(&message.text) else {
            continue;
        };
        declined |= consent == Consent::Declined;
        let statement =
            capture::consent_statement(&session.0, message.sender.as_deref(), consent, &now_iso());
        if let Err(error) = transact(runtime, vec![statement]).await {
            tracing::warn!(%error, "failed to persist participant consent");
        }
    }
    Ok(declined)
}

async fn record_disclosure(
    runtime: &RuntimeHandle,
    session: &SessionId,
    target: (&str, &str),
    delivery: Delivery,
    reason: &str,
) {
    let statement = capture::disclosure_statement(&session.0, &now_iso(), target, delivery, reason);
    if let Err(error) = transact(runtime, vec![statement]).await {
        tracing::warn!(%error, "failed to persist disclosure attempt");
    }
}

/// Shipping's `readDueScheduledMeeting`: the event as stored now, if it is still due.
async fn read_due(runtime: &RuntimeHandle, event: &Arc<str>) -> Result<Option<Meeting>> {
    let event = event.clone();
    let rows = runtime
        .read(CancellationToken::new(), move |services| async move {
            services
                .executor
                .execute(
                    format!("{} AND e.id = ? LIMIT 1", auto_start::MEETINGS_SQL),
                    vec![json!(event)],
                )
                .await
                .map_err(failure)
        })?
        .receive()
        .await?;
    Ok(rows
        .first()
        .map(Meeting::from_row)
        .filter(|meeting| meeting.due(now_ms())))
}

/// The note may record automatically: still there, and not locked unless revealed.
async fn session_eligible(runtime: &RuntimeHandle, session: &SessionId) -> Result<bool> {
    let session = session.clone();
    let rows = runtime
        .read(CancellationToken::new(), move |services| async move {
            services
                .executor
                .execute(
                    "SELECT 1 FROM sessions WHERE id = ? AND deleted_at IS NULL AND (locked = 0 OR id IN (SELECT value FROM json_each(?)))"
                        .into(),
                    vec![json!(session), crate::meeting::access::param()],
                )
                .await
                .map_err(failure)
        })?
        .receive()
        .await?;
    Ok(!rows.is_empty())
}
