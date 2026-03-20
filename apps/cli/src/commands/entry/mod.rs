use hypr_cli_tui::{Screen, ScreenContext, ScreenControl, TuiEvent, run_screen};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

mod action;
mod app;
mod effect;
mod ui;

pub enum EntryCommand {
    MeetingsNew,
    Chat { session_id: Option<String> },
    View { session_id: String },
}

pub enum EntryAction {
    Launch(EntryCommand),
    Model(crate::commands::model::Commands),
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
    pub pool: SqlitePool,
}

enum ExternalEvent {
    ConnectRuntime(crate::commands::connect::runtime::RuntimeEvent),
    MeetingsLoaded(Vec<hypr_db_app::MeetingRow>),
    MeetingsLoadError(String),
    EventsLoaded(Vec<hypr_db_app::EventRow>),
    CalendarNotConfigured,
    ModelsLoaded(Vec<crate::commands::model::list::ModelRow>),
    ModelsLoadError(String),
    ConnectSaved {
        connection_types: Vec<crate::commands::connect::ConnectionType>,
        provider_id: String,
    },
    ConnectSaveError(String),
    TimelineContactsLoaded {
        orgs: Vec<hypr_db_app::OrganizationRow>,
        humans: Vec<hypr_db_app::HumanRow>,
    },
    TimelineContactsLoadError(String),
    TimelineEntriesLoaded(Vec<hypr_db_app::TimelineRow>),
    TimelineEntriesLoadError(String),
}

struct EntryScreen {
    app: App,
    external_tx: mpsc::UnboundedSender<ExternalEvent>,
    connect_runtime: crate::commands::connect::runtime::Runtime,
    pool: SqlitePool,
}

impl EntryScreen {
    fn apply_effects(&mut self, effects: Vec<Effect>) -> ScreenControl<EntryAction> {
        for effect in effects {
            match effect {
                Effect::Launch(cmd) => return ScreenControl::Exit(EntryAction::Launch(cmd)),
                Effect::LoadMeetings => {
                    let tx = self.external_tx.clone();
                    let pool = self.pool.clone();
                    tokio::spawn(async move {
                        match hypr_db_app::list_meetings(&pool).await {
                            Ok(meetings) => {
                                let _ = tx.send(ExternalEvent::MeetingsLoaded(meetings));
                            }
                            Err(e) => {
                                let _ = tx.send(ExternalEvent::MeetingsLoadError(e.to_string()));
                            }
                        }
                    });
                    let tx = self.external_tx.clone();
                    let pool = self.pool.clone();
                    tokio::spawn(async move {
                        match hypr_db_app::has_calendars(&pool).await {
                            Ok(true) => {
                                let today = chrono::Local::now().date_naive();
                                let start = today.format("%Y-%m-%d").to_string();
                                let end = (today + chrono::Duration::days(2))
                                    .format("%Y-%m-%d")
                                    .to_string();
                                match hypr_db_app::list_events_in_range(&pool, &start, &end).await {
                                    Ok(events) => {
                                        let _ = tx.send(ExternalEvent::EventsLoaded(events));
                                    }
                                    Err(e) => {
                                        let _ = tx
                                            .send(ExternalEvent::MeetingsLoadError(e.to_string()));
                                    }
                                }
                            }
                            Ok(false) => {
                                let _ = tx.send(ExternalEvent::CalendarNotConfigured);
                            }
                            Err(e) => {
                                let _ = tx.send(ExternalEvent::MeetingsLoadError(e.to_string()));
                            }
                        }
                    });
                }
                Effect::LoadModels => {
                    let tx = self.external_tx.clone();
                    let pool = self.pool.clone();
                    tokio::spawn(async move {
                        let paths = crate::config::paths::resolve_paths();
                        let models_base = paths.models_base.clone();
                        let runtime =
                            std::sync::Arc::new(crate::commands::model::runtime::CliModelRuntime {
                                models_base: models_base.clone(),
                                progress_tx: None,
                            });
                        let manager = hypr_model_downloader::ModelDownloadManager::new(runtime);
                        let current = crate::config::paths::load_settings_from_db(&pool).await;
                        let models: Vec<hypr_local_model::LocalModel> =
                            hypr_local_model::LocalModel::all()
                                .into_iter()
                                .filter(|m| crate::commands::model::model_is_enabled(m))
                                .collect();
                        let rows = crate::commands::model::list::collect_model_rows(
                            &models,
                            &models_base,
                            &current,
                            &manager,
                        )
                        .await;
                        let _ = tx.send(ExternalEvent::ModelsLoaded(rows));
                    });
                }
                Effect::LoadTimelineContacts => {
                    let tx = self.external_tx.clone();
                    let pool = self.pool.clone();
                    tokio::spawn(async move {
                        let orgs = hypr_db_app::list_organizations(&pool).await;
                        let humans = hypr_db_app::list_humans(&pool).await;
                        match (orgs, humans) {
                            (Ok(orgs), Ok(humans)) => {
                                let _ =
                                    tx.send(ExternalEvent::TimelineContactsLoaded { orgs, humans });
                            }
                            (Err(e), _) | (_, Err(e)) => {
                                let _ = tx
                                    .send(ExternalEvent::TimelineContactsLoadError(e.to_string()));
                            }
                        }
                    });
                }
                Effect::LoadTimelineEntries(human_id) => {
                    let tx = self.external_tx.clone();
                    let pool = self.pool.clone();
                    tokio::spawn(async move {
                        match hypr_db_app::list_timeline_by_human(&pool, &human_id).await {
                            Ok(entries) => {
                                let _ = tx.send(ExternalEvent::TimelineEntriesLoaded(entries));
                            }
                            Err(e) => {
                                let _ =
                                    tx.send(ExternalEvent::TimelineEntriesLoadError(e.to_string()));
                            }
                        }
                    });
                }
                Effect::SaveConnect {
                    connection_types,
                    provider,
                    base_url,
                    api_key,
                } => {
                    let provider_id = provider.id().to_string();
                    let pool = self.pool.clone();
                    let tx = self.external_tx.clone();
                    let ct = connection_types.clone();
                    tokio::spawn(async move {
                        match crate::commands::connect::save_config(
                            &pool,
                            crate::commands::connect::effect::SaveData {
                                connection_types: ct,
                                provider,
                                base_url,
                                api_key,
                            },
                        )
                        .await
                        {
                            Ok(()) => {
                                let _ = tx.send(ExternalEvent::ConnectSaved {
                                    connection_types,
                                    provider_id,
                                });
                            }
                            Err(error) => {
                                let _ = tx.send(ExternalEvent::ConnectSaveError(error.to_string()));
                            }
                        }
                    });
                }
                Effect::CheckCalendarPermission => {
                    let effects = self.app.dispatch(Action::ConnectRuntime(
                        crate::commands::connect::runtime::RuntimeEvent::CalendarPermissionStatus(
                            crate::commands::connect::runtime::check_permission_sync(),
                        ),
                    ));
                    if let ScreenControl::Exit(output) = self.apply_effects(effects) {
                        return ScreenControl::Exit(output);
                    }
                }
                Effect::RequestCalendarPermission => {
                    self.connect_runtime.request_permission();
                }
                Effect::ResetCalendarPermission => {
                    self.connect_runtime.reset_permission();
                }
                Effect::LoadCalendars => {
                    let effects = self.app.dispatch(Action::ConnectRuntime(
                        match crate::commands::connect::runtime::load_calendars_sync() {
                            Ok(items) => {
                                crate::commands::connect::runtime::RuntimeEvent::CalendarsLoaded(
                                    items,
                                )
                            }
                            Err(err) => crate::commands::connect::runtime::RuntimeEvent::Error(err),
                        },
                    ));
                    if let ScreenControl::Exit(output) = self.apply_effects(effects) {
                        return ScreenControl::Exit(output);
                    }
                }
                Effect::SaveCalendars(data) => {
                    let connection_id = format!("cal:{}", data.provider);
                    self.connect_runtime.save_calendars(
                        self.pool.clone(),
                        data.provider,
                        connection_id,
                        data.items,
                    );
                }
                Effect::OpenAuth => {
                    let message = match crate::commands::auth::run() {
                        Ok(()) => "Opened auth page in browser".to_string(),
                        Err(error) => error.to_string(),
                    };
                    let inner = self.app.dispatch(Action::StatusMessage(message));
                    debug_assert!(inner.is_empty());
                }
                Effect::OpenBug => {
                    let message = match crate::commands::bug::run() {
                        Ok(()) => "Opened bug report page in browser".to_string(),
                        Err(error) => error.to_string(),
                    };
                    let inner = self.app.dispatch(Action::StatusMessage(message));
                    debug_assert!(inner.is_empty());
                }
                Effect::OpenHello => {
                    let message = match crate::commands::hello::run() {
                        Ok(()) => "Opened char.com in browser".to_string(),
                        Err(error) => error.to_string(),
                    };
                    let inner = self.app.dispatch(Action::StatusMessage(message));
                    debug_assert!(inner.is_empty());
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
                    let inner = self.app.dispatch(Action::StatusMessage(message));
                    debug_assert!(inner.is_empty());
                }
                Effect::RunModel(cmd) => return ScreenControl::Exit(EntryAction::Model(cmd)),
                Effect::Exit => return ScreenControl::Exit(EntryAction::Quit),
            }
        }

        ScreenControl::Continue
    }
}

impl Screen for EntryScreen {
    type ExternalEvent = ExternalEvent;
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
            TuiEvent::Draw | TuiEvent::Resize => ScreenControl::Continue,
        }
    }

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        let action = match event {
            ExternalEvent::ConnectRuntime(event) => Action::ConnectRuntime(event),
            ExternalEvent::MeetingsLoaded(meetings) => Action::MeetingsLoaded(meetings),
            ExternalEvent::MeetingsLoadError(msg) => Action::MeetingsLoadError(msg),
            ExternalEvent::EventsLoaded(events) => Action::EventsLoaded(events),
            ExternalEvent::CalendarNotConfigured => Action::CalendarNotConfigured,
            ExternalEvent::ModelsLoaded(models) => Action::ModelsLoaded(models),
            ExternalEvent::ModelsLoadError(msg) => Action::ModelsLoadError(msg),
            ExternalEvent::ConnectSaved {
                connection_types,
                provider_id,
            } => Action::ConnectSaved {
                connection_types,
                provider_id,
            },
            ExternalEvent::ConnectSaveError(msg) => Action::StatusMessage(msg),
            ExternalEvent::TimelineContactsLoaded { orgs, humans } => {
                Action::TimelineContactsLoaded { orgs, humans }
            }
            ExternalEvent::TimelineContactsLoadError(msg) => Action::TimelineContactsLoadError(msg),
            ExternalEvent::TimelineEntriesLoaded(entries) => Action::TimelineEntriesLoaded(entries),
            ExternalEvent::TimelineEntriesLoadError(msg) => Action::TimelineEntriesLoadError(msg),
        };
        let effects = self.app.dispatch(action);
        self.apply_effects(effects)
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        ui::draw(frame, &mut self.app);
    }

    fn on_resize(&mut self) {
        self.app.reload_logo();
    }

    fn title(&self) -> String {
        hypr_cli_tui::terminal_title(None)
    }
}

pub async fn run(args: Args) -> EntryAction {
    let (external_tx, external_rx) = mpsc::unbounded_channel();
    let (connect_tx, mut connect_rx) = mpsc::unbounded_channel();

    {
        let external_tx = external_tx.clone();
        tokio::spawn(async move {
            while let Some(event) = connect_rx.recv().await {
                if external_tx
                    .send(ExternalEvent::ConnectRuntime(event))
                    .is_err()
                {
                    break;
                }
            }
        });
    }

    let pool = args.pool.clone();
    let mut screen = EntryScreen {
        app: App::new(args.status_message, args.stt_provider, args.llm_provider),
        external_tx,
        connect_runtime: crate::commands::connect::runtime::Runtime::new(connect_tx),
        pool,
    };

    if let Some(command) = args.initial_command {
        let effects = screen.app.dispatch(Action::SubmitCommand(command));
        if let ScreenControl::Exit(action) = screen.apply_effects(effects) {
            return action;
        }
    }

    run_screen::<EntryScreen>(screen, Some(external_rx))
        .await
        .unwrap_or(EntryAction::Quit)
}
