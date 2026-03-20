use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use hypr_listener_core::actors::{RootActor, RootArgs, RootMsg, SessionParams};
use hypr_listener_core::{RecordingMode, StopSessionParams, TranscriptionMode};
use ractor::Actor;
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum AudioMode {
    Dual,
    #[cfg(feature = "dev")]
    Mock,
}
use crate::config::paths;
use crate::config::stt::{SttGlobalArgs, resolve_config};
use crate::error::{CliError, CliResult};
use crate::output::format_hhmmss;
use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen, run_screen_inline};

mod action;
mod app;
mod effect;
mod exit;
pub mod post_meeting;
mod runtime;
mod ui;

use self::action::Action;
use self::app::App;
use self::effect::Effect;
use self::exit::ExitScreen;
use self::post_meeting::spawn_post_meeting;
use self::runtime::Runtime;

pub struct Args {
    pub stt: SttGlobalArgs,
    pub record: bool,
    pub audio: AudioMode,
    pub pool: SqlitePool,
}

const ANIMATION_FRAME: std::time::Duration = std::time::Duration::from_millis(33);
const IDLE_FRAME: std::time::Duration = std::time::Duration::from_secs(1);

struct Output {
    elapsed: std::time::Duration,
    force_quit: bool,
    app: App,
}

enum ExternalEvent {
    Listener(runtime::RuntimeEvent),
}

struct LiveScreen {
    app: App,
    capture_post_exit_events: Arc<AtomicBool>,
}

impl LiveScreen {
    fn new(
        participant_names: HashMap<String, String>,
        capture_post_exit_events: Arc<AtomicBool>,
    ) -> Self {
        Self {
            app: App::new(participant_names),
            capture_post_exit_events,
        }
    }

    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<Output> {
        for effect in effects {
            match effect {
                Effect::Exit { force } => {
                    if !force {
                        self.capture_post_exit_events.store(true, Ordering::SeqCst);
                    }
                    let app = std::mem::replace(&mut self.app, App::new(HashMap::new()));
                    return ScreenControl::Exit(Output {
                        elapsed: app.elapsed(),
                        force_quit: force,
                        app,
                    });
                }
            }
        }

        ScreenControl::Continue
    }
}

impl Screen for LiveScreen {
    type ExternalEvent = ExternalEvent;
    type Output = Output;

    fn on_tui_event(
        &mut self,
        event: TuiEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {
            TuiEvent::Key(key) => {
                let effects = self.app.dispatch(Action::Key(key));
                self.apply_effects(effects)
            }
            TuiEvent::Paste(pasted) => {
                let effects = self.app.dispatch(Action::Paste(pasted));
                self.apply_effects(effects)
            }
            TuiEvent::Draw | TuiEvent::Resize => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        let action = match event {
            ExternalEvent::Listener(event) => Action::RuntimeEvent(event),
        };
        let effects = self.app.dispatch(action);
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        ui::draw(frame, &mut self.app);
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(Some(&format!(
            "{} ({})",
            self.app.status(),
            format_hhmmss(self.app.elapsed())
        )))
    }

    fn next_frame_delay(&self) -> std::time::Duration {
        if self.app.has_active_animations() {
            ANIMATION_FRAME
        } else {
            IDLE_FRAME
        }
    }
}

pub async fn run(args: Args) -> CliResult<()> {
    let Args {
        stt,
        record,
        audio: audio_mode,
        pool,
    } = args;

    let resolved = resolve_config(
        stt.provider,
        stt.base_url,
        stt.api_key,
        stt.model,
        stt.language,
    )
    .await?;
    let languages = vec![resolved.language.clone()];

    let meeting_id = uuid::Uuid::new_v4().to_string();
    let meeting_label = meeting_id.clone();

    let (event_id, participant_names) = resolve_event(&pool).await;

    let vault_base = paths::resolve_paths().base;

    let (listener_tx, mut listener_rx) = tokio::sync::mpsc::unbounded_channel();
    let runtime = Arc::new(Runtime::new(vault_base.clone(), listener_tx));

    let audio: Arc<dyn hypr_audio_actual::AudioProvider> = match audio_mode {
        AudioMode::Dual => Arc::new(hypr_audio_actual::ActualAudio),
        #[cfg(feature = "dev")]
        AudioMode::Mock => Arc::new(hypr_audio_mock::MockAudio::new(1)),
    };

    let (root_ref, _handle) = Actor::spawn(
        Some(RootActor::name()),
        RootActor,
        RootArgs {
            runtime: runtime.clone(),
            audio,
        },
    )
    .await
    .map_err(|e| CliError::operation_failed("spawn root actor", e.to_string()))?;

    let params = SessionParams {
        session_id: meeting_id,
        languages,
        onboarding: false,
        transcription_mode: TranscriptionMode::Live,
        recording_mode: if record {
            RecordingMode::Disk
        } else {
            RecordingMode::Memory
        },
        model: resolved.model.clone(),
        base_url: resolved.base_url.clone(),
        api_key: resolved.api_key.clone(),
        keywords: vec![],
    };

    ractor::call!(root_ref, RootMsg::StartSession, params)
        .map_err(|e| CliError::operation_failed("start session", e.to_string()))?
        .map_err(|e| CliError::operation_failed("start session", format!("{e:?}")))?;

    let (external_tx, external_rx) = mpsc::unbounded_channel();
    let (post_exit_tx, mut post_exit_rx) = mpsc::unbounded_channel();
    let capture_post_exit_events = Arc::new(AtomicBool::new(false));
    let capture_post_exit_events_task = capture_post_exit_events.clone();
    tokio::spawn(async move {
        while let Some(event) = listener_rx.recv().await {
            let capture_post_exit_events = capture_post_exit_events_task.load(Ordering::SeqCst);
            if capture_post_exit_events {
                let _ = post_exit_tx.send(event.clone());
            }
            if external_tx.send(ExternalEvent::Listener(event)).is_err()
                && !capture_post_exit_events
            {
                break;
            }
        }
    });

    let output = run_screen(
        LiveScreen::new(participant_names, capture_post_exit_events.clone()),
        Some(external_rx),
    )
    .await
    .map_err(|e| CliError::operation_failed("live tui", e.to_string()))?;

    let Output {
        elapsed,
        force_quit,
        mut app,
    } = output;

    if !force_quit {
        ractor::call!(root_ref, RootMsg::StopSession, StopSessionParams::default())
            .map_err(|e| CliError::operation_failed("stop session", e.to_string()))?;
        tokio::task::yield_now().await;
        app.apply_runtime_events(std::iter::from_fn(|| post_exit_rx.try_recv().ok()));

        let llm_config = crate::llm::resolve_config(&pool, None, None, None, None)
            .await
            .map_err(|e| {
                e.to_string()
                    .lines()
                    .next()
                    .unwrap_or("LLM not configured")
                    .to_string()
            });

        let (exit_tx, exit_rx) = mpsc::unbounded_channel();
        spawn_post_meeting(
            llm_config,
            exit_tx,
            app.words(),
            post_meeting::to_persistable_hints(&app.hints()),
            app.memo_text(),
            meeting_label.clone(),
            event_id,
            pool,
        );

        let exit_screen = ExitScreen::new(
            meeting_label,
            elapsed,
            vec!["Saving to database", "Generating summary"],
        );
        let height = exit_screen.viewport_height();
        run_screen_inline(exit_screen, height, Some(exit_rx))
            .await
            .map_err(|e| CliError::operation_failed("exit summary", e.to_string()))?;
    }

    Ok(())
}

async fn resolve_event(pool: &SqlitePool) -> (Option<String>, HashMap<String, String>) {
    let event = match hypr_db_app::find_current_or_upcoming_event(pool, 15).await {
        Ok(Some(e)) => e,
        _ => return (None, HashMap::new()),
    };

    let event_id = event.id.clone();
    let participants = hypr_db_app::list_event_participants(pool, &event_id)
        .await
        .unwrap_or_default();

    let mut names = HashMap::new();
    for p in &participants {
        if let Some(human_id) = &p.human_id {
            let name = if p.name.is_empty() {
                match hypr_db_app::get_human(pool, human_id).await {
                    Ok(Some(h)) => h.name,
                    _ => continue,
                }
            } else {
                p.name.clone()
            };
            names.insert(human_id.clone(), name);
        }
    }

    (Some(event_id), names)
}
