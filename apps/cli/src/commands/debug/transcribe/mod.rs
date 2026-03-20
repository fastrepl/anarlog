mod action;
mod app;
mod audio;
mod effect;
mod runtime;
mod server;
#[cfg(feature = "dev")]
mod tracing;
mod ui;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};
use tokio::sync::mpsc;

pub use super::{DebugProvider, TranscribeArgs as DebugTranscribeArgs, TranscribeMode};
use crate::error::{CliError, CliResult};

use self::action::Action;
use self::app::App;
use self::effect::Effect;
use self::runtime::Runtime;

struct TranscribeScreen {
    app: App,
}

impl TranscribeScreen {
    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<()> {
        for effect in effects {
            match effect {
                Effect::Exit => return ScreenControl::Exit(()),
            }
        }

        ScreenControl::Continue
    }
}

impl Screen for TranscribeScreen {
    type ExternalEvent = runtime::RuntimeEvent;
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
            TuiEvent::Paste(_) | TuiEvent::Draw | TuiEvent::Resize => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        let effects = self.app.dispatch(Action::Runtime(event));
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        ui::draw(frame, &mut self.app);
    }

    fn title(&self) -> String {
        self.app.title()
    }

    fn next_frame_delay(&self) -> std::time::Duration {
        if self.app.is_raw_mode() {
            std::time::Duration::from_millis(50)
        } else if self.app.has_recent_words() {
            std::time::Duration::from_millis(16)
        } else {
            std::time::Duration::from_millis(100)
        }
    }
}

pub async fn run(args: DebugTranscribeArgs) -> CliResult<()> {
    let mode = args.mode.clone();
    let (tx, rx) = mpsc::unbounded_channel();
    let runtime = Runtime::start(args, tx).await?;
    let screen = TranscribeScreen {
        app: App::new(mode, runtime.tracing_capture()),
    };

    let result = run_screen(screen, Some(rx))
        .await
        .map_err(|e| CliError::operation_failed("run transcribe screen", e.to_string()));
    runtime.abort();
    result
}
