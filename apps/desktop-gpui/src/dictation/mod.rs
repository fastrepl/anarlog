//! System-wide dictation, ported from shipping's `apps/desktop/src/dictation`: while dictation
//! is enabled for a Pro account and no meeting is recording, the global shortcut records the
//! microphone, transcribes it with the selected provider (plus dictionary terms) and inserts
//! the text into the field that was focused when it started. The floating panel shows the
//! level, the live preview and Done; Escape cancels.

pub mod controller;
pub mod insertion;
pub mod overlay;
mod preview;
pub mod recorder;
pub mod shortcut;
pub(crate) mod transcribe;
mod window;

use std::{path::PathBuf, sync::Arc};

use anlg_audio::AudioProvider;
use desktop_runtime::{CancellationToken, Result, RuntimeHandle, ServiceError};
use futures::{StreamExt, channel::mpsc::UnboundedReceiver, future::BoxFuture};
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, Global, SharedString, Subscription, Task,
    WindowHandle,
};

use crate::{meeting::config::ProviderServices, meeting::live::LiveState, product::settings};
use controller::{Controller, Effect, MAX_RECORDING, Phase};
use insertion::Insertion;
use overlay::{BarEvent, DictationBar, Panel};
use recorder::{Recorder, Update};
use shortcut::{ShortcutEvent, Shortcuts};

/// What the Dictation settings page reads, and its requests (shipping's `useDictationStatus`).
#[derive(Default)]
pub struct DictationStatus {
    pub phase: Phase,
    /// The shortcut is registered and waiting for a press.
    pub ready: bool,
    pub error: Option<SharedString>,
    /// Kept in memory only until dictation is disabled or the app quits.
    pub last_transcript: SharedString,
    pub capturing_shortcut: bool,
}

pub enum StatusRequest {
    Retry,
    Cancel,
}

impl EventEmitter<StatusRequest> for DictationStatus {}

struct StatusGlobal(Entity<DictationStatus>);
impl Global for StatusGlobal {}

impl DictationStatus {
    pub fn global(cx: &App) -> Option<Entity<Self>> {
        cx.try_global::<StatusGlobal>()
            .map(|global| global.0.clone())
    }

    /// Makes a fresh status the one dictation and the settings page share.
    pub fn install(cx: &mut App) -> Entity<Self> {
        let status = cx.new(|_| Self::default());
        cx.set_global(StatusGlobal(status.clone()));
        status
    }

    /// The shortcut recorder pauses dictation while it listens for keys.
    pub fn set_capturing_shortcut(&mut self, capturing: bool, cx: &mut Context<Self>) {
        if self.capturing_shortcut != capturing {
            self.capturing_shortcut = capturing;
            cx.notify();
        }
    }

    /// Registers the shortcut again after a setup error.
    pub fn retry(&mut self, cx: &mut Context<Self>) {
        cx.emit(StatusRequest::Retry);
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        cx.emit(StatusRequest::Cancel);
    }
}

/// A dictation error the application shows as a toast.
pub struct DictationFailed(pub SharedString);

/// Shipping's `DictationLifecycle` condition for listening to the shortcut.
pub fn armed(pro: bool, enabled: bool, meeting: bool, capturing_shortcut: bool) -> bool {
    pro && enabled && !meeting && !capturing_shortcut
}

struct Services {
    runtime: tokio::runtime::Runtime,
    recorder: Arc<Recorder>,
    audio: Arc<dyn AudioProvider>,
    insertion: Arc<dyn Insertion>,
}

#[derive(Clone, PartialEq)]
struct Armed {
    shortcut: String,
    hands_free: bool,
    retry: u64,
}

struct Started {
    target: String,
    request: Arc<transcribe::Request>,
    preview: bool,
}

struct Run {
    owner: String,
    cancellation: CancellationToken,
    live_preview: bool,
    started: Option<Started>,
    _updates: Task<()>,
}

pub struct Dictation {
    services: Arc<Services>,
    shortcuts: Box<dyn Shortcuts>,
    controller: Controller,
    armed: Option<Armed>,
    retry: u64,
    preferences: Option<Arc<settings::Snapshot>>,
    status: Entity<DictationStatus>,
    run: Option<Run>,
    panel: Option<Panel>,
    window: Option<(WindowHandle<DictationBar>, Subscription)>,
    timer: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
    _tasks: Vec<Task<()>>,
}

impl EventEmitter<DictationFailed> for Dictation {}

fn failed(message: impl Into<String>) -> ServiceError {
    ServiceError::Failed(message.into().into())
}

fn message(error: &ServiceError) -> SharedString {
    match error {
        ServiceError::Failed(message) | ServiceError::Unsupported(message) => {
            message.to_string().into()
        }
        error => error.to_string().into(),
    }
}

impl Dictation {
    /// Installs dictation with the platform's microphone, insertion and the shortcut handle
    /// `native_events` registered; without one, the shortcut reports itself unavailable.
    pub fn install(runtime: RuntimeHandle, cx: &mut App) -> Result<Entity<Self>> {
        let (shortcuts, events): (Box<dyn Shortcuts>, _) =
            match cx.has_global::<shortcut::NativeHotkeys>() {
                true => {
                    let (port, events) =
                        shortcut::NativePort::new(cx.remove_global::<shortcut::NativeHotkeys>());
                    (Box::new(port), Some(events))
                }
                false => (Box::new(shortcut::UnavailableShortcuts), None),
            };
        let tokio = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("dictation")
            .enable_all()
            .build()
            .map_err(|error| failed(error.to_string()))?;
        let services = Arc::new(Services {
            recorder: Arc::new(Recorder::new(tokio.handle().clone())),
            runtime: tokio,
            audio: Arc::new(anlg_audio_actual::ActualAudio),
            insertion: Arc::new(insertion::NativeInsertion),
        });
        let status = DictationStatus::install(cx);
        Ok(cx.new(|cx| Self::new(runtime, services, shortcuts, events, status, cx)))
    }

    fn new(
        runtime: RuntimeHandle,
        services: Arc<Services>,
        shortcuts: Box<dyn Shortcuts>,
        events: Option<UnboundedReceiver<ShortcutEvent>>,
        status: Entity<DictationStatus>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut subscriptions = vec![
            cx.observe_global::<crate::product::account::Account>(|this, cx| this.sync(cx)),
            cx.observe(&status, |this, _, cx| this.sync(cx)),
            cx.subscribe(&status, |this, _, request, cx| match request {
                StatusRequest::Retry => {
                    this.retry += 1;
                    this.sync(cx);
                }
                StatusRequest::Cancel => this.cancel(cx),
            }),
        ];
        if let Some(live) = LiveState::global(cx) {
            subscriptions.push(cx.observe(&live, |this, _, cx| this.sync(cx)));
        }
        let mut tasks = vec![Self::watch_preferences(runtime, cx)];
        if let Some(mut events) = events {
            tasks.push(cx.spawn(async move |this, cx| {
                while let Some(event) = events.next().await {
                    if this
                        .update(cx, |this, cx| this.shortcut(event, cx))
                        .is_err()
                    {
                        break;
                    }
                }
            }));
        }
        Self {
            services,
            shortcuts,
            controller: Controller::default(),
            armed: None,
            retry: 0,
            preferences: None,
            status,
            run: None,
            panel: None,
            window: None,
            timer: None,
            _subscriptions: subscriptions,
            _tasks: tasks,
        }
    }

    fn watch_preferences(runtime: RuntimeHandle, cx: &mut Context<Self>) -> Task<()> {
        let reply = settings::watch(&runtime);
        cx.spawn(async move |this, cx| {
            let Ok(mut watch) = async { reply?.receive().await }.await else {
                return;
            };
            loop {
                if watch
                    .terminal_error()
                    .or_else(|| watch.errors.try_recv().ok())
                    .is_some()
                {
                    break;
                }
                let rows = watch.snapshots.borrow_and_update().rows.clone();
                let snapshot = settings::decode(&rows).ok().map(Arc::new);
                if this
                    .update(cx, |this, cx| {
                        this.preferences = snapshot;
                        this.sync(cx);
                    })
                    .is_err()
                    || watch.snapshots.changed().await.is_err()
                {
                    break;
                }
            }
            let _ = watch.unsubscribe().await;
        })
    }

    fn flag(&self, key: &str) -> bool {
        self.preferences
            .as_ref()
            .and_then(|preferences| preferences.get(key))
            .and_then(|setting| setting.value.as_bool())
            .unwrap_or(false)
    }

    fn text(&self, key: &str) -> String {
        self.preferences
            .as_ref()
            .and_then(|preferences| preferences.get(key))
            .and_then(|setting| setting.value.as_str())
            .unwrap_or_default()
            .to_owned()
    }

    fn update_status(&self, cx: &mut Context<Self>, f: impl FnOnce(&mut DictationStatus)) {
        self.status.update(cx, |status, cx| {
            f(status);
            cx.notify();
        });
    }

    fn fail(&mut self, error: &ServiceError, cx: &mut Context<Self>) {
        if matches!(error, ServiceError::Cancelled) {
            return;
        }
        let message = message(error);
        self.update_status(cx, |status| status.error = Some(message.clone()));
        cx.emit(DictationFailed(message));
    }

    /// Arms or disarms the shortcut when the account, settings or meeting state change.
    fn sync(&mut self, cx: &mut Context<Self>) {
        if self.preferences.is_none() {
            return;
        }
        let pro = crate::product::account::is_pro(cx);
        let enabled = self.flag("dictation_enabled");
        let meeting = LiveState::global(cx).is_some_and(|live| live.read(cx).current().is_some());
        let capturing = self.status.read(cx).capturing_shortcut;
        if !(pro && enabled) && !self.status.read(cx).last_transcript.is_empty() {
            self.update_status(cx, |status| {
                status.last_transcript = SharedString::default()
            });
        }
        let desired = armed(pro, enabled, meeting, capturing).then(|| Armed {
            shortcut: self.text("dictation_shortcut"),
            hands_free: self.flag("dictation_hands_free"),
            retry: self.retry,
        });
        if desired == self.armed {
            return;
        }
        self.disarm(cx);
        let Some(armed) = desired else {
            return;
        };
        self.controller.set_hands_free(armed.hands_free);
        match self.shortcuts.configure(Some(&armed.shortcut)) {
            Ok(()) => {
                self.update_status(cx, |status| {
                    status.ready = true;
                    status.error = None;
                });
            }
            Err(error) => self.fail(&error, cx),
        }
        self.armed = Some(armed);
    }

    fn disarm(&mut self, cx: &mut Context<Self>) {
        if self.armed.take().is_none() {
            return;
        }
        self.cancel(cx);
        self.panel = None;
        self.present(cx);
        self.update_status(cx, |status| {
            status.ready = false;
            status.phase = Phase::Idle;
        });
        if let Err(error) = self
            .shortcuts
            .set_active(false)
            .and_then(|()| self.shortcuts.configure(None))
        {
            tracing::warn!(%error, "dictation shortcut release failed");
        }
    }

    fn shortcut(&mut self, event: ShortcutEvent, cx: &mut Context<Self>) {
        if self.armed.is_none() || self.status.read(cx).capturing_shortcut {
            return;
        }
        let effect = match event {
            ShortcutEvent::Pressed => self.controller.press(),
            ShortcutEvent::Released => self.controller.release(false),
            ShortcutEvent::Cancelled => return self.cancel(cx),
        };
        self.after(effect, cx);
    }

    /// Escape, the settings page's Cancel, or disabling dictation mid-way.
    fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(run) = &self.run {
            run.cancellation.cancel();
        }
        let effect = self.controller.cancel();
        self.after(effect, cx);
    }

    /// The panel's Done button and the five-minute limit.
    fn finish_now(&mut self, cx: &mut Context<Self>) {
        let effect = self.controller.release(true);
        self.after(effect, cx);
    }

    fn after(&mut self, effect: Option<Effect>, cx: &mut Context<Self>) {
        match effect {
            Some(Effect::Start) => self.start(cx),
            Some(Effect::Finish) => self.finish(cx),
            Some(Effect::Cancel) => self.release_microphone(cx),
            None => {}
        }
        self.phase_changed(cx);
    }

    fn phase_changed(&mut self, cx: &mut Context<Self>) {
        let phase = self.controller.phase();
        if self.status.read(cx).phase != phase {
            self.update_status(cx, |status| status.phase = phase);
            if self.armed.is_some()
                && let Err(error) = self.shortcuts.set_active(phase != Phase::Idle)
            {
                self.fail(&error, cx);
            }
        }
        if phase == Phase::Idle {
            self.run = None;
            self.panel = None;
            self.timer = None;
        } else if let Some(panel) = &mut self.panel {
            panel.phase = phase;
        }
        self.present(cx);
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        let owner = format!("system-dictation:{}", uuid::Uuid::new_v4());
        let cancellation = CancellationToken::new();
        let live_preview = self.flag("dictation_live_preview");
        let microphone = self.text("microphone_device");
        let (sender, mut updates) = futures::channel::mpsc::unbounded();
        let updates_task = {
            let owner = owner.clone();
            cx.spawn(async move |this, cx| {
                while let Some(update) = updates.next().await {
                    if this
                        .update(cx, |this, cx| this.recording_update(&owner, update, cx))
                        .is_err()
                    {
                        break;
                    }
                }
            })
        };
        self.run = Some(Run {
            owner: owner.clone(),
            cancellation: cancellation.clone(),
            live_preview,
            started: None,
            _updates: updates_task,
        });
        self.panel = Some(Panel {
            owner: owner.clone(),
            phase: Phase::Starting,
            microphone: if microphone.is_empty() {
                "Default microphone".into()
            } else {
                microphone.clone()
            },
            preview_enabled: live_preview,
            preview_unavailable: live_preview
                && (self.text("current_stt_provider").is_empty()
                    || self.text("current_stt_model").is_empty()),
            expanded: live_preview,
            ..Panel::default()
        });
        self.update_status(cx, |status| status.error = None);
        let meeting = LiveState::global(cx).is_some_and(|live| live.read(cx).current().is_some());
        let providers = cx.try_global::<ProviderServices>().cloned();
        let services = self.services.clone();
        let work = {
            let owner = owner.clone();
            let services = services.clone();
            async move {
                microphone_authorized()?;
                if meeting {
                    return Err(failed(
                        "Dictation is unavailable while Anarlog is recording a meeting.",
                    ));
                }
                let insertion = services.insertion.clone();
                let target = tokio::task::spawn_blocking(move || insertion.capture())
                    .await
                    .map_err(|error| failed(error.to_string()))??;
                let providers = providers
                    .ok_or_else(|| failed("Speech-to-text is still starting. Try again."))?;
                let request = Arc::new(transcribe::Request {
                    preferences: crate::meeting::config::preferences(&providers.runtime).await?,
                });
                let preview = match live_preview {
                    true => transcribe::preview(&providers, &request).await,
                    false => None,
                };
                let has_preview = preview.is_some();
                if cancellation.is_cancelled() {
                    return Err(ServiceError::Cancelled);
                }
                let recorder = services.recorder.clone();
                let audio = services.audio.clone();
                let microphone = Some(microphone).filter(|name| !name.is_empty());
                let updates: recorder::Updates = Arc::new(move |update| {
                    let _ = sender.unbounded_send(update);
                });
                tokio::task::spawn_blocking(move || {
                    recorder.start(audio, microphone, owner, preview, Some(updates))
                })
                .await
                .map_err(|error| failed(error.to_string()))??;
                Ok(Started {
                    target,
                    request,
                    preview: has_preview,
                })
            }
        };
        let task = services.runtime.spawn(work);
        // Shipping waits for its panel before opening the microphone.
        self.present(cx);
        if self.window.is_none() {
            if let Some(run) = &self.run {
                run.cancellation.cancel();
            }
            self.fail(
                &failed("Could not open the dictation panel. Please try again."),
                cx,
            );
        }
        cx.spawn(async move |this, cx| {
            let result = task
                .await
                .unwrap_or_else(|error| Err(failed(error.to_string())));
            let _ = this.update(cx, |this, cx| this.started(owner, result, cx));
        })
        .detach();
    }

    fn started(&mut self, owner: String, result: Result<Started>, cx: &mut Context<Self>) {
        let current = self.run.as_ref().is_some_and(|run| run.owner == owner);
        if !current {
            if result.is_ok() {
                self.release(owner, cx);
            }
            return;
        }
        match result {
            Ok(started) => {
                if let Some(panel) = &mut self.panel {
                    panel.preview_unavailable =
                        panel.preview_unavailable || (panel.preview_enabled && !started.preview);
                }
                if let Some(run) = &mut self.run {
                    run.started = Some(started);
                }
                let effect = self.controller.started(true);
                if self.controller.phase() == Phase::Recording {
                    let recording = self.controller.recording();
                    self.timer = Some(cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(MAX_RECORDING).await;
                        let _ = this.update(cx, |this, cx| {
                            let effect = this.controller.timeout(recording);
                            this.after(effect, cx);
                        });
                    }));
                }
                self.after(effect, cx);
            }
            Err(error) => {
                if !self.controller.cancelled() {
                    self.fail(&error, cx);
                }
                let effect = self.controller.started(false);
                self.after(effect, cx);
            }
        }
    }

    fn recording_update(&mut self, owner: &str, update: Update, cx: &mut Context<Self>) {
        let Some(run) = self.run.as_ref().filter(|run| run.owner == owner) else {
            return;
        };
        if !matches!(self.controller.phase(), Phase::Starting | Phase::Recording) {
            return;
        }
        let live_preview = run.live_preview;
        let Some(panel) = &mut self.panel else {
            return;
        };
        match update {
            Update::Amplitude(amplitude) => panel.amplitude = amplitude,
            _ if !live_preview => return,
            Update::Transcript { text, partial } => {
                panel.text = text;
                panel.partial = partial;
            }
            Update::PreviewUnavailable => {
                panel.preview_unavailable = true;
                panel.partial.clear();
            }
        }
        self.present(cx);
    }

    /// Releases a recording that opened after its dictation was abandoned.
    fn release(&self, owner: String, cx: &mut Context<Self>) {
        let recorder = self.services.recorder.clone();
        let task = self
            .services
            .runtime
            .spawn(async move { recorder.cancel(&owner).await });
        cx.background_executor()
            .spawn(async move {
                if let Ok(Err(error)) = task.await {
                    tracing::warn!(%error, "abandoned dictation recording could not be released");
                }
            })
            .detach();
    }

    fn release_microphone(&mut self, cx: &mut Context<Self>) {
        let Some(owner) = self.run.as_ref().map(|run| run.owner.clone()) else {
            self.controller.released();
            return;
        };
        let recorder = self.services.recorder.clone();
        let task = self
            .services
            .runtime
            .spawn(async move { recorder.cancel(&owner).await });
        cx.spawn(async move |this, cx| {
            let result = task
                .await
                .unwrap_or_else(|error| Err(failed(error.to_string())));
            let _ = this.update(cx, |this, cx| {
                if let Err(error) = result {
                    this.fail(&error, cx);
                }
                this.controller.released();
                this.phase_changed(cx);
            });
        })
        .detach();
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        let Some(run) = &self.run else {
            self.controller.finished();
            return;
        };
        let Some(started) = &run.started else {
            self.controller.finished();
            return;
        };
        self.timer = None;
        let owner = run.owner.clone();
        let run_owner = owner.clone();
        let cancellation = run.cancellation.clone();
        let target = started.target.clone();
        let request = started.request.clone();
        let services = self.services.clone();
        let providers = cx.try_global::<ProviderServices>().cloned();
        let task = self.services.runtime.spawn(async move {
            let recorded = services.recorder.stop(&owner).await?;
            let path = recorded.path.clone();
            let result = async {
                if cancellation.is_cancelled() {
                    return Ok(None);
                }
                let text = match recorded.transcript {
                    Some(text) => text,
                    None => {
                        let providers =
                            providers.ok_or_else(|| failed("Speech-to-text is still starting."))?;
                        transcribe::transcribe(
                            &providers,
                            &request,
                            &owner,
                            &path,
                            cancellation.clone(),
                        )
                        .await?
                    }
                };
                if cancellation.is_cancelled() {
                    return Ok(None);
                }
                let text = text.trim().to_owned();
                if text.is_empty() {
                    return Err(failed("No speech detected. Please try again."));
                }
                Ok(Some(text))
            }
            .await;
            Ok::<_, ServiceError>((result, path))
        });
        cx.spawn(async move |this, cx| {
            let outcome = task
                .await
                .unwrap_or_else(|error| Err(failed(error.to_string())));
            let (result, path): (Result<Option<String>>, Option<PathBuf>) = match outcome {
                Ok((result, path)) => (result, Some(path)),
                Err(error) => (Err(error), None),
            };
            let inserted = match result {
                Ok(Some(text)) => {
                    let insert = this.update(cx, |this, cx| {
                        this.update_status(cx, |status| {
                            status.last_transcript = text.clone().into()
                        });
                        // Escape can land after the text is ready; only the live run inserts it.
                        let live = this.run.as_ref().is_some_and(|run| run.owner == run_owner)
                            && !this.controller.cancelled();
                        (this.armed.is_some() && live).then(|| {
                            let insertion = this.services.insertion.clone();
                            this.services
                                .runtime
                                .spawn_blocking(move || insertion.insert(&target, &text))
                        })
                    });
                    match insert {
                        Ok(Some(task)) => task
                            .await
                            .unwrap_or_else(|error| Err(failed(error.to_string()))),
                        _ => Ok(()),
                    }
                }
                Ok(None) => Ok(()),
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                if let Err(error) = inserted
                    && !this.controller.cancelled()
                {
                    this.fail(&error, cx);
                }
                if let Some(path) = path
                    && let Err(error) = this.services.recorder.discard(&path)
                {
                    this.fail(&error, cx);
                }
                this.controller.finished();
                this.phase_changed(cx);
            });
        })
        .detach();
    }

    /// Shows, updates or closes the panel window to match the dictation.
    fn present(&mut self, cx: &mut Context<Self>) {
        match (self.panel.clone(), &self.window) {
            (Some(panel), Some((handle, _))) => {
                let _ = handle.update(cx, |bar, _, cx| bar.set(panel, cx));
            }
            (Some(panel), None) => {
                let options = window::options(
                    crate::zoom::window_size(overlay::size(panel.expanded), cx),
                    cx,
                );
                let opened = cx.open_window(options, |window, cx| {
                    window::keep_focus_elsewhere(window);
                    cx.new(|_| DictationBar::new(panel))
                });
                let bar = opened
                    .ok()
                    .and_then(|handle| Some((handle, handle.entity(cx).ok()?)));
                match bar {
                    Some((handle, bar)) => {
                        let subscription = cx.subscribe(&bar, |this, _, event, cx| match event {
                            BarEvent::Finish => this.finish_now(cx),
                            BarEvent::ToggleExpanded => {
                                if let Some(panel) = &mut this.panel {
                                    panel.expanded = !panel.expanded;
                                    this.present(cx);
                                }
                            }
                        });
                        self.window = Some((handle, subscription));
                    }
                    None => tracing::warn!("dictation panel could not open"),
                }
            }
            (None, Some(_)) => {
                if let Some((handle, _)) = self.window.take() {
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                }
            }
            (None, None) => {}
        }
    }
}

impl Dictation {
    /// The chat composer's voice input over this recorder.
    pub fn voice(&self, providers: ProviderServices) -> crate::chat::voice::Voice {
        crate::chat::voice::Voice(Arc::new(ChatVoice {
            services: self.services.clone(),
            providers,
            releasing: Arc::new(tokio::sync::watch::channel(0).0),
        }))
    }
}

/// Shipping's chat `useDictation`: the dictation recorder under an owner of its own for each
/// recording, transcribed in one batch by the selected provider when it stops.
struct ChatVoice {
    services: Arc<Services>,
    providers: ProviderServices,
    /// Dropped chat recordings still letting go of the microphone; the next one waits for them.
    releasing: Arc<tokio::sync::watch::Sender<usize>>,
}

impl ChatVoice {
    fn record(
        &self,
        microphone: BoxFuture<'static, Result<Option<String>>>,
    ) -> (ChatRecording, BoxFuture<'static, Result<()>>) {
        let owner = format!("chat:{}", uuid::Uuid::new_v4());
        let cancellation = CancellationToken::new();
        let (sender, receiver) = futures::channel::oneshot::channel();
        let opening = {
            let (services, owner, cancellation) =
                (self.services.clone(), owner.clone(), cancellation.clone());
            let mut releasing = self.releasing.subscribe();
            self.services.runtime.spawn(async move {
                let result = async {
                    let released = async { releasing.wait_for(|count| *count == 0).await.is_ok() };
                    cancellation
                        .run_until_cancelled(released)
                        .await
                        .ok_or(ServiceError::Cancelled)?;
                    let microphone = cancellation
                        .run_until_cancelled(microphone)
                        .await
                        .ok_or(ServiceError::Cancelled)??;
                    let (recorder, audio) = (services.recorder.clone(), services.audio.clone());
                    tokio::task::spawn_blocking(move || {
                        recorder.start(audio, microphone, owner, None, None)
                    })
                    .await
                    .map_err(|error| failed(error.to_string()))?
                }
                .await;
                let _ = sender.send(result);
            })
        };
        let recording = ChatRecording {
            services: self.services.clone(),
            providers: self.providers.clone(),
            releasing: self.releasing.clone(),
            owner,
            cancellation,
            opening: Some(opening),
        };
        let started =
            Box::pin(async move { receiver.await.unwrap_or(Err(ServiceError::Cancelled)) });
        (recording, started)
    }
}

impl crate::chat::voice::VoiceInput for ChatVoice {
    fn start(
        &self,
    ) -> (
        Box<dyn crate::chat::voice::VoiceRecording>,
        BoxFuture<'static, Result<()>>,
    ) {
        let runtime = self.providers.runtime.clone();
        let (recording, started) = self.record(Box::pin(async move {
            microphone_authorized()?;
            let preferences = crate::meeting::config::preferences(&runtime).await?;
            Ok(Some(preferences.text("microphone_device")).filter(|name| !name.is_empty()))
        }));
        (Box::new(recording), started)
    }
}

/// One chat recording. Dropped before it stops, it waits for its start and then closes the
/// microphone and deletes the audio.
struct ChatRecording {
    services: Arc<Services>,
    providers: ProviderServices,
    releasing: Arc<tokio::sync::watch::Sender<usize>>,
    owner: String,
    cancellation: CancellationToken,
    /// The start, until the recording stops.
    opening: Option<tokio::task::JoinHandle<()>>,
}

impl crate::chat::voice::VoiceRecording for ChatRecording {
    fn stop(mut self: Box<Self>) -> BoxFuture<'static, Result<String>> {
        let opening = self.opening.take();
        let (services, providers, owner) = (
            self.services.clone(),
            self.providers.clone(),
            self.owner.clone(),
        );
        let task = self.services.runtime.spawn(async move {
            if let Some(opening) = opening {
                let _ = opening.await;
            }
            let recorded = services.recorder.stop(&owner).await?;
            let result = async {
                let request = transcribe::Request {
                    preferences: crate::meeting::config::preferences(&providers.runtime).await?,
                };
                transcribe::transcribe(
                    &providers,
                    &request,
                    &owner,
                    &recorded.path,
                    CancellationToken::new(),
                )
                .await
            }
            .await;
            let _ = services.recorder.discard(&recorded.path);
            result
        });
        Box::pin(async move { task.await.map_err(|error| failed(error.to_string()))? })
    }
}

impl Drop for ChatRecording {
    fn drop(&mut self) {
        let Some(opening) = self.opening.take() else {
            return;
        };
        self.cancellation.cancel();
        self.releasing.send_modify(|count| *count += 1);
        let (recorder, owner, releasing) = (
            self.services.recorder.clone(),
            std::mem::take(&mut self.owner),
            self.releasing.clone(),
        );
        self.services.runtime.spawn(async move {
            let _ = opening.await;
            if let Err(error) = recorder.cancel(&owner).await {
                tracing::warn!(%error, "chat voice recording could not be released");
            }
            releasing.send_modify(|count| *count -= 1);
        });
    }
}

#[cfg(target_os = "macos")]
fn microphone_authorized() -> Result<()> {
    use crate::platform::permissions::{NativePermissions, Permission, Status};
    match NativePermissions::new(Arc::new(anlg_audio_actual::ActualAudio))
        .check(Permission::Microphone)
    {
        Status::Granted | Status::NotRequired => Ok(()),
        Status::Denied(_) => Err(failed(
            "Enable microphone permission in Settings > Permissions before dictating.",
        )),
    }
}

#[cfg(not(target_os = "macos"))]
fn microphone_authorized() -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoInsertion;

    impl Insertion for NoInsertion {
        fn capture(&self) -> Result<String> {
            Ok(String::new())
        }

        fn insert(&self, _: &str, _: &str) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_dropped_chat_recording_frees_the_microphone_for_the_next_one() {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        futures::executor::block_on(ready.receive()).unwrap();
        let tokio = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let services = Arc::new(Services {
            recorder: Arc::new(Recorder::new(tokio.handle().clone())),
            runtime: tokio,
            audio: Arc::new(anlg_audio_mock::MockAudio::new(1)),
            insertion: Arc::new(NoInsertion),
        });
        let offline = || failed("offline in tests");
        let voice = ChatVoice {
            services,
            providers: ProviderServices {
                runtime: runtime.clone(),
                api_url: String::new(),
                cloud: Arc::new(move || Box::pin(async move { Err(offline()) })),
                local: Arc::new(move |_| Box::pin(async move { Err(offline()) })),
                secret: Arc::new(|_, _| Box::pin(async { Ok(None) })),
                secret_write: Arc::new(|_, _, _| Box::pin(async { Ok(()) })),
            },
            releasing: Arc::new(tokio::sync::watch::channel(0).0),
        };
        let default_microphone =
            || -> BoxFuture<'static, Result<Option<String>>> { Box::pin(async { Ok(None) }) };
        // Closed while its microphone opens, like a note window shut mid-start.
        let (recording, _) = voice.record(default_microphone());
        drop(recording);
        // Closed while recording; the next one waits for the microphone instead of failing.
        let (recording, started) = voice.record(default_microphone());
        futures::executor::block_on(started).unwrap();
        drop(recording);
        let (recording, started) = voice.record(default_microphone());
        futures::executor::block_on(started).unwrap();
        drop(recording);
        let mut releasing = voice.releasing.subscribe();
        futures::executor::block_on(releasing.wait_for(|count| *count == 0)).unwrap();
        drop(voice);
        futures::executor::block_on(runtime.shutdown()).unwrap();
    }

    #[test]
    fn dictation_listens_only_for_enabled_pro_accounts_outside_meetings() {
        assert!(armed(true, true, false, false));
        assert!(!armed(false, true, false, false));
        assert!(!armed(true, false, false, false));
        assert!(!armed(true, true, true, false));
        assert!(!armed(true, true, false, true));
    }
}
