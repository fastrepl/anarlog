use std::convert::Infallible;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};

mod action;
mod app;
mod effect;
mod ui;

use self::action::Action;
use self::app::App;
use self::effect::Effect;

pub enum UpdateOutcome {
    RunUpdate,
    Continue,
}

struct UpdateScreen {
    app: App,
}

impl UpdateScreen {
    fn apply_effects(&self, effects: Vec<Effect>) -> ScreenControl<UpdateOutcome> {
        for effect in effects {
            match effect {
                Effect::AcceptUpdate => {
                    return ScreenControl::Exit(UpdateOutcome::RunUpdate);
                }
                Effect::Skip => return ScreenControl::Exit(UpdateOutcome::Continue),
                Effect::SkipVersion => {
                    crate::update_check::save_skipped_version(&self.app.latest);
                    return ScreenControl::Exit(UpdateOutcome::Continue);
                }
            }
        }
        ScreenControl::Continue
    }
}

impl Screen for UpdateScreen {
    type ExternalEvent = Infallible;
    type Output = UpdateOutcome;

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
            _ => ScreenControl::Continue,
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
        ui::draw(frame, &self.app);
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(Some("Update"))
    }
}

pub async fn run(
    current: String,
    latest: String,
    action: &crate::update_check::UpdateAction,
) -> UpdateOutcome {
    let screen = UpdateScreen {
        app: App::new(current, latest, action.command_str()),
    };

    run_screen::<UpdateScreen>(screen, None)
        .await
        .unwrap_or(UpdateOutcome::Continue)
}
