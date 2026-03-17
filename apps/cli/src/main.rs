mod agent;
mod cli;
mod commands;
mod config;
mod error;
mod llm;
mod output;
mod theme;
mod widgets;

use clap::Parser;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;

use crate::cli::{Cli, Commands};
use crate::config::stt::SttGlobalArgs;
use crate::error::CliResult;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let tui_chat = matches!(&cli.command, Some(Commands::Chat { prompt: None, .. }));
    let tui_entry = cli.command.is_none()
        || matches!(
            &cli.command,
            Some(Commands::Connect {
                r#type: None,
                provider: None
            })
        );
    let skip_tracing_init = {
        #[cfg(feature = "dev")]
        {
            matches!(
                &cli.command,
                Some(Commands::Debug {
                    command: cli::DebugCommands::Transcribe { .. }
                })
            )
        }
        #[cfg(not(feature = "dev"))]
        {
            false
        }
    };

    if cli.global.no_color || std::env::var_os("NO_COLOR").is_some() {
        colored::control::set_override(false);
    }

    if !skip_tracing_init {
        let default_directive = if tui_chat || tui_entry {
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

    match command {
        Some(Commands::Chat {
            session,
            prompt,
            provider,
        }) => {
            commands::chat::run(commands::chat::Args {
                session,
                prompt,
                provider,
                base_url: global.base_url,
                api_key: global.api_key,
                model: global.model,
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
                })
                .await?;
                if saved {
                    eprintln!("Next: run `char status` to verify");
                }
                Ok(())
            } else {
                run_entry_loop(global, Some("/connect".to_string())).await
            }
        }
        Some(Commands::Status) => commands::status::run(),
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
        Some(Commands::Listen { provider, audio }) => {
            commands::listen::run(commands::listen::Args {
                stt: SttGlobalArgs {
                    provider,
                    base_url: global.base_url,
                    api_key: global.api_key,
                    model: global.model,
                    language: global.language,
                },
                record: global.record,
                audio,
            })
            .await
        }
        Some(Commands::Batch { args }) => {
            let stt = SttGlobalArgs {
                provider: args.provider,
                base_url: global.base_url,
                api_key: global.api_key,
                model: global.model,
                language: global.language,
            };
            commands::batch::run(args, stt, verbose.is_silent()).await
        }
        Some(Commands::Model { command }) => commands::model::run(command).await,
        #[cfg(feature = "dev")]
        Some(Commands::Debug { command }) => commands::debug::run(command).await,
        Some(Commands::Completions { shell }) => {
            cli::generate_completions(shell);
            Ok(())
        }
        None => run_entry_loop(global, None).await,
    }
}

async fn run_entry_loop(global: cli::GlobalArgs, initial_command: Option<String>) -> CliResult<()> {
    let mut status_message: Option<String> = None;
    let mut initial_cmd = initial_command;
    loop {
        let settings = load_entry_settings();
        let action = commands::entry::run(commands::entry::Args {
            status_message: status_message.take(),
            initial_command: initial_cmd.take(),
            stt_provider: settings
                .as_ref()
                .and_then(|value| value.current_stt_provider.clone()),
            llm_provider: settings
                .as_ref()
                .and_then(|value| value.current_llm_provider.clone()),
        })
        .await;
        match action {
            commands::entry::EntryAction::Listen => {
                let listen = match resolve_entry_listen_config(settings.as_ref()) {
                    Ok(listen) => listen,
                    Err(message) => {
                        status_message = Some(message);
                        continue;
                    }
                };

                return commands::listen::run(commands::listen::Args {
                    stt: SttGlobalArgs {
                        provider: listen.provider,
                        base_url: global.base_url.clone(),
                        api_key: global.api_key.clone(),
                        model: global.model.clone().or(listen.model),
                        language: global.language.clone(),
                    },
                    record: global.record,
                    audio: cli::AudioMode::Dual,
                })
                .await;
            }
            commands::entry::EntryAction::Connect => {
                let saved = commands::connect::run(commands::connect::Args {
                    connection_type: None,
                    provider: None,
                    base_url: None,
                    api_key: None,
                })
                .await?;
                if saved {
                    status_message = Some("Provider configured".into());
                }
            }
            commands::entry::EntryAction::Quit => return Ok(()),
        }
    }
}

fn load_entry_settings() -> Option<config::desktop::DesktopSettings> {
    let paths = config::desktop::resolve_paths();
    config::desktop::load_settings(&paths.settings_path)
}

struct EntryListenConfig {
    provider: cli::Provider,
    model: Option<String>,
}

fn resolve_entry_listen_config(
    settings: Option<&config::desktop::DesktopSettings>,
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
