use std::convert::Infallible;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};

mod action;
mod app;
mod effect;
mod ui;

#[derive(Clone, Copy)]
pub enum EntryAction {
    Listen,
    Connect,
    Quit,
}

use self::action::Action;
use self::app::App;
use self::effect::Effect;

pub struct Args {
    pub status_message: Option<String>,
    pub initial_command: Option<String>,
    pub stt_provider: Option<String>,
    pub llm_provider: Option<String>,
}

struct EntryScreen {
    app: App,
}

impl EntryScreen {
    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<EntryAction> {
        for effect in effects {
            match effect {
                Effect::LaunchListen => return ScreenControl::Exit(EntryAction::Listen),
                Effect::LaunchConnect => return ScreenControl::Exit(EntryAction::Connect),
                Effect::OpenAuth => {
                    let message = match crate::commands::auth::run() {
                        Ok(()) => "Opened auth page in browser".to_string(),
                        Err(error) => error.to_string(),
                    };
                    let _ = self.app.dispatch(Action::StatusMessage(message));
                }
                Effect::OpenDesktop => {
                    let message = match crate::commands::desktop::run() {
                        Ok(crate::commands::desktop::DesktopAction::OpenedApp) => {
                            "Opened desktop app".to_string()
                        }
                        Ok(crate::commands::desktop::DesktopAction::OpenedDownloadPage) => {
                            "Desktop app not found. Opened download page".to_string()
                        }
                        Err(error) => error.to_string(),
                    };
                    let _ = self.app.dispatch(Action::StatusMessage(message));
                }
                Effect::Exit => return ScreenControl::Exit(EntryAction::Quit),
            }
        }

        ScreenControl::Continue
    }
}

impl Screen for EntryScreen {
    type ExternalEvent = Infallible;
    type Output = EntryAction;

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
            TuiEvent::Draw => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        match event {}
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        ui::draw(frame, &mut self.app);
    }

    fn title(&self) -> String {
        "char".into()
    }
}

pub async fn run(args: Args) -> EntryAction {
    let mut screen = EntryScreen {
        app: App::new(args.status_message, args.stt_provider, args.llm_provider),
    };

    if let Some(command) = args.initial_command {
        let effects = screen.app.dispatch(Action::SubmitCommand(command));
        if let ScreenControl::Exit(action) = screen.apply_effects(effects) {
            return action;
        }
    }

    run_screen::<EntryScreen>(screen, None)
        .await
        .unwrap_or(EntryAction::Quit)
}
