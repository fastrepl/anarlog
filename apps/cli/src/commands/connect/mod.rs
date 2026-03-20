pub(crate) mod action;
pub(crate) mod app;
pub(crate) mod effect;
mod providers;
pub(crate) mod runtime;
pub(crate) mod ui;

use std::collections::HashSet;
use std::time::Duration;

use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use clap::ValueEnum;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ConnectionType {
    Stt,
    Llm,
    Cal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ConnectProvider {
    Deepgram,
    Soniox,
    Assemblyai,
    Openai,
    Gladia,
    Elevenlabs,
    Mistral,
    Fireworks,
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    Cactus,
    Anthropic,
    Openrouter,
    GoogleGenerativeAi,
    AzureOpenai,
    AzureAi,
    Ollama,
    Lmstudio,
    Custom,
    #[cfg(target_os = "macos")]
    AppleCalendar,
    GoogleCalendar,
    OutlookCalendar,
}
use crate::error::{CliError, CliResult};

use self::action::Action;
use self::app::{App, FormFieldId, Step};
use self::effect::{Effect, SaveData};
use self::runtime::{Runtime, RuntimeEvent};

const IDLE_FRAME: Duration = Duration::from_secs(1);

// --- Screen ---

struct ConnectScreen {
    app: App,
    runtime: Runtime,
    pool: SqlitePool,
}

impl ConnectScreen {
    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<Option<SaveData>> {
        for effect in effects {
            match effect {
                Effect::Save(data) => return ScreenControl::Exit(Some(data)),
                Effect::Exit => return ScreenControl::Exit(None),
                Effect::CheckCalendarPermission => {
                    self.runtime.check_permission();
                }
                Effect::RequestCalendarPermission => {
                    self.runtime.request_permission();
                }
                Effect::ResetCalendarPermission => {
                    self.runtime.reset_permission();
                }
                Effect::LoadCalendars => {
                    let event = match runtime::load_calendars_sync() {
                        Ok(items) => RuntimeEvent::CalendarsLoaded(items),
                        Err(err) => RuntimeEvent::Error(err),
                    };
                    let effects = self.app.dispatch(Action::Runtime(event));
                    if let ScreenControl::Exit(output) = self.apply_effects(effects) {
                        return ScreenControl::Exit(output);
                    }
                }
                Effect::SaveCalendars(data) => {
                    let provider = self.app.provider().unwrap();
                    let connection_id = format!("cal:{}", provider.id());
                    self.runtime.save_calendars(
                        self.pool.clone(),
                        data.provider,
                        connection_id,
                        data.items,
                    );
                }
            }
        }
        ScreenControl::Continue
    }
}

impl Screen for ConnectScreen {
    type ExternalEvent = RuntimeEvent;
    type Output = Option<SaveData>;

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
            TuiEvent::Paste(text) => {
                let effects = self.app.dispatch(Action::Paste(text));
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
        let effects = self.app.dispatch(Action::Runtime(event));
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        ui::draw(frame, &mut self.app);
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(Some("connect"))
    }

    fn next_frame_delay(&self) -> Duration {
        IDLE_FRAME
    }
}

// --- Public API ---

pub struct Args {
    pub connection_type: Option<ConnectionType>,
    pub provider: Option<ConnectProvider>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub pool: SqlitePool,
}

pub async fn run(args: Args) -> CliResult<bool> {
    let interactive = std::io::IsTerminal::is_terminal(&std::io::stdin());

    if let (Some(ct), Some(p)) = (args.connection_type, &args.provider)
        && !p.valid_for(ct)
    {
        return Err(CliError::invalid_argument(
            "--provider",
            p.id(),
            format!("not a valid {ct} provider"),
        ));
    }

    if let Some(ref url) = args.base_url {
        app::validate_base_url(url)
            .map_err(|reason| CliError::invalid_argument("--base-url", url, reason))?;
    }

    let configured: HashSet<String> = hypr_db_app::list_configured_provider_ids(&args.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();

    let current_stt = hypr_db_app::get_setting(&args.pool, "current_stt_provider")
        .await
        .unwrap_or(None);
    let current_llm = hypr_db_app::get_setting(&args.pool, "current_llm_provider")
        .await
        .unwrap_or(None);

    let (app, initial_effects) = App::new_with_configured(
        args.connection_type,
        args.provider,
        args.base_url,
        args.api_key,
        configured,
        current_stt,
        current_llm,
    );

    let save_data = if app.step() == Step::Done {
        initial_effects.into_iter().find_map(|e| match e {
            Effect::Save(data) => Some(data),
            _ => None,
        })
    } else if !interactive {
        return Err(match app.step() {
            Step::SelectProvider => CliError::required_argument_with_hint(
                "--provider",
                "pass --provider <name> (interactive prompts require a terminal)",
            ),
            Step::InputForm => {
                if app
                    .form_fields()
                    .iter()
                    .any(|f| f.id == FormFieldId::BaseUrl)
                {
                    CliError::required_argument_with_hint(
                        "--base-url",
                        format!(
                            "{} requires a base URL",
                            app.provider()
                                .map(|p: ConnectProvider| p.id())
                                .unwrap_or("provider")
                        ),
                    )
                } else {
                    CliError::required_argument_with_hint(
                        "--api-key",
                        "pass --api-key <key> (interactive prompts require a terminal)",
                    )
                }
            }
            Step::Calendar => CliError::required_argument_with_hint(
                "--provider",
                "calendar setup requires an interactive terminal",
            ),
            Step::Done => unreachable!(),
        });
    } else {
        let (runtime_tx, runtime_rx) = mpsc::unbounded_channel();
        let runtime = Runtime::new(runtime_tx);

        let mut app = app;

        // Resolve initial calendar permission synchronously so the screen
        // opens with the status already known (avoids async channel timing).
        for effect in &initial_effects {
            match effect {
                Effect::CheckCalendarPermission => {
                    let state = runtime::check_permission_sync();
                    let effects = app.dispatch(Action::Runtime(
                        RuntimeEvent::CalendarPermissionStatus(state),
                    ));
                    // If authorized, this may produce LoadCalendars
                    for e in &effects {
                        if matches!(e, Effect::LoadCalendars) {
                            let event = match runtime::load_calendars_sync() {
                                Ok(items) => RuntimeEvent::CalendarsLoaded(items),
                                Err(err) => RuntimeEvent::Error(err),
                            };
                            let _ = app.dispatch(Action::Runtime(event));
                        }
                    }
                }
                Effect::LoadCalendars => {
                    let event = match runtime::load_calendars_sync() {
                        Ok(items) => RuntimeEvent::CalendarsLoaded(items),
                        Err(err) => RuntimeEvent::Error(err),
                    };
                    let _ = app.dispatch(Action::Runtime(event));
                }
                _ => {}
            }
        }

        let screen = ConnectScreen {
            app,
            runtime,
            pool: args.pool.clone(),
        };
        run_screen(screen, Some(runtime_rx))
            .await
            .map_err(|e| CliError::operation_failed("connect tui", e.to_string()))?
    };

    match save_data {
        Some(data) => {
            save_config(&args.pool, data).await?;
            Ok(true)
        }
        None => Ok(false),
    }
}

pub(crate) async fn save_config(pool: &SqlitePool, data: SaveData) -> CliResult<()> {
    let provider_id = data.provider.id();

    for ct in &data.connection_types {
        let type_key = ct.to_string();

        let _ =
            hypr_db_app::set_setting(pool, &format!("current_{type_key}_provider"), provider_id)
                .await
                .map_err(|e| CliError::operation_failed("save setting", e.to_string()))?;

        let _ = hypr_db_app::upsert_connection(
            pool,
            &type_key,
            provider_id,
            data.base_url.as_deref().unwrap_or(""),
            data.api_key.as_deref().unwrap_or(""),
        )
        .await
        .map_err(|e| CliError::operation_failed("save connection", e.to_string()))?;
    }

    let type_keys: Vec<String> = data
        .connection_types
        .iter()
        .map(|t| t.to_string())
        .collect();
    eprintln!("Saved {} provider: {provider_id}", type_keys.join("+"),);
    Ok(())
}
