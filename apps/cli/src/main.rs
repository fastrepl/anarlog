mod agent;
mod cli;
mod commands;
mod config;
mod error;
mod llm;
mod output;
mod services;
mod theme;
mod update_check;
mod widgets;

use clap::Parser;
use sqlx::SqlitePool;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;

use crate::cli::{Cli, Commands};
use crate::config::stt::SttGlobalArgs;
use crate::error::CliResult;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let tui_command = matches!(
        &cli.command,
        Some(Commands::Chat { prompt: None, .. })
            | Some(Commands::Meetings { .. })
            | Some(Commands::Connect {
                r#type: None,
                provider: None
            })
            | Some(Commands::Configure { .. })
    ) || cli.command.is_none();
    let skip_tracing_init = {
        #[cfg(feature = "dev")]
        {
            matches!(
                &cli.command,
                Some(Commands::Debug {
                    command: commands::debug::Commands::Transcribe { .. }
                })
            )
        }
        #[cfg(not(feature = "dev"))]
        {
            false
        }
    };

    if let Some(base) = &cli.global.base {
        config::paths::set_base(base.clone());
    }

    if cli.global.no_color || std::env::var_os("NO_COLOR").is_some() {
        colored::control::set_override(false);
    }

    if !skip_tracing_init {
        let default_directive = if tui_command {
            LevelFilter::OFF.into()
        } else {
            cli.verbose.tracing_level_filter().into()
        };

        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::builder()
                    .with_default_directive(default_directive)
                    .from_env_lossy(),
            )
            .with_writer(std::io::stderr)
            .init();
    }

    if let Err(error) = run(cli).await {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn analytics_client() -> hypr_analytics::AnalyticsClient {
    let mut builder = hypr_analytics::AnalyticsClientBuilder::default();
    if let Some(key) = option_env!("POSTHOG_API_KEY") {
        builder = builder.with_posthog(key);
    }
    builder.build()
}

fn track_command(client: &hypr_analytics::AnalyticsClient, subcommand: &'static str) {
    let client = client.clone();
    tokio::spawn(async move {
        let machine_id = hypr_host::fingerprint();
        let payload = hypr_analytics::AnalyticsPayload::builder("cli_command_invoked")
            .with("subcommand", subcommand)
            .with("app_identifier", "com.char.cli")
            .with("app_version", option_env!("APP_VERSION").unwrap_or("dev"))
            .build();
        let _ = client.event(machine_id, payload).await;
    });
}

async fn init_pool() -> CliResult<SqlitePool> {
    let paths = config::paths::resolve_paths();

    let db = if cfg!(debug_assertions) {
        hypr_db_core2::Db3::connect_memory_plain()
            .await
            .map_err(|e| error::CliError::operation_failed("db connect", e.to_string()))?
    } else {
        let db_path = paths.base.join("app.db");
        hypr_db_core2::Db3::connect_local_plain(&db_path)
            .await
            .map_err(|e| error::CliError::operation_failed("db connect", e.to_string()))?
    };

    hypr_db_app::migrate(db.pool())
        .await
        .map_err(|e| error::CliError::operation_failed("db migrate", e.to_string()))?;
    config::paths::migrate_json_settings_to_db(db.pool(), &paths.base).await;
    Ok(db.pool().clone())
}

async fn run(cli: Cli) -> CliResult<()> {
    let analytics = analytics_client();

    if let Some(ref command) = cli.command {
        let subcommand: &'static str = command.into();
        track_command(&analytics, subcommand);
    }

    let Cli {
        command,
        global,
        verbose,
    } = cli;

    let pool = init_pool().await?;

    let _calendar_sync_handle = {
        let apple_authorized = commands::connect::runtime::check_permission_sync()
            == commands::connect::runtime::CalendarPermissionState::Authorized;
        let api_base_url =
            std::env::var("CHAR_API_URL").unwrap_or_else(|_| "https://app.char.com".to_string());
        let access_token = std::env::var("CHAR_ACCESS_TOKEN").ok();
        let user_id = hypr_host::fingerprint();
        services::calendar_sync::spawn_calendar_sync(
            pool.clone(),
            services::calendar_sync::CalendarSyncConfig {
                api_base_url,
                access_token,
                apple_authorized,
                user_id,
            },
        )
    };

    let is_tui = matches!(
        &command,
        Some(Commands::Chat { prompt: None, .. })
            | Some(Commands::Meetings { .. })
            | Some(Commands::Connect {
                r#type: None,
                provider: None
            })
            | Some(Commands::Configure { .. })
    ) || command.is_none();

    if is_tui {
        if let update_check::UpdateStatus::UpdateAvailable {
            current,
            latest,
            channel,
        } = update_check::check_for_update().await
        {
            if let Some(action) = update_check::UpdateAction::detect(channel) {
                if let commands::update::UpdateOutcome::RunUpdate =
                    commands::update::run(current, latest, &action).await
                {
                    return run_update(&action);
                }
            }
        }
    }

    match command {
        Some(Commands::Chat {
            command,
            prompt,
            provider,
        }) => {
            let (meeting, resume_meeting_id) = match command {
                Some(commands::chat::Commands::Resume { meeting }) => (None, meeting),
                None => (None, None),
            };
            commands::chat::run(commands::chat::Args {
                meeting,
                prompt,
                provider,
                base_url: global.base_url,
                api_key: global.api_key,
                model: global.model,
                pool,
                resume_meeting_id,
            })
            .await
        }
        Some(Commands::Connect { r#type, provider }) => {
            if r#type.is_some() || provider.is_some() {
                let saved = commands::connect::run(commands::connect::Args {
                    connection_type: r#type,
                    provider,
                    base_url: global.base_url,
                    api_key: global.api_key,
                    pool,
                })
                .await?;
                if saved {
                    eprintln!("Next: run `char configure` to verify");
                }
                Ok(())
            } else {
                run_entry_loop(pool, global, Some("/connect".to_string())).await
            }
        }
        Some(Commands::Configure { tab }) => commands::configure::run(&pool, tab).await,
        Some(Commands::Auth) => {
            commands::auth::run()?;
            eprintln!("Opened auth page in browser");
            eprintln!("Next: run `char connect` to configure a provider");
            Ok(())
        }
        Some(Commands::Desktop) => {
            use commands::desktop::DesktopAction;
            match commands::desktop::run()? {
                DesktopAction::OpenedApp => eprintln!("Opened desktop app"),
                DesktopAction::OpenedDownloadPage => {
                    eprintln!("Desktop app not found — opened download page")
                }
            }
            Ok(())
        }
        Some(Commands::Bug) => {
            commands::bug::run()?;
            eprintln!("Opened bug report page in browser");
            Ok(())
        }
        Some(Commands::Hello) => {
            commands::hello::run()?;
            eprintln!("Opened char.com in browser");
            Ok(())
        }
        Some(Commands::Meetings { command }) => match command {
            Some(commands::meetings::Commands::New {
                provider,
                audio,
                keywords,
            }) => {
                if let Some(audio_input) = audio {
                    let stt = resolve_stt_args(&pool, &global, provider).await?;
                    commands::meetings::new_from_audio(audio_input, stt, keywords, pool).await
                } else {
                    let stt = resolve_stt_args(&pool, &global, provider).await?;
                    commands::meetings::live::run(commands::meetings::live::Args {
                        stt,
                        record: global.record,
                        audio: commands::meetings::live::AudioMode::Dual,
                        pool,
                    })
                    .await
                }
            }
            Some(commands::meetings::Commands::View { id }) => {
                commands::meetings::view::run(commands::meetings::view::Args {
                    meeting_id: id,
                    pool,
                })
                .await
            }
            Some(commands::meetings::Commands::Participants { id }) => {
                commands::meetings::participants(&pool, &id).await
            }
            Some(commands::meetings::Commands::AddParticipant { meeting, human }) => {
                commands::meetings::add_participant(&pool, &meeting, &human).await
            }
            Some(commands::meetings::Commands::RmParticipant { meeting, human }) => {
                commands::meetings::remove_participant(&pool, &meeting, &human).await
            }
            None => {
                let selected = commands::meetings::run(pool.clone()).await?;
                if let Some(meeting_id) = selected {
                    commands::meetings::view::run(commands::meetings::view::Args {
                        meeting_id,
                        pool,
                    })
                    .await
                } else {
                    Ok(())
                }
            }
        },
        Some(Commands::Humans { command }) => commands::humans::run(&pool, command).await,
        Some(Commands::Orgs { command }) => commands::orgs::run(&pool, command).await,
        Some(Commands::Transcribe { args }) => {
            let stt = SttGlobalArgs {
                provider: args.provider,
                base_url: global.base_url,
                api_key: global.api_key,
                model: global.model,
                language: global.language,
            };
            commands::transcribe::run(args, stt, verbose.is_silent()).await
        }
        Some(Commands::Export { command }) => commands::export::run(&pool, command).await,
        Some(Commands::Models { command }) => commands::model::run(command, &pool).await,
        #[cfg(feature = "dev")]
        Some(Commands::Debug { command }) => commands::debug::run(command).await,
        Some(Commands::Completions { shell }) => {
            cli::generate_completions(shell);
            Ok(())
        }
        None => run_entry_loop(pool, global, None).await,
    }
}

async fn run_entry_loop(
    pool: SqlitePool,
    global: cli::GlobalArgs,
    initial_command: Option<String>,
) -> CliResult<()> {
    let mut status_message: Option<String> = None;
    let mut initial_cmd = initial_command;
    loop {
        let settings = load_entry_settings(&pool).await;
        let action = commands::entry::run(commands::entry::Args {
            status_message: status_message.take(),
            initial_command: initial_cmd.take(),
            stt_provider: settings
                .as_ref()
                .and_then(|value| value.current_stt_provider.clone()),
            llm_provider: settings
                .as_ref()
                .and_then(|value| value.current_llm_provider.clone()),
            pool: pool.clone(),
        })
        .await;
        match action {
            commands::entry::EntryAction::Launch(cmd) => match cmd {
                commands::entry::EntryCommand::MeetingsNew => {
                    let stt = match resolve_stt_args(&pool, &global, None).await {
                        Ok(stt) => stt,
                        Err(e) => {
                            status_message = Some(e.to_string());
                            continue;
                        }
                    };

                    return commands::meetings::live::run(commands::meetings::live::Args {
                        stt,
                        record: global.record,
                        audio: commands::meetings::live::AudioMode::Dual,
                        pool: pool.clone(),
                    })
                    .await;
                }
                commands::entry::EntryCommand::Chat { session_id } => {
                    return commands::chat::run(commands::chat::Args {
                        meeting: session_id,
                        prompt: None,
                        provider: None,
                        base_url: global.base_url.clone(),
                        api_key: global.api_key.clone(),
                        model: global.model.clone(),
                        pool: pool.clone(),
                        resume_meeting_id: None,
                    })
                    .await;
                }
                commands::entry::EntryCommand::View { session_id } => {
                    return commands::meetings::view::run(commands::meetings::view::Args {
                        meeting_id: session_id,
                        pool: pool.clone(),
                    })
                    .await;
                }
            },
            commands::entry::EntryAction::Model(cmd) => {
                if let Err(e) = commands::model::run(cmd, &pool).await {
                    status_message = Some(format!("model error: {e}"));
                }
            }
            commands::entry::EntryAction::Quit => return Ok(()),
        }
    }
}

async fn load_entry_settings(pool: &SqlitePool) -> Option<config::paths::Settings> {
    config::paths::load_settings_from_db(pool).await
}

async fn resolve_stt_args(
    pool: &SqlitePool,
    global: &cli::GlobalArgs,
    explicit_provider: Option<cli::Provider>,
) -> CliResult<SttGlobalArgs> {
    let resolved = match explicit_provider {
        Some(p) => EntryListenConfig {
            provider: p,
            model: None,
        },
        None => {
            let settings = load_entry_settings(pool).await;
            resolve_entry_listen_config(settings.as_ref()).map_err(|msg| {
                error::CliError::msg(format!(
                    "{msg}\nOr pass --provider explicitly: char meetings new -p <provider>"
                ))
            })?
        }
    };

    Ok(SttGlobalArgs {
        provider: resolved.provider,
        base_url: global.base_url.clone(),
        api_key: global.api_key.clone(),
        model: global.model.clone().or(resolved.model),
        language: global.language.clone(),
    })
}

struct EntryListenConfig {
    provider: cli::Provider,
    model: Option<String>,
}

fn resolve_entry_listen_config(
    settings: Option<&config::paths::Settings>,
) -> Result<EntryListenConfig, String> {
    let Some(settings) = settings else {
        return Err("No STT provider configured. Run /connect.".to_string());
    };

    let Some(provider_id) = settings.current_stt_provider.as_deref() else {
        return Err("No STT provider configured. Run /connect.".to_string());
    };

    let saved_model = settings
        .current_stt_model
        .clone()
        .filter(|value| !value.trim().is_empty());

    let provider = match provider_id {
        "deepgram" => cli::Provider::Deepgram,
        "soniox" => cli::Provider::Soniox,
        "assemblyai" => cli::Provider::Assemblyai,
        "fireworks" => cli::Provider::Fireworks,
        "openai" => cli::Provider::Openai,
        "gladia" => cli::Provider::Gladia,
        "elevenlabs" => cli::Provider::Elevenlabs,
        "mistral" => cli::Provider::Mistral,
        "hyprnote" => resolve_hyprnote_listen_provider(saved_model.as_deref())?,
        #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
        "cactus" => cli::Provider::Cactus,
        _ => {
            return Err(format!(
                "Configured STT provider `{provider_id}` is not supported by CLI listen."
            ));
        }
    };

    Ok(EntryListenConfig {
        provider,
        model: saved_model,
    })
}

fn resolve_hyprnote_listen_provider(model: Option<&str>) -> Result<cli::Provider, String> {
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    if model.is_some_and(|value| value.starts_with("cactus-")) {
        return Ok(cli::Provider::Cactus);
    }

    Err(
        "Configured STT provider `hyprnote` is not supported by CLI listen. Run /connect to choose a supported provider."
            .to_string(),
    )
}

fn run_update(action: &update_check::UpdateAction) -> CliResult<()> {
    eprintln!("Running: {}", action.command_str());
    let status = action
        .run()
        .map_err(|e| error::CliError::operation_failed("update", e.to_string()))?;

    if status.success() {
        eprintln!("Update complete!");
    } else {
        eprintln!("Update failed (exit code: {})", status.code().unwrap_or(1));
    }
    Ok(())
}
