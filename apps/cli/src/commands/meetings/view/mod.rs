mod action;
mod app;
mod effect;
mod runtime;
mod ui;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::error::{CliError, CliResult};

use self::action::Action;
use self::app::App;
use self::effect::Effect;
use self::runtime::{Runtime, RuntimeEvent};

const IDLE_FRAME: std::time::Duration = std::time::Duration::from_secs(1);

pub struct Args {
    pub meeting_id: String,
    pub pool: SqlitePool,
}

struct ViewScreen {
    app: App,
    runtime: Runtime,
}

impl ViewScreen {
    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<()> {
        for effect in effects {
            match effect {
                Effect::SaveMemo { meeting_id, memo } => {
                    self.runtime.save_memo(meeting_id, memo);
                }
                Effect::Exit => return ScreenControl::Exit(()),
            }
        }
        ScreenControl::Continue
    }
}

impl Screen for ViewScreen {
    type ExternalEvent = RuntimeEvent;
    type Output = ();

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
            RuntimeEvent::Loaded {
                meeting,
                segments,
                memo,
            } => Action::Loaded {
                meeting,
                segments,
                memo,
            },
            RuntimeEvent::LoadError(msg) => Action::LoadError(msg),
            RuntimeEvent::Saved => Action::Saved,
            RuntimeEvent::SaveError(msg) => Action::SaveError(msg),
        };
        let effects = self.app.dispatch(action);
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        ui::draw(frame, &mut self.app);
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(Some("view"))
    }

    fn next_frame_delay(&self) -> std::time::Duration {
        IDLE_FRAME
    }
}

pub async fn run(args: Args) -> CliResult<()> {
    let (external_tx, external_rx) = mpsc::unbounded_channel();

    let runtime = Runtime::new(args.pool, external_tx);
    runtime.load(args.meeting_id.clone());

    let screen = ViewScreen {
        app: App::new(args.meeting_id),
        runtime,
    };

    run_screen(screen, Some(external_rx))
        .await
        .map_err(|e| CliError::operation_failed("view tui", e.to_string()))
}
